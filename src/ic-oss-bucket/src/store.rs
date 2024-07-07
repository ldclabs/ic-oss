use candid::{CandidType, Principal};
use ciborium::{from_reader, into_writer};
use ic_http_certification::{
    cel::{create_cel_expr, DefaultCelBuilder},
    HttpCertification, HttpCertificationPath, HttpCertificationTree, HttpCertificationTreeEntry,
};
use ic_oss_types::{
    file::{FileChunk, FileInfo, MAX_CHUNK_SIZE, MAX_FILE_SIZE, MAX_FILE_SIZE_PER_CALL},
    folder::{FolderInfo, FolderName},
    ByteN, MapValue,
};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, Storable,
};
use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    ops::{self, Deref, DerefMut},
};

use crate::MILLISECONDS;

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Clone, Default, Deserialize, Serialize)]
pub struct Bucket {
    pub name: String,
    pub file_count: u64,
    pub file_id: u32,
    pub folder_count: u64,
    pub folder_id: u32,
    pub max_file_size: u64,
    pub max_folder_depth: u8,
    pub max_children: u16,
    pub status: i8,     // -1: archived; 0: readable and writable; 1: readonly
    pub visibility: u8, // 0: private; 1: public
    pub max_custom_data_size: u16,
    pub enable_hash_index: bool,
    pub managers: BTreeSet<Principal>, // managers can read and write
    // auditors can read and list even if the bucket is private
    pub auditors: BTreeSet<Principal>,
    // used to verify the request token signed with SECP256K1
    pub trusted_ecdsa_pub_keys: Vec<ByteBuf>,
    // used to verify the request token signed with ED25519
    pub trusted_eddsa_pub_keys: Vec<ByteBuf>,
}

impl Storable for Bucket {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode Bucket data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Bucket data")
    }
}

// FileId: (file id, chunk id)
// a file is a collection of chunks.
#[derive(Clone, Default, Deserialize, Serialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct FileId(pub u32, pub u32);
impl Storable for FileId {
    const BOUND: Bound = Bound::Bounded {
        max_size: 11,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode FileId data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode FileId data")
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct FileMetadata {
    pub parent: u32, // 0: root
    pub name: String,
    pub content_type: String, // MIME types
    pub size: u64,
    pub filled: u64,
    pub created_at: u64, // unix timestamp in milliseconds
    pub updated_at: u64, // unix timestamp in milliseconds
    pub chunks: u32,
    pub status: i8,              // -1: archived; 0: readable and writable; 1: readonly
    pub hash: Option<ByteN<32>>, // recommend sha3 256
    pub dek: Option<ByteN<32>>,  // Data Encryption Key
    pub custom: Option<MapValue>, // custom metadata
    pub ex: Option<MapValue>, // External Resource, ER indicates that the file is an external resource.
}

impl Storable for FileMetadata {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode FileMetadata data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode FileMetadata data")
    }
}

impl FileMetadata {
    pub fn into_info(self, id: u32) -> FileInfo {
        FileInfo {
            id,
            parent: self.parent,
            name: self.name,
            content_type: self.content_type,
            size: self.size,
            filled: self.filled,
            created_at: self.created_at,
            updated_at: self.updated_at,
            chunks: self.chunks,
            status: self.status,
            hash: self.hash,
            custom: self.custom,
            ex: self.ex,
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Chunk(pub Vec<u8>);

impl Storable for Chunk {
    const BOUND: Bound = Bound::Bounded {
        max_size: MAX_CHUNK_SIZE,
        is_fixed_size: false,
    };

    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Borrowed(&self.0)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self(bytes.to_vec())
    }
}

// folder
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct FolderMetadata {
    pub parent: u32, // 0: root
    pub name: String,
    pub files: BTreeSet<u32>,   // length <= max_children
    pub folders: BTreeSet<u32>, // length <= max_children
    pub created_at: u64,        // unix timestamp in milliseconds
    pub updated_at: u64,        // unix timestamp in milliseconds
    pub status: i8,             // -1: archived; 0: readable and writable; 1: readonly
}

impl FolderMetadata {
    pub fn into_info(self, id: u32) -> FolderInfo {
        FolderInfo {
            id,
            parent: self.parent,
            name: self.name,
            created_at: self.created_at,
            updated_at: self.updated_at,
            status: self.status,
            files: self.files,
            folders: self.folders,
        }
    }
}

impl Storable for FolderMetadata {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode FolderMetadata data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode FolderMetadata data")
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct FoldersTree(BTreeMap<u32, FolderMetadata>);

impl Deref for FoldersTree {
    type Target = BTreeMap<u32, FolderMetadata>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FoldersTree {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<BTreeMap<u32, FolderMetadata>> for FoldersTree {
    fn as_ref(&self) -> &BTreeMap<u32, FolderMetadata> {
        &self.0
    }
}

impl FoldersTree {
    fn depth(&self, mut id: u32) -> usize {
        let mut depth = 0;
        // depth hard limit is 1024
        while id != 0 && depth < 1024 {
            match self.get(&id) {
                None => break,
                Some(folder) => {
                    id = folder.parent;
                    depth += 1;
                }
            }
        }
        depth
    }

    fn depth_or_is_ancestor(&self, mut id: u32, parent: u32) -> (usize, bool) {
        let mut depth = 0;
        while id != 0 && depth < 1024 {
            if id == parent {
                return (depth, true);
            }

            match self.get(&id) {
                None => break,
                Some(folder) => {
                    id = folder.parent;
                    depth += 1;
                }
            }
        }
        (depth, false)
    }

    fn ancestors(&self, mut parent: u32) -> Vec<FolderName> {
        let mut res = Vec::with_capacity(10);
        while parent != 0 {
            match self.get(&parent) {
                None => break,
                Some(folder) => {
                    res.push(FolderName {
                        id: parent,
                        name: folder.name.clone(),
                    });
                    parent = folder.parent;
                }
            }
        }
        res
    }

    fn list_folders(&self, parent: u32) -> Vec<FolderInfo> {
        match self.0.get(&parent) {
            None => Vec::new(),
            Some(parent) => {
                let mut res = Vec::with_capacity(parent.folders.len());
                for &folder_id in parent.folders.iter().rev() {
                    match self.get(&folder_id) {
                        None => break,
                        Some(folder) => {
                            res.push(folder.clone().into_info(folder_id));
                        }
                    }
                }
                res
            }
        }
    }

    fn list_files(
        &self,
        fs_metadata: &StableBTreeMap<u32, FileMetadata, Memory>,
        parent: u32,
        prev: u32,
        take: u32,
    ) -> Vec<FileInfo> {
        match self.get(&parent) {
            None => Vec::new(),
            Some(parent) => {
                let mut res = Vec::with_capacity(take as usize);
                for &file_id in parent.files.range(ops::RangeTo { end: prev }).rev() {
                    match fs_metadata.get(&file_id) {
                        None => break,
                        Some(meta) => {
                            res.push(meta.into_info(file_id));
                            if res.len() >= take as usize {
                                break;
                            }
                        }
                    }
                }
                res
            }
        }
    }

    fn add_folder(
        &mut self,
        metadata: FolderMetadata,
        id: u32, // id should be unique
        max_folder_depth: usize,
        max_children: usize,
    ) -> Result<(), String> {
        if self.depth(metadata.parent) >= max_folder_depth {
            Err("folder depth exceeds limit".to_string())?;
        }

        let parent = self
            .get_mut(&metadata.parent)
            .ok_or_else(|| format!("parent folder not found: {}", metadata.parent))?;

        if parent.status != 0 {
            Err("parent folder is not writeable".to_string())?;
        }

        if parent.folders.len() + parent.files.len() >= max_children {
            Err("children exceeds limit".to_string())?;
        }
        parent.folders.insert(id);
        self.insert(id, metadata);
        Ok(())
    }

    fn parent_add_file(
        &mut self,
        parent: u32,
        max_children: usize,
    ) -> Result<&mut FolderMetadata, String> {
        let folder = self
            .get_mut(&parent)
            .ok_or_else(|| format!("parent folder not found: {}", parent))?;

        if folder.status != 0 {
            Err("parent folder is not writeable".to_string())?;
        }

        if folder.folders.len() + folder.files.len() >= max_children {
            Err("children exceeds limit".to_string())?;
        }

        Ok(folder)
    }

    fn check_moving_folder(
        &self,
        id: u32,
        from: u32,
        to: u32,
        max_folder_depth: usize,
        max_children: usize,
    ) -> Result<(), String> {
        let folder = self
            .get(&id)
            .ok_or_else(|| format!("folder not found: {}", id))?;

        if folder.parent != from {
            Err(format!("folder {} is not in folder {}", id, from))?;
        }
        if folder.status != 0 {
            Err(format!("folder {} is not writeable", id))?;
        }

        let from_folder = self
            .get(&from)
            .ok_or_else(|| format!("folder not found: {}", from))?;
        if from_folder.status != 0 {
            Err(format!("folder {} is not writeable", from))?;
        }

        let to_folder = self
            .get(&to)
            .ok_or_else(|| format!("folder not found: {}", to))?;
        if to_folder.status != 0 {
            Err(format!("folder {} is not writeable", to))?;
        }

        if to_folder.folders.len() + to_folder.files.len() >= max_children {
            Err("children exceeds limit".to_string())?;
        }

        let (depth, is_ancestor) = self.depth_or_is_ancestor(to, id);
        if is_ancestor {
            Err("folder cannot be moved to its sub folder".to_string())?;
        }

        if depth >= max_folder_depth {
            Err("folder depth exceeds limit".to_string())?;
        }

        Ok(())
    }

    fn move_folder(&mut self, id: u32, from: u32, to: u32, now_ms: u64) {
        self.entry(from).and_modify(|from_folder| {
            from_folder.folders.remove(&id);
            from_folder.updated_at = now_ms;
        });
        self.entry(to).and_modify(|to_folder| {
            to_folder.folders.insert(id);
            to_folder.updated_at = now_ms;
        });
        self.entry(id).and_modify(|folder| {
            folder.parent = to;
            folder.updated_at = now_ms;
        });
    }

    fn check_moving_file(&self, from: u32, to: u32, max_children: usize) -> Result<(), String> {
        let from_folder = self
            .get(&from)
            .ok_or_else(|| format!("folder not found: {}", from))?;
        if from_folder.status != 0 {
            Err(format!("folder {} is not writeable", from))?;
        }

        let to_folder = self
            .get(&to)
            .ok_or_else(|| format!("folder not found: {}", to))?;
        if to_folder.status != 0 {
            Err(format!("folder {} is not writeable", to))?;
        }

        if to_folder.folders.len() + to_folder.files.len() >= max_children {
            Err("children exceeds limit".to_string())?;
        }

        Ok(())
    }

    fn move_file(&mut self, id: u32, from: u32, to: u32, now_ms: u64) {
        self.entry(from).and_modify(|from_folder| {
            from_folder.files.remove(&id);
            from_folder.updated_at = now_ms;
        });
        self.entry(to).and_modify(|to_folder| {
            to_folder.files.insert(id);
            to_folder.updated_at = now_ms;
        });
    }

    fn delete_folder(&mut self, id: u32, now_ms: u64) -> Result<bool, String> {
        let parent_id = match self.get(&id) {
            None => return Ok(false),
            Some(folder) => {
                if folder.status > 0 {
                    return Err("folder is readonly".to_string());
                }
                if !folder.folders.is_empty() || !folder.files.is_empty() {
                    return Err("folder is not empty".to_string());
                }
                folder.parent
            }
        };

        let parent = self
            .get_mut(&parent_id)
            .ok_or_else(|| format!("parent folder not found: {}", parent_id))?;

        if parent.status != 0 {
            Err("parent folder is not writeable".to_string())?;
        }

        if parent.folders.remove(&id) {
            parent.updated_at = now_ms;
        }

        Ok(self.remove(&id).is_some())
    }
}

const BUCKET_MEMORY_ID: MemoryId = MemoryId::new(0);
const HASH_INDEX_MEMORY_ID: MemoryId = MemoryId::new(1);
const FOLDERS_MEMORY_ID: MemoryId = MemoryId::new(2);
const FS_METADATA_MEMORY_ID: MemoryId = MemoryId::new(3);
const FS_DATA_MEMORY_ID: MemoryId = MemoryId::new(4);

thread_local! {
    static HTTP_TREE: RefCell<HttpCertificationTree> = RefCell::new(HttpCertificationTree::default());
    static BUCKET: RefCell<Bucket> = RefCell::new(Bucket::default());
    static HASHS: RefCell<BTreeMap<ByteArray<32>, u32>> = RefCell::new(BTreeMap::default());
    static FOLDERS: RefCell<FoldersTree> = RefCell::new(FoldersTree(BTreeMap::from([(0, FolderMetadata{
        name: "root".to_string(),
        ..Default::default()
    })])));

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static BUCKET_STORE: RefCell<StableCell<Bucket, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(BUCKET_MEMORY_ID)),
            Bucket::default()
        ).expect("failed to init BUCKET_STORE store")
    );

    static FOLDER_STORE: RefCell<StableCell<Vec<u8>, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(FOLDERS_MEMORY_ID)),
            Vec::new()
        ).expect("failed to init FOLDER_STORE store")
    );

    static HASH_INDEX: RefCell<StableCell<Vec<u8>, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(HASH_INDEX_MEMORY_ID)),
            Vec::new()
        ).expect("failed to init HASH_INDEX store")
    );

    static FS_METADATA: RefCell<StableBTreeMap<u32, FileMetadata, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(FS_METADATA_MEMORY_ID)),
        )
    );

    static FS_DATA: RefCell<StableBTreeMap<FileId, Chunk, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(FS_DATA_MEMORY_ID)),
        )
    );
}

pub mod state {
    use super::*;

    lazy_static! {
        pub static ref DEFAULT_EXPR_PATH: HttpCertificationPath<'static> =
            HttpCertificationPath::wildcard("");
        pub static ref DEFAULT_CERTIFICATION: HttpCertification = HttpCertification::skip();
        pub static ref DEFAULT_CEL_EXPR: String =
            create_cel_expr(&DefaultCelBuilder::skip_certification());
    }

    pub static DEFAULT_CERT_ENTRY: Lazy<HttpCertificationTreeEntry> =
        Lazy::new(|| HttpCertificationTreeEntry::new(&*DEFAULT_EXPR_PATH, *DEFAULT_CERTIFICATION));

    pub fn is_manager(caller: &Principal) -> bool {
        BUCKET.with(|r| r.borrow().managers.contains(caller))
    }

    pub fn with<R>(f: impl FnOnce(&Bucket) -> R) -> R {
        BUCKET.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut Bucket) -> R) -> R {
        BUCKET.with(|r| f(&mut r.borrow_mut()))
    }

    pub fn http_tree_with<R>(f: impl FnOnce(&HttpCertificationTree) -> R) -> R {
        HTTP_TREE.with(|r| f(&r.borrow()))
    }

    pub fn init_http_certified_data() {
        HTTP_TREE.with(|r| {
            let mut tree = r.borrow_mut();
            tree.insert(&DEFAULT_CERT_ENTRY);
            ic_cdk::api::set_certified_data(&tree.root_hash())
        });
    }

    pub fn load() {
        BUCKET_STORE.with(|r| {
            let s = r.borrow().get().clone();
            BUCKET.with(|h| {
                *h.borrow_mut() = s;
            });
        });
        HASH_INDEX.with(|r| {
            HASHS.with(|h| {
                let v: BTreeMap<ByteArray<32>, u32> =
                    from_reader(&r.borrow().get()[..]).expect("failed to decode HASH_INDEX data");
                *h.borrow_mut() = v;
            });
        });
        FOLDER_STORE.with(|r| {
            FOLDERS.with(|h| {
                let v: FoldersTree =
                    from_reader(&r.borrow().get()[..]).expect("failed to decode FOLDER_STORE data");
                *h.borrow_mut() = v;
            });
        });
    }

    pub fn save() {
        BUCKET.with(|h| {
            BUCKET_STORE.with(|r| {
                r.borrow_mut()
                    .set(h.borrow().clone())
                    .expect("failed to set BUCKET_STORE data");
            });
        });
        HASHS.with(|h| {
            HASH_INDEX.with(|r| {
                let mut buf = vec![];
                into_writer(&(*h.borrow()), &mut buf).expect("failed to encode HASH_INDEX data");
                r.borrow_mut()
                    .set(buf)
                    .expect("failed to set HASH_INDEX data");
            });
        });
        FOLDERS.with(|h| {
            FOLDER_STORE.with(|r| {
                let mut buf = vec![];
                into_writer(&(*h.borrow()), &mut buf).expect("failed to encode FOLDER_STORE data");
                r.borrow_mut()
                    .set(buf)
                    .expect("failed to set FOLDER_STORE data");
            });
        });
    }
}

pub mod fs {
    use super::*;

    pub fn get_file_id(hash: &[u8; 32]) -> Option<u32> {
        HASHS.with(|r| r.borrow().get(hash).copied())
    }

    pub fn get_folder(id: u32) -> Option<FolderMetadata> {
        FOLDERS.with(|r| r.borrow().get(&id).cloned())
    }

    pub fn get_file(id: u32) -> Option<FileMetadata> {
        FS_METADATA.with(|r| r.borrow().get(&id))
    }

    pub fn get_folder_ancestors(id: u32) -> Vec<FolderName> {
        FOLDERS.with(|r| {
            let m = r.borrow();
            match m.get(&id) {
                None => Vec::new(),
                Some(folder) => m.ancestors(folder.parent),
            }
        })
    }

    pub fn get_file_ancestors(id: u32) -> Vec<FolderName> {
        match FS_METADATA.with(|r| r.borrow().get(&id).map(|meta| meta.parent)) {
            None => Vec::new(),
            Some(parent) => FOLDERS.with(|r| r.borrow().ancestors(parent)),
        }
    }

    pub fn list_folders(parent: u32) -> Vec<FolderInfo> {
        FOLDERS.with(|r| r.borrow().list_folders(parent))
    }

    pub fn list_files(parent: u32, prev: u32, take: u32) -> Vec<FileInfo> {
        FOLDERS.with(|r1| {
            FS_METADATA.with(|r2| r1.borrow().list_files(&r2.borrow(), parent, prev, take))
        })
    }

    pub fn add_folder(metadata: FolderMetadata) -> Result<u32, String> {
        state::with_mut(|s| {
            FOLDERS.with(|r| {
                let id = s.folder_id.saturating_add(1);
                if id == u32::MAX {
                    return Err("folder id overflow".to_string());
                }

                let mut m = r.borrow_mut();
                m.add_folder(
                    metadata,
                    id,
                    s.max_folder_depth as usize,
                    s.max_children as usize,
                )?;

                s.folder_id = id;
                s.folder_count += 1;
                Ok(id)
            })
        })
    }

    pub fn add_file(metadata: FileMetadata) -> Result<u32, String> {
        state::with_mut(|s| {
            FOLDERS.with(|r| {
                let id = s.file_id.saturating_add(1);
                if id == u32::MAX {
                    return Err("file id overflow".to_string());
                }

                let mut m = r.borrow_mut();
                let parent = m.parent_add_file(metadata.parent, s.max_children as usize)?;

                if s.enable_hash_index {
                    if let Some(ref hash) = metadata.hash {
                        HASHS.with(|r| {
                            let mut m = r.borrow_mut();
                            if let Some(prev) = m.get(hash.as_ref()) {
                                return Err(format!("file hash conflict, {}", prev));
                            }

                            m.insert(hash.0, id);
                            Ok(())
                        })?;
                    }
                }

                s.file_id = id;
                s.file_count += 1;
                parent.files.insert(id);
                FS_METADATA.with(|r| r.borrow_mut().insert(id, metadata));
                Ok(id)
            })
        })
    }

    pub fn move_folder(id: u32, from: u32, to: u32) -> Result<u64, String> {
        if from == to {
            Err(format!("target parent folder should not be {}", from))?;
        }

        state::with_mut(|s| {
            FOLDERS.with(|r| {
                {
                    r.borrow().check_moving_folder(
                        id,
                        from,
                        to,
                        s.max_folder_depth as usize,
                        s.max_children as usize,
                    )?;
                };

                let now_ms = ic_cdk::api::time() / MILLISECONDS;
                r.borrow_mut().move_folder(id, from, to, now_ms);
                Ok(now_ms)
            })
        })
    }

    pub fn move_file(id: u32, from: u32, to: u32) -> Result<u64, String> {
        if from == to {
            Err(format!("target parent should not be {}", from))?;
        }

        state::with_mut(|s| {
            FOLDERS.with(|r| {
                {
                    r.borrow()
                        .check_moving_file(from, to, s.max_children as usize)?;
                };

                let now_ms = ic_cdk::api::time() / MILLISECONDS;
                FS_METADATA.with(|r| {
                    let mut m = r.borrow_mut();
                    let mut file = m
                        .get(&id)
                        .ok_or_else(|| format!("file not found: {}", id))?;

                    if file.status != 0 {
                        return Err(format!("file {} is not writeable", id));
                    }

                    if file.parent != from {
                        return Err(format!("file {} is not in folder {}", id, from));
                    }

                    file.parent = to;
                    file.updated_at = now_ms;
                    m.insert(id, file);
                    Ok(())
                })?;

                r.borrow_mut().move_file(id, from, to, now_ms);
                Ok(now_ms)
            })
        })
    }

    pub fn update_folder<R>(
        id: u32,
        f: impl FnOnce(&mut FolderMetadata) -> R,
    ) -> Result<R, String> {
        FOLDERS.with(|r| {
            let mut m = r.borrow_mut();
            match m.get_mut(&id) {
                None => Err(format!("folder not found: {}", id)),
                Some(folder) => {
                    if folder.status > 0 {
                        return Err("folder is readonly".to_string());
                    }

                    Ok(f(folder))
                }
            }
        })
    }

    pub fn update_file<R>(id: u32, f: impl FnOnce(&mut FileMetadata) -> R) -> Result<R, String> {
        FS_METADATA.with(|r| {
            let mut m = r.borrow_mut();
            match m.get(&id) {
                None => Err(format!("file not found: {}", id)),
                Some(mut file) => {
                    let prev_hash = file.hash;
                    if file.status > 0 {
                        Err("file is readonly".to_string())?;
                    }

                    let r = f(&mut file);
                    let enable_hash_index = state::with(|s| s.enable_hash_index);
                    if enable_hash_index && prev_hash != file.hash {
                        HASHS.with(|r| {
                            let mut hm = r.borrow_mut();
                            if let Some(ref hash) = file.hash {
                                if let Some(prev) = hm.get(&hash.0) {
                                    Err(format!("file hash conflict, {}", prev))?;
                                }
                                hm.insert(hash.0, id);
                            }
                            if let Some(prev_hash) = prev_hash {
                                hm.remove(&prev_hash.0);
                            }
                            Ok::<(), String>(())
                        })?;
                    }
                    m.insert(id, file);
                    Ok(r)
                }
            }
        })
    }

    pub fn get_chunk(id: u32, chunk_index: u32) -> Option<FileChunk> {
        FS_DATA.with(|r| {
            r.borrow()
                .get(&FileId(id, chunk_index))
                .map(|v| FileChunk(chunk_index, ByteBuf::from(v.0)))
        })
    }

    pub fn get_chunks(id: u32, chunk_index: u32, max_take: u32) -> Vec<FileChunk> {
        FS_DATA.with(|r| {
            let mut buf: Vec<FileChunk> = Vec::with_capacity(max_take as usize);
            if max_take > 0 {
                let mut filled = 0usize;
                for (FileId(_, index), Chunk(chunk)) in r.borrow().range((
                    ops::Bound::Included(FileId(id, chunk_index)),
                    ops::Bound::Included(FileId(id, chunk_index + max_take - 1)),
                )) {
                    filled += chunk.len();
                    if filled > MAX_FILE_SIZE_PER_CALL as usize {
                        break;
                    }

                    buf.push(FileChunk(index, ByteBuf::from(chunk)));
                    if filled == MAX_FILE_SIZE_PER_CALL as usize {
                        break;
                    }
                }
            }

            buf
        })
    }

    pub fn get_full_chunks(id: u32) -> Result<Vec<u8>, String> {
        let (size, chunks) = FS_METADATA.with(|r| match r.borrow().get(&id) {
            None => Err(format!("file not found: {}", id)),
            Some(file) => {
                if file.size != file.filled {
                    return Err("file not fully uploaded".to_string());
                }
                Ok((file.size, file.chunks))
            }
        })?;

        if size > MAX_FILE_SIZE.min(usize::MAX as u64) {
            return Err(format!(
                "file size exceeds limit: {}",
                MAX_FILE_SIZE.min(usize::MAX as u64)
            ));
        }

        FS_DATA.with(|r| {
            let mut filled = 0usize;
            let mut buf = Vec::with_capacity(size as usize);
            if chunks == 0 {
                return Ok(buf);
            }

            for (_, chunk) in r.borrow().range((
                ops::Bound::Included(FileId(id, 0)),
                ops::Bound::Included(FileId(id, chunks - 1)),
            )) {
                filled += chunk.0.len();
                buf.extend_from_slice(&chunk.0);
            }

            if filled as u64 != size {
                return Err(format!(
                    "file size mismatch, expected {}, got {}",
                    size, filled
                ));
            }
            Ok(buf)
        })
    }

    pub fn update_chunk(
        file_id: u32,
        chunk_index: u32,
        now_ms: u64,
        chunk: Vec<u8>,
    ) -> Result<u64, String> {
        if chunk.is_empty() {
            return Err("empty chunk".to_string());
        }

        if chunk.len() > MAX_CHUNK_SIZE as usize {
            return Err(format!(
                "chunk size too large, max size is {} bytes",
                MAX_CHUNK_SIZE
            ));
        }

        let max = state::with(|s| s.max_file_size);
        FS_METADATA.with(|r| {
            let mut m = r.borrow_mut();
            match m.get(&file_id) {
                None => Err(format!("file not found: {}", file_id)),
                Some(mut file) => {
                    if file.status != 0 {
                        return Err(format!("file {} is not writeable", file_id));
                    }

                    file.updated_at = now_ms;
                    file.filled += chunk.len() as u64;
                    if file.filled > max {
                        panic!("file size exceeds limit: {}", max);
                    }

                    match FS_DATA.with(|r| {
                        r.borrow_mut()
                            .insert(FileId(file_id, chunk_index), Chunk(chunk))
                    }) {
                        None => {
                            if file.chunks <= chunk_index {
                                file.chunks = chunk_index + 1;
                            }
                        }
                        Some(old) => {
                            file.filled -= old.0.len() as u64;
                        }
                    }

                    let filled = file.filled;
                    if file.size < filled {
                        file.size = filled;
                    }

                    m.insert(file_id, file);
                    Ok(filled)
                }
            }
        })
    }

    pub fn delete_folder(id: u32) -> Result<bool, String> {
        if id == 0 {
            return Err("root folder cannot be deleted".to_string());
        }

        FOLDERS.with(|r| {
            r.borrow_mut()
                .delete_folder(id, ic_cdk::api::time() / MILLISECONDS)
        })
    }

    pub fn delete_file(id: u32) -> Result<bool, String> {
        FS_METADATA.with(|r| {
            let mut m = r.borrow_mut();
            match m.get(&id) {
                Some(file) => {
                    if file.status > 0 {
                        return Err("file is readonly".to_string());
                    }

                    FOLDERS.with(|r| {
                        let mut m = r.borrow_mut();
                        let parent = m
                            .get_mut(&file.parent)
                            .ok_or_else(|| format!("parent folder not found: {}", file.parent))?;

                        if parent.status != 0 {
                            Err("parent folder is not writeable".to_string())?;
                        }
                        parent.files.remove(&id);
                        parent.updated_at = ic_cdk::api::time() / MILLISECONDS;
                        Ok::<(), String>(())
                    })?;

                    m.remove(&id);
                    if let Some(hash) = file.hash {
                        HASHS.with(|r| r.borrow_mut().remove(&hash.0));
                    }
                    FS_DATA.with(|r| {
                        let mut fs_data = r.borrow_mut();
                        for i in 0..file.chunks {
                            fs_data.remove(&FileId(id, i));
                        }
                    });
                    Ok(true)
                }
                None => Ok(false),
            }
        })
    }

    pub fn batch_delete_subfiles(parent: u32, ids: BTreeSet<u32>) -> Result<Vec<u32>, String> {
        FOLDERS.with(|r| {
            let mut folders = r.borrow_mut();
            let folder = folders
                .get_mut(&parent)
                .ok_or_else(|| format!("parent folder not found: {}", parent))?;

            if folder.status != 0 {
                Err("parent folder is not writeable".to_string())?;
            }

            FS_METADATA.with(|r| {
                let mut fs_metadata = r.borrow_mut();
                let mut removed = Vec::with_capacity(ids.len());

                FS_DATA.with(|r| {
                    let mut fs_data = r.borrow_mut();
                    for id in ids {
                        if folder.files.contains(&id) {
                            match fs_metadata.get(&id) {
                                Some(file) => {
                                    if file.status < 1 && fs_metadata.remove(&id).is_some() {
                                        removed.push(id);
                                        folder.files.remove(&id);
                                        if let Some(hash) = file.hash {
                                            HASHS.with(|r| r.borrow_mut().remove(&hash.0));
                                        }

                                        for i in 0..file.chunks {
                                            fs_data.remove(&FileId(id, i));
                                        }
                                    }
                                }
                                None => {
                                    folder.files.remove(&id);
                                }
                            }
                        }
                    }
                });

                if !removed.is_empty() {
                    folder.updated_at = ic_cdk::api::time() / MILLISECONDS;
                }
                Ok(removed)
            })
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_bound_max_size() {
        let v = FileId(u32::MAX, u32::MAX);
        let v = v.to_bytes();
        println!("FileId max_size: {:?}, {}", v.len(), hex::encode(&v));

        let v = FileId(0u32, 0u32);
        let v = v.to_bytes();
        println!("FileId min_size: {:?}, {}", v.len(), hex::encode(&v));
    }

    #[test]
    fn test_file() {
        state::with_mut(|b| {
            b.name = "default".to_string();
            b.max_file_size = MAX_FILE_SIZE;
            b.max_folder_depth = 10;
            b.max_children = 1000;
        });

        assert!(fs::get_file(0).is_none());
        assert!(fs::get_full_chunks(0).is_err());
        assert!(fs::get_full_chunks(1).is_err());

        let f1 = fs::add_file(FileMetadata {
            name: "f1.bin".to_string(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(f1, 1);

        assert!(fs::get_full_chunks(0).is_err());
        let f1_data = fs::get_full_chunks(f1).unwrap();
        assert!(f1_data.is_empty());

        let f1_meta = fs::get_file(f1).unwrap();
        assert_eq!(f1_meta.name, "f1.bin");

        assert!(fs::update_chunk(0, 0, 999, [0u8; 32].to_vec()).is_err());
        let _ = fs::update_chunk(f1, 0, 999, [0u8; 32].to_vec()).unwrap();
        let _ = fs::update_chunk(f1, 1, 1000, [0u8; 32].to_vec()).unwrap();
        let f1_data = fs::get_full_chunks(f1).unwrap();
        assert_eq!(f1_data, [0u8; 64]);

        let f1_meta = fs::get_file(f1).unwrap();
        assert_eq!(f1_meta.name, "f1.bin");
        assert_eq!(f1_meta.size, 64);
        assert_eq!(f1_meta.filled, 64);
        assert_eq!(f1_meta.chunks, 2);

        let f2 = fs::add_file(FileMetadata {
            name: "f2.bin".to_string(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(f2, 2);
        fs::update_chunk(f2, 0, 999, [0u8; 16].to_vec()).unwrap();
        fs::update_chunk(f2, 1, 1000, [1u8; 16].to_vec()).unwrap();
        fs::update_chunk(f1, 3, 1000, [1u8; 16].to_vec()).unwrap();
        fs::update_chunk(f2, 2, 1000, [2u8; 16].to_vec()).unwrap();
        fs::update_chunk(f1, 2, 1000, [2u8; 16].to_vec()).unwrap();

        let f1_data = fs::get_full_chunks(f1).unwrap();
        assert_eq!(&f1_data[0..64], &[0u8; 64]);
        assert_eq!(&f1_data[64..80], &[2u8; 16]);
        assert_eq!(&f1_data[80..96], &[1u8; 16]);

        let f1_meta = fs::get_file(f1).unwrap();
        assert_eq!(f1_meta.size, 96);
        assert_eq!(f1_meta.filled, 96);
        assert_eq!(f1_meta.chunks, 4);

        let f2_data = fs::get_full_chunks(f2).unwrap();
        assert_eq!(&f2_data[0..16], &[0u8; 16]);
        assert_eq!(&f2_data[16..32], &[1u8; 16]);
        assert_eq!(&f2_data[32..48], &[2u8; 16]);

        let f2_meta = fs::get_file(f2).unwrap();
        assert_eq!(f2_meta.size, 48);
        assert_eq!(f2_meta.filled, 48);
        assert_eq!(f2_meta.chunks, 3);
    }
}
