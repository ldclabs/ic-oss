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
    ops,
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
    pub er: Option<MapValue>, // External Resource, ER indicates that the file is an external resource.
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
            er: self.er,
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
    pub ancestors: Vec<u32>,    // parent, [parent's upper layer, ...], root
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
            ancestors: self.ancestors,
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
pub struct RootChildren {
    pub files: BTreeSet<u32>,
    pub folders: BTreeSet<u32>,
}

impl Storable for RootChildren {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode RootChildren data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode RootChildren data")
    }
}

const BUCKET_MEMORY_ID: MemoryId = MemoryId::new(0);
const HASH_INDEX_MEMORY_ID: MemoryId = MemoryId::new(1);
const FOLDERS_MEMORY_ID: MemoryId = MemoryId::new(2);
const FS_METADATA_MEMORY_ID: MemoryId = MemoryId::new(3);
const FS_DATA_MEMORY_ID: MemoryId = MemoryId::new(4);

thread_local! {
    static HTTP_TREE: RefCell<HttpCertificationTree> = RefCell::new(HttpCertificationTree::default());
    static BUCKET_HEAP: RefCell<Bucket> = RefCell::new(Bucket::default());
    static HASHS_HEAP: RefCell<BTreeMap<ByteArray<32>, u32>> = RefCell::new(BTreeMap::default());
    static FOLDERS_HEAP: RefCell<BTreeMap<u32, FolderMetadata>> = RefCell::new(BTreeMap::from([(0, FolderMetadata{
        name: "root".to_string(),
        ..Default::default()
    })]));

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static BUCKET: RefCell<StableCell<Bucket, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(BUCKET_MEMORY_ID)),
            Bucket::default()
        ).expect("failed to init BUCKET store")
    );

    static FOLDERS: RefCell<StableCell<Vec<u8>, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(FOLDERS_MEMORY_ID)),
            Vec::new()
        ).expect("failed to init FOLDERS store")
    );

    static HASH_INDEX: RefCell<StableCell<Vec<u8>, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(HASH_INDEX_MEMORY_ID)),
            Vec::new()
        ).expect("failed to init FOLDERS store")
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
        BUCKET_HEAP.with(|r| r.borrow().managers.contains(caller))
    }

    pub fn with<R>(f: impl FnOnce(&Bucket) -> R) -> R {
        BUCKET_HEAP.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut Bucket) -> R) -> R {
        BUCKET_HEAP.with(|r| f(&mut r.borrow_mut()))
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
        BUCKET.with(|r| {
            let s = r.borrow().get().clone();
            BUCKET_HEAP.with(|h| {
                *h.borrow_mut() = s;
            });
        });
        HASH_INDEX.with(|r| {
            HASHS_HEAP.with(|h| {
                let v: BTreeMap<ByteArray<32>, u32> =
                    from_reader(&r.borrow().get()[..]).expect("failed to decode HASH_INDEX data");
                *h.borrow_mut() = v;
            });
        });
        FOLDERS.with(|r| {
            FOLDERS_HEAP.with(|h| {
                let v: BTreeMap<u32, FolderMetadata> =
                    from_reader(&r.borrow().get()[..]).expect("failed to decode FOLDERS data");
                *h.borrow_mut() = v;
            });
        });
    }

    pub fn save() {
        BUCKET_HEAP.with(|h| {
            BUCKET.with(|r| {
                r.borrow_mut()
                    .set(h.borrow().clone())
                    .expect("failed to set BUCKET data");
            });
        });
        HASHS_HEAP.with(|h| {
            HASH_INDEX.with(|r| {
                let mut buf = vec![];
                into_writer(&(*h.borrow()), &mut buf).expect("failed to encode HASH_INDEX data");
                r.borrow_mut()
                    .set(buf)
                    .expect("failed to set HASH_INDEX data");
            });
        });
        FOLDERS_HEAP.with(|h| {
            FOLDERS.with(|r| {
                let mut buf = vec![];
                into_writer(&(*h.borrow()), &mut buf).expect("failed to encode FOLDERS data");
                r.borrow_mut().set(buf).expect("failed to set FOLDERS data");
            });
        });
    }
}

pub mod fs {
    use super::*;

    pub fn get_file_id(hash: &[u8; 32]) -> Option<u32> {
        HASHS_HEAP.with(|r| r.borrow().get(hash).copied())
    }

    pub fn get_folder(id: u32) -> Option<FolderMetadata> {
        FOLDERS_HEAP.with(|r| r.borrow().get(&id).cloned())
    }

    pub fn get_file(id: u32) -> Option<FileMetadata> {
        FS_METADATA.with(|r| r.borrow().get(&id))
    }

    pub fn get_folder_ancestors(id: u32) -> Vec<FolderName> {
        FOLDERS_HEAP.with(|r| {
            let m = r.borrow();
            match m.get(&id) {
                None => Vec::new(),
                Some(folder) => {
                    let mut res = Vec::with_capacity(folder.ancestors.len());
                    for &folder_id in folder.ancestors.iter() {
                        if let Some(meta) = m.get(&folder_id) {
                            res.push(FolderName {
                                id: folder_id,
                                name: meta.name.clone(),
                            });
                        }
                    }
                    res
                }
            }
        })
    }

    pub fn get_file_ancestors(id: u32) -> Vec<FolderName> {
        match FS_METADATA.with(|r| r.borrow().get(&id).map(|meta| meta.parent)) {
            None => Vec::new(),
            Some(parent) => FOLDERS_HEAP.with(|r| {
                let m = r.borrow();
                match m.get(&parent) {
                    None => Vec::new(),
                    Some(folder) => {
                        let mut res = Vec::with_capacity(folder.ancestors.len() + 1);
                        res.push(FolderName {
                            id: parent,
                            name: folder.name.clone(),
                        });

                        for &folder_id in folder.ancestors.iter() {
                            if let Some(meta) = m.get(&folder_id) {
                                res.push(FolderName {
                                    id: folder_id,
                                    name: meta.name.clone(),
                                });
                            }
                        }
                        res
                    }
                }
            }),
        }
    }

    pub fn list_folders(parent: u32) -> Vec<FolderInfo> {
        FOLDERS_HEAP.with(|r| {
            let m = r.borrow();
            match m.get(&parent) {
                None => Vec::new(),
                Some(parent) => {
                    let mut res = Vec::with_capacity(parent.folders.len());
                    for &folder_id in parent.folders.iter().rev() {
                        if let Some(meta) = m.get(&folder_id) {
                            res.push(meta.clone().into_info(folder_id));
                        }
                    }
                    res
                }
            }
        })
    }

    pub fn list_files(parent: u32, prev: u32, take: u32) -> Vec<FileInfo> {
        FOLDERS_HEAP.with(|r| match r.borrow().get(&parent) {
            None => Vec::new(),
            Some(folder) => FS_METADATA.with(|r| {
                let m = r.borrow();
                let mut res = Vec::with_capacity(take as usize);
                for &file_id in folder.files.iter().rev() {
                    if file_id >= prev {
                        continue;
                    }
                    if let Some(meta) = m.get(&file_id) {
                        res.push(meta.into_info(file_id));
                        if res.len() >= take as usize {
                            break;
                        }
                    }
                }
                res
            }),
        })
    }

    pub fn add_folder(mut meta: FolderMetadata) -> Result<u32, String> {
        state::with_mut(|s| {
            FOLDERS_HEAP.with(|r| {
                let mut m = r.borrow_mut();
                let parent = m
                    .get_mut(&meta.parent)
                    .ok_or_else(|| format!("parent folder not found: {}", meta.parent))?;

                if parent.status != 0 {
                    return Err("parent folder is not writeable".to_string());
                }

                if parent.ancestors.len() >= s.max_folder_depth as usize {
                    return Err("folder depth exceeds limit".to_string());
                }

                if parent.folders.len() + parent.files.len() >= s.max_children as usize {
                    return Err("children exceeds limit".to_string());
                }

                meta.ancestors.push(meta.parent);
                meta.ancestors
                    .extend_from_slice(parent.ancestors.as_slice());

                let id = s.folder_id.saturating_add(1);
                if id == u32::MAX {
                    return Err("folder id overflow".to_string());
                }

                s.folder_id = id;
                parent.folders.insert(id);
                m.insert(id, meta);
                Ok(id)
            })
        })
    }

    pub fn add_file(meta: FileMetadata) -> Result<u32, String> {
        state::with_mut(|s| {
            FOLDERS_HEAP.with(|r| {
                let mut m = r.borrow_mut();
                let folder = m
                    .get_mut(&meta.parent)
                    .ok_or_else(|| format!("parent folder not found: {}", meta.parent))?;

                if folder.folders.len() + folder.files.len() >= s.max_children as usize {
                    return Err("children exceeds limit".to_string());
                }

                if folder.status != 0 {
                    return Err("parent folder is not writeable".to_string());
                }

                let id = s.file_id.saturating_add(1);
                if id == u32::MAX {
                    return Err("file id overflow".to_string());
                }

                if s.enable_hash_index {
                    if let Some(ref hash) = meta.hash {
                        HASHS_HEAP.with(|r| {
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
                folder.files.insert(id);
                FS_METADATA.with(|r| r.borrow_mut().insert(id, meta));
                Ok(id)
            })
        })
    }

    pub fn move_folder(id: u32, from: u32, to: u32) -> Result<u64, String> {
        if from == to {
            Err(format!("target parent should not be {}", from))?;
        }

        state::with_mut(|s| {
            FOLDERS_HEAP.with(|r| {
                let ancestors: Vec<u32> = {
                    let m = r.borrow();
                    let folder = m
                        .get(&id)
                        .ok_or_else(|| format!("folder not found: {}", id))?;

                    if folder.parent != from {
                        return Err(format!("folder {} is not in folder {}", id, from));
                    }
                    if folder.status != 0 {
                        return Err(format!("folder {} is not writeable", id));
                    }

                    let to_folder = m
                        .get(&to)
                        .ok_or_else(|| format!("folder not found: {}", to))?;
                    if to_folder.status != 0 {
                        return Err(format!("folder {} is not writeable", to));
                    }
                    if to_folder.ancestors.len() >= s.max_folder_depth as usize {
                        return Err("folder depth exceeds limit".to_string());
                    }

                    if to_folder.folders.len() + to_folder.files.len() >= s.max_children as usize {
                        return Err("children exceeds limit".to_string());
                    }

                    if to_folder.ancestors.contains(&id) {
                        return Err("folder cannot be moved to its sub folder".to_string());
                    }

                    let mut ancestors = Vec::with_capacity(to_folder.ancestors.len() + 1);
                    ancestors.push(to);
                    ancestors.extend_from_slice(to_folder.ancestors.as_slice());
                    ancestors
                };

                let mut m = r.borrow_mut();
                let now_ms = ic_cdk::api::time() / MILLISECONDS;
                m.entry(from).and_modify(|from_folder| {
                    from_folder.folders.remove(&id);
                    from_folder.updated_at = now_ms;
                });
                m.entry(id).and_modify(|folder| {
                    folder.parent = to;
                    folder.ancestors = ancestors;
                    folder.updated_at = now_ms;
                });
                m.entry(to).and_modify(|to_folder| {
                    to_folder.folders.insert(id);
                    to_folder.updated_at = now_ms;
                });

                Ok(now_ms)
            })
        })
    }

    pub fn move_file(id: u32, from: u32, to: u32) -> Result<u64, String> {
        if from == to {
            Err(format!("target parent should not be {}", from))?;
        }

        state::with_mut(|s| {
            FOLDERS_HEAP.with(|r| {
                {
                    let m = r.borrow();

                    let to_folder = m
                        .get(&to)
                        .ok_or_else(|| format!("folder not found: {}", to))?;
                    if to_folder.status != 0 {
                        return Err(format!("folder {} is not writeable", to));
                    }

                    if to_folder.folders.len() + to_folder.files.len() >= s.max_children as usize {
                        return Err("children exceeds limit".to_string());
                    }
                };

                let now_ms = ic_cdk::api::time() / MILLISECONDS;
                FS_METADATA.with(|r| {
                    let mut m = r.borrow_mut();
                    let mut metadata = m
                        .get(&id)
                        .ok_or_else(|| format!("file not found: {}", id))?;

                    if metadata.status > 0 {
                        return Err("file is readonly".to_string());
                    }

                    if metadata.parent != from {
                        return Err(format!("file {} is not in folder {}", id, from));
                    }

                    metadata.parent = to;
                    metadata.updated_at = now_ms;
                    m.insert(id, metadata);
                    Ok(())
                })?;

                let mut m = r.borrow_mut();
                m.entry(from).and_modify(|from_folder| {
                    from_folder.files.remove(&id);
                    from_folder.updated_at = now_ms;
                });
                m.entry(to).and_modify(|to_folder| {
                    to_folder.files.insert(id);
                    to_folder.updated_at = now_ms;
                });
                Ok(now_ms)
            })
        })
    }

    pub fn update_folder<R>(
        id: u32,
        f: impl FnOnce(&mut FolderMetadata) -> R,
    ) -> Result<R, String> {
        FOLDERS_HEAP.with(|r| {
            let mut m = r.borrow_mut();
            match m.get_mut(&id) {
                None => Err(format!("folder not found: {}", id)),
                Some(metadata) => {
                    if metadata.status > 0 {
                        return Err("folder is readonly".to_string());
                    }

                    Ok(f(metadata))
                }
            }
        })
    }

    pub fn update_file<R>(id: u32, f: impl FnOnce(&mut FileMetadata) -> R) -> Result<R, String> {
        FS_METADATA.with(|r| {
            let mut m = r.borrow_mut();
            match m.get(&id) {
                None => Err(format!("file not found: {}", id)),
                Some(mut metadata) => {
                    let prev_hash = metadata.hash;
                    if metadata.status > 0 {
                        return Err("file is readonly".to_string());
                    }

                    let r = f(&mut metadata);
                    let enable_hash_index = state::with(|s| s.enable_hash_index);
                    if enable_hash_index && prev_hash != metadata.hash {
                        HASHS_HEAP.with(|r| {
                            let mut hm = r.borrow_mut();
                            if let Some(ref hash) = metadata.hash {
                                if let Some(prev) = hm.get(&hash.0) {
                                    return Err(format!("file hash conflict, {}", prev));
                                }
                                hm.insert(hash.0, id);
                            }
                            if let Some(prev_hash) = prev_hash {
                                hm.remove(&prev_hash.0);
                            }
                            Ok(())
                        })?;
                    }
                    m.insert(id, metadata);
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
            Some(meta) => {
                if meta.size != meta.filled {
                    return Err("file not fully uploaded".to_string());
                }
                Ok((meta.size, meta.chunks))
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
                Some(mut metadata) => {
                    if metadata.status > 0 {
                        return Err("file is readonly".to_string());
                    }

                    metadata.updated_at = now_ms;
                    metadata.filled += chunk.len() as u64;
                    if metadata.filled > max {
                        panic!("file size exceeds limit: {}", max);
                    }

                    match FS_DATA.with(|r| {
                        r.borrow_mut()
                            .insert(FileId(file_id, chunk_index), Chunk(chunk))
                    }) {
                        None => {
                            if metadata.chunks <= chunk_index {
                                metadata.chunks = chunk_index + 1;
                            }
                        }
                        Some(old) => {
                            metadata.filled -= old.0.len() as u64;
                        }
                    }

                    let filled = metadata.filled;
                    if metadata.size < filled {
                        metadata.size = filled;
                    }

                    m.insert(file_id, metadata);
                    Ok(filled)
                }
            }
        })
    }

    pub fn delete_folder(id: u32) -> Result<bool, String> {
        if id == 0 {
            return Err("root folder cannot be deleted".to_string());
        }

        let now_ms = ic_cdk::api::time() / MILLISECONDS;

        FOLDERS_HEAP.with(|r| {
            let parent = {
                let m = r.borrow();
                if let Some(metadata) = m.get(&id) {
                    if metadata.status > 0 {
                        return Err("folder is readonly".to_string());
                    }
                    if !metadata.folders.is_empty() || !metadata.files.is_empty() {
                        return Err("folder is not empty".to_string());
                    }
                    Some(metadata.parent)
                } else {
                    None
                }
            };

            if let Some(parent) = parent {
                let mut m = r.borrow_mut();
                m.entry(parent).and_modify(|folder| {
                    folder.folders.remove(&id);
                    folder.updated_at = now_ms;
                });
                m.remove(&id);
            }

            Ok(parent.is_some())
        })
    }

    pub fn delete_file(id: u32) -> Result<bool, String> {
        let now_ms = ic_cdk::api::time() / MILLISECONDS;
        let metadata = FS_METADATA.with(|r| {
            let mut m = r.borrow_mut();
            if let Some(metadata) = m.get(&id) {
                if metadata.status > 0 {
                    return Err("file is readonly".to_string());
                }
                m.remove(&id);
                Ok(Some(metadata))
            } else {
                Ok(None)
            }
        })?;

        if let Some(metadata) = metadata {
            FOLDERS_HEAP.with(|r| {
                r.borrow_mut().entry(metadata.parent).and_modify(|folder| {
                    folder.files.remove(&id);
                    folder.updated_at = now_ms;
                });
            });

            if let Some(hash) = metadata.hash {
                HASHS_HEAP.with(|r| r.borrow_mut().remove(&hash.0));
            }

            FS_DATA.with(|r| {
                for chunk_index in 0..metadata.chunks {
                    r.borrow_mut().remove(&FileId(id, chunk_index));
                }
            });
            return Ok(true);
        }

        Ok(false)
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
    fn test_fs() {
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
