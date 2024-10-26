use candid::Principal;
use ciborium::{from_reader, into_writer};
use ic_http_certification::{
    cel::{create_cel_expr, DefaultCelBuilder},
    HttpCertification, HttpCertificationPath, HttpCertificationTree, HttpCertificationTreeEntry,
};
use ic_oss_types::{
    cose::{Token, BUCKET_TOKEN_AAD},
    file::{
        FileChunk, FileInfo, UpdateFileInput, CHUNK_SIZE, CUSTOM_KEY_BY_HASH, MAX_FILE_SIZE,
        MAX_FILE_SIZE_PER_CALL,
    },
    folder::{FolderInfo, FolderName, UpdateFolderInput},
    permission::Policies,
    MapValue,
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

type Memory = VirtualMemory<DefaultMemoryImpl>;

static ZERO_HASH: [u8; 32] = [0; 32];

#[derive(Clone, Deserialize, Serialize)]
pub struct Bucket {
    #[serde(rename = "n", alias = "name")]
    pub name: String,
    #[serde(rename = "fi", alias = "file_id")]
    pub file_id: u32,
    #[serde(rename = "fo", alias = "folder_id")]
    pub folder_id: u32,
    #[serde(rename = "fz", alias = "max_file_size")]
    pub max_file_size: u64,
    #[serde(rename = "fd", alias = "max_folder_depth")]
    pub max_folder_depth: u8,
    #[serde(rename = "mc", alias = "max_children")]
    pub max_children: u16,
    #[serde(rename = "cds", alias = "max_custom_data_size")]
    pub max_custom_data_size: u16,
    #[serde(rename = "h", alias = "enable_hash_index")]
    pub enable_hash_index: bool,
    #[serde(rename = "s", alias = "status")]
    pub status: i8, // -1: archived; 0: readable and writable; 1: readonly
    #[serde(rename = "v", alias = "visibility")]
    pub visibility: u8, // 0: private; 1: public
    #[serde(rename = "m", alias = "managers")]
    pub managers: BTreeSet<Principal>, // managers can read and write
    // auditors can read and list even if the bucket is private
    #[serde(rename = "a", alias = "auditors")]
    pub auditors: BTreeSet<Principal>,
    // used to verify the request token signed with SECP256K1
    #[serde(rename = "ec", alias = "trusted_ecdsa_pub_keys")]
    pub trusted_ecdsa_pub_keys: Vec<ByteBuf>,
    // used to verify the request token signed with ED25519
    #[serde(rename = "ed", alias = "trusted_eddsa_pub_keys")]
    pub trusted_eddsa_pub_keys: Vec<ByteArray<32>>,
    #[serde(default, rename = "gov")]
    pub governance_canister: Option<Principal>,
}

impl Default for Bucket {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            file_id: 0,
            folder_id: 1, // The root folder 0 is created by default
            max_file_size: MAX_FILE_SIZE,
            max_folder_depth: 10,
            max_children: 100,
            max_custom_data_size: 1024 * 4,
            enable_hash_index: false,
            status: 0,
            visibility: 0,
            managers: BTreeSet::new(),
            auditors: BTreeSet::new(),
            trusted_ecdsa_pub_keys: Vec::new(),
            trusted_eddsa_pub_keys: Vec::new(),
            governance_canister: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Context {
    pub caller: Principal,
    pub ps: Policies,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum Role {
    User,
    Auditor,
    Manager,
}

impl Bucket {
    pub fn read_permission(
        &self,
        caller: Principal,
        canister: &Principal,
        sign1_token: Option<ByteBuf>,
        now_sec: u64,
    ) -> Result<Context, (u16, String)> {
        let mut ctx = Context {
            caller,
            ps: Policies::read(),
            role: if self.managers.contains(&caller) {
                Role::Manager
            } else if self.auditors.contains(&caller) {
                Role::Auditor
            } else {
                Role::User
            },
        };

        if self.status < 0 {
            if ctx.role >= Role::Auditor {
                return Ok(ctx);
            }

            Err((403, "bucket is archived".to_string()))?;
        }

        if self.visibility > 0 || ctx.role >= Role::Auditor {
            return Ok(ctx);
        }

        if let Some(token) = sign1_token {
            let token = Token::from_sign1(
                &token,
                &self.trusted_ecdsa_pub_keys,
                &self.trusted_eddsa_pub_keys,
                BUCKET_TOKEN_AAD,
                now_sec as i64,
            )
            .map_err(|err| (401, err))?;

            if &token.audience == canister {
                ctx.ps =
                    Policies::try_from(token.policies.as_str()).map_err(|err| (403u16, err))?;
                ctx.caller = token.subject;
                return Ok(ctx);
            }
        }

        Err((401, "Unauthorized".to_string()))
    }

    pub fn write_permission(
        &self,
        caller: Principal,
        canister: &Principal,
        sign1_token: Option<ByteBuf>,
        now_sec: u64,
    ) -> Result<Context, (u16, String)> {
        if self.status != 0 {
            Err((403, "bucket is not writable".to_string()))?;
        }

        let mut ctx = Context {
            caller,
            ps: Policies::all(),
            role: if self.managers.contains(&caller) {
                Role::Manager
            } else if self.auditors.contains(&caller) {
                Role::Auditor
            } else {
                Role::User
            },
        };

        if ctx.role >= Role::Manager {
            return Ok(ctx);
        }

        if let Some(token) = sign1_token {
            let token = Token::from_sign1(
                &token,
                &self.trusted_ecdsa_pub_keys,
                &self.trusted_eddsa_pub_keys,
                BUCKET_TOKEN_AAD,
                now_sec as i64,
            )
            .map_err(|err| (401, err))?;
            if &token.audience == canister {
                ctx.ps =
                    Policies::try_from(token.policies.as_str()).map_err(|err| (403u16, err))?;
                ctx.caller = token.subject;
                return Ok(ctx);
            }
        }

        Err((401, "Unauthorized".to_string()))
    }
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
    #[serde(rename = "p", alias = "parent")]
    pub parent: u32, // 0: root
    #[serde(rename = "n", alias = "name")]
    pub name: String,
    #[serde(rename = "t", alias = "content_type")]
    pub content_type: String, // MIME types
    #[serde(rename = "i", alias = "size")]
    pub size: u64,
    #[serde(rename = "f", alias = "filled")]
    pub filled: u64,
    #[serde(rename = "ca", alias = "created_at")]
    pub created_at: u64, // unix timestamp in milliseconds
    #[serde(rename = "ua", alias = "updated_at")]
    pub updated_at: u64, // unix timestamp in milliseconds
    #[serde(rename = "c", alias = "chunks")]
    pub chunks: u32,
    #[serde(rename = "s", alias = "status")]
    pub status: i8, // -1: archived; 0: readable and writable; 1: readonly
    #[serde(rename = "h", alias = "hash")]
    pub hash: Option<ByteArray<32>>, // recommend sha3 256
    #[serde(rename = "k", alias = "dek")]
    pub dek: Option<ByteBuf>, // // Data Encryption Key that encrypted by BYOK or vetKey in COSE_Encrypt0
    #[serde(rename = "cu", alias = "custom")]
    pub custom: Option<MapValue>, // custom metadata
    #[serde(rename = "e", alias = "ex")]
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
            dek: self.dek,
            custom: self.custom,
            ex: self.ex,
        }
    }

    pub fn read_by_hash(&self, access_token: &Option<ByteBuf>) -> bool {
        if let Some(access_token) = access_token {
            self.status >= 0
                && self
                    .custom
                    .as_ref()
                    .map_or(false, |c| c.contains_key(CUSTOM_KEY_BY_HASH))
                && self
                    .hash
                    .as_ref()
                    .map_or(false, |h| h.as_slice() == access_token.as_ref())
        } else {
            false
        }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Chunk(pub Vec<u8>);

impl Storable for Chunk {
    const BOUND: Bound = Bound::Bounded {
        max_size: CHUNK_SIZE,
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
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct FolderMetadata {
    #[serde(rename = "p", alias = "parent")]
    pub parent: u32, // 0: root
    #[serde(rename = "n", alias = "name")]
    pub name: String,
    #[serde(rename = "fi", alias = "files")]
    pub files: BTreeSet<u32>, // length <= max_children
    #[serde(rename = "fo", alias = "folders")]
    pub folders: BTreeSet<u32>, // length <= max_children
    #[serde(rename = "ca", alias = "created_at")]
    pub created_at: u64, // unix timestamp in milliseconds
    #[serde(rename = "ua", alias = "updated_at")]
    pub updated_at: u64, // unix timestamp in milliseconds
    #[serde(rename = "s", alias = "status")]
    pub status: i8, // -1: archived; 0: readable and writable; 1: readonly
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
    fn new() -> Self {
        Self(BTreeMap::from([(
            0,
            FolderMetadata {
                name: "root".to_string(),
                ..Default::default()
            },
        )]))
    }

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
        (depth, parent == 0)
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

    fn ancestors_map<F, U>(&self, mut parent: u32, f: F) -> Vec<U>
    where
        F: Fn(u32, &FolderMetadata) -> U,
    {
        let mut res = Vec::with_capacity(10);
        while parent != 0 {
            match self.get(&parent) {
                None => break,
                Some(folder) => {
                    res.push(f(parent, folder));
                    parent = folder.parent;
                }
            }
        }
        res
    }

    fn list_folders(&self, ctx: &Context, parent: u32, prev: u32, take: u32) -> Vec<FolderInfo> {
        match self.0.get(&parent) {
            None => Vec::new(),
            Some(parent) => {
                if parent.status < 0 && ctx.role < Role::Auditor {
                    return Vec::new();
                }

                let mut res = Vec::with_capacity(parent.folders.len());
                for &folder_id in parent.folders.range(ops::RangeTo { end: prev }).rev() {
                    match self.get(&folder_id) {
                        None => break,
                        Some(folder) => {
                            res.push(folder.clone().into_info(folder_id));
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

    fn list_files(
        &self,
        ctx: &Context,
        fs_metadata: &StableBTreeMap<u32, FileMetadata, Memory>,
        parent: u32,
        prev: u32,
        take: u32,
    ) -> Vec<FileInfo> {
        match self.get(&parent) {
            None => Vec::new(),
            Some(parent) => {
                if parent.status < 0 && ctx.role < Role::Auditor {
                    return Vec::new();
                }

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
        if self.get(&id).is_some() {
            Err(format!("folder id already exists: {}", id))?;
        }

        if self.depth(metadata.parent) >= max_folder_depth {
            Err("folder depth exceeds limit".to_string())?;
        }

        let parent = self
            .get_mut(&metadata.parent)
            .ok_or_else(|| format!("parent folder not found: {}", metadata.parent))?;

        if parent.status != 0 {
            Err("parent folder is not writable".to_string())?;
        }

        // no limit for root folder
        if metadata.parent > 0 && parent.folders.len() + parent.files.len() >= max_children {
            Err("children exceeds limit".to_string())?;
        }
        parent.folders.insert(id);
        self.insert(id, metadata);
        Ok(())
    }

    fn parent_to_update(&mut self, parent: u32) -> Result<&mut FolderMetadata, String> {
        let folder = self
            .get_mut(&parent)
            .ok_or_else(|| format!("parent folder not found: {}", parent))?;

        if folder.status != 0 {
            Err("parent folder is not writable".to_string())?;
        }

        Ok(folder)
    }

    fn parent_to_add_file(
        &mut self,
        parent: u32,
        max_children: usize,
    ) -> Result<&mut FolderMetadata, String> {
        let folder = self
            .get_mut(&parent)
            .ok_or_else(|| format!("parent folder not found: {}", parent))?;

        if folder.status != 0 {
            Err("parent folder is not writable".to_string())?;
        }

        // no limit for root folder
        if parent > 0 && folder.folders.len() + folder.files.len() >= max_children {
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
        if id == 0 {
            Err("root folder cannot be moved".to_string())?;
        }

        if from == to {
            Err(format!("target parent folder should not be {}", from))?;
        }

        let folder = self
            .get(&id)
            .ok_or_else(|| format!("folder not found: {}", id))?;

        if folder.parent != from {
            Err(format!("folder {} is not in folder {}", id, from))?;
        }
        if folder.status != 0 {
            Err(format!("folder {} is not writable", id))?;
        }

        let from_folder = self
            .get(&from)
            .ok_or_else(|| format!("folder not found: {}", from))?;
        if from_folder.status != 0 {
            Err(format!("folder {} is not writable", from))?;
        }

        let to_folder = self
            .get(&to)
            .ok_or_else(|| format!("folder not found: {}", to))?;
        if to_folder.status != 0 {
            Err(format!("folder {} is not writable", to))?;
        }

        if to > 0 && to_folder.folders.len() + to_folder.files.len() >= max_children {
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
        if from == to {
            Err(format!("target parent should not be {}", from))?;
        }

        let from_folder = self
            .get(&from)
            .ok_or_else(|| format!("folder not found: {}", from))?;
        if from_folder.status != 0 {
            Err(format!("folder {} is not writable", from))?;
        }

        let to_folder = self
            .get(&to)
            .ok_or_else(|| format!("folder not found: {}", to))?;
        if to_folder.status != 0 {
            Err(format!("folder {} is not writable", to))?;
        }

        if to > 0 && to_folder.folders.len() + to_folder.files.len() >= max_children {
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
        if id == 0 {
            Err("root folder cannot be deleted".to_string())?;
        }

        let parent_id = match self.get(&id) {
            None => return Ok(false),
            Some(folder) => {
                if folder.status > 0 {
                    Err("folder is readonly".to_string())?;
                }
                if !folder.folders.is_empty() || !folder.files.is_empty() {
                    Err("folder is not empty".to_string())?;
                }
                folder.parent
            }
        };

        let parent = self
            .get_mut(&parent_id)
            .ok_or_else(|| format!("parent folder not found: {}", parent_id))?;

        if parent.status != 0 {
            Err("parent folder is not writable".to_string())?;
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
const FS_CHUNKS_MEMORY_ID: MemoryId = MemoryId::new(4);

thread_local! {
    static HTTP_TREE: RefCell<HttpCertificationTree> = RefCell::new(HttpCertificationTree::default());
    static BUCKET: RefCell<Bucket> = RefCell::new(Bucket::default());
    static HASHS: RefCell<BTreeMap<ByteArray<32>, u32>> = RefCell::new(BTreeMap::default());
    static FOLDERS: RefCell<FoldersTree> = RefCell::new(FoldersTree::new());

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

    static HASH_INDEX_STORE: RefCell<StableCell<Vec<u8>, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(HASH_INDEX_MEMORY_ID)),
            Vec::new()
        ).expect("failed to init HASH_INDEX_STORE store")
    );

    static FS_METADATA_STORE: RefCell<StableBTreeMap<u32, FileMetadata, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(FS_METADATA_MEMORY_ID)),
        )
    );

    static FS_CHUNKS_STORE: RefCell<StableBTreeMap<FileId, Chunk, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(FS_CHUNKS_MEMORY_ID)),
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

    pub fn with<R>(f: impl FnOnce(&Bucket) -> R) -> R {
        BUCKET.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut Bucket) -> R) -> R {
        BUCKET.with(|r| f(&mut r.borrow_mut()))
    }

    pub fn is_controller(caller: &Principal) -> bool {
        BUCKET.with(|r| {
            r.borrow()
                .governance_canister
                .as_ref()
                .map_or(false, |p| p == caller)
        })
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
        HASH_INDEX_STORE.with(|r| {
            HASHS.with(|h| {
                let v: BTreeMap<ByteArray<32>, u32> = from_reader(&r.borrow().get()[..])
                    .expect("failed to decode HASH_INDEX_STORE data");
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
            HASH_INDEX_STORE.with(|r| {
                let mut buf = vec![];
                into_writer(&(*h.borrow()), &mut buf)
                    .expect("failed to encode HASH_INDEX_STORE data");
                r.borrow_mut()
                    .set(buf)
                    .expect("failed to set HASH_INDEX_STORE data");
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

    pub fn total_files() -> u64 {
        FS_METADATA_STORE.with(|r| r.borrow().len())
    }

    pub fn total_chunks() -> u64 {
        FS_CHUNKS_STORE.with(|r| r.borrow().len())
    }

    pub fn total_folders() -> u64 {
        FOLDERS.with(|r| r.borrow().len() as u64)
    }

    pub fn get_file_id(hash: &[u8; 32]) -> Option<u32> {
        HASHS.with(|r| r.borrow().get(hash).copied())
    }

    pub fn get_folder(id: u32) -> Option<FolderMetadata> {
        FOLDERS.with(|r| r.borrow().get(&id).cloned())
    }

    pub fn get_file(id: u32) -> Option<FileMetadata> {
        FS_METADATA_STORE.with(|r| r.borrow().get(&id))
    }

    pub fn get_ancestors(start: u32) -> Vec<String> {
        FOLDERS.with(|r| {
            let m = r.borrow();
            m.ancestors_map(start, |id, _| id.to_string())
        })
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
        match FS_METADATA_STORE.with(|r| r.borrow().get(&id).map(|meta| meta.parent)) {
            None => Vec::new(),
            Some(parent) => FOLDERS.with(|r| r.borrow().ancestors(parent)),
        }
    }

    pub fn list_folders(ctx: &Context, parent: u32, prev: u32, take: u32) -> Vec<FolderInfo> {
        FOLDERS.with(|r| r.borrow().list_folders(ctx, parent, prev, take))
    }

    pub fn list_files(ctx: &Context, parent: u32, prev: u32, take: u32) -> Vec<FileInfo> {
        FOLDERS.with(|r1| {
            FS_METADATA_STORE.with(|r2| {
                r1.borrow()
                    .list_files(ctx, &r2.borrow(), parent, prev, take)
            })
        })
    }

    pub fn add_folder(metadata: FolderMetadata) -> Result<u32, String> {
        state::with_mut(|s| {
            FOLDERS.with(|r| {
                let id = s.folder_id;
                if id == u32::MAX {
                    Err("folder id overflow".to_string())?;
                }

                let mut m = r.borrow_mut();
                m.add_folder(
                    metadata,
                    id,
                    s.max_folder_depth as usize,
                    s.max_children as usize,
                )?;

                s.folder_id = s.folder_id.saturating_add(1);
                Ok(id)
            })
        })
    }

    pub fn add_file(metadata: FileMetadata) -> Result<u32, String> {
        state::with_mut(|s| {
            FOLDERS.with(|r| {
                let id = s.file_id;
                if id == u32::MAX {
                    Err("file id overflow".to_string())?;
                }

                let mut m = r.borrow_mut();
                let parent = m.parent_to_add_file(metadata.parent, s.max_children as usize)?;

                if s.enable_hash_index {
                    match metadata.hash {
                        Some(hash) => {
                            // ignore zero hash, client should delete the file when hash conflict
                            if hash.as_ref() != &ZERO_HASH {
                                HASHS.with(|r| {
                                    let mut m = r.borrow_mut();
                                    if let Some(prev) = m.get(hash.as_ref()) {
                                        Err(format!("file hash conflict, {}", prev))?;
                                    }

                                    m.insert(hash, id);
                                    Ok::<(), String>(())
                                })?;
                            }
                        }
                        None => {
                            Err("file hash is required when enable_hash_index".to_string())?;
                        }
                    }
                }

                s.file_id = s.file_id.saturating_add(1);
                parent.files.insert(id);
                FS_METADATA_STORE.with(|r| r.borrow_mut().insert(id, metadata));
                Ok(id)
            })
        })
    }

    pub fn move_folder(id: u32, from: u32, to: u32, now_ms: u64) -> Result<(), String> {
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

                r.borrow_mut().move_folder(id, from, to, now_ms);
                Ok(())
            })
        })
    }

    pub fn move_file(id: u32, from: u32, to: u32, now_ms: u64) -> Result<(), String> {
        state::with_mut(|s| {
            FOLDERS.with(|r| {
                {
                    r.borrow()
                        .check_moving_file(from, to, s.max_children as usize)?;
                };

                FS_METADATA_STORE.with(|r| {
                    let mut m = r.borrow_mut();
                    let mut file = m
                        .get(&id)
                        .ok_or_else(|| format!("file not found: {}", id))?;

                    if file.status != 0 {
                        Err(format!("file {} is not writable", id))?;
                    }

                    if file.parent != from {
                        Err(format!("file {} is not in folder {}", id, from))?;
                    }

                    file.parent = to;
                    file.updated_at = now_ms;
                    m.insert(id, file);
                    Ok::<(), String>(())
                })?;

                r.borrow_mut().move_file(id, from, to, now_ms);
                Ok(())
            })
        })
    }

    pub fn update_folder(
        change: UpdateFolderInput,
        now_ms: u64,
        checker: impl FnOnce(&FolderMetadata) -> Result<(), String>,
    ) -> Result<(), String> {
        if change.id == 0 {
            Err("root folder cannot be updated".to_string())?;
        }

        FOLDERS.with(|r| {
            let mut m = r.borrow_mut();
            match m.get_mut(&change.id) {
                None => Err(format!("folder not found: {}", change.id)),
                Some(folder) => {
                    checker(folder)?;

                    let status = change.status.unwrap_or(folder.status);
                    if folder.status > 0 && status > 0 {
                        Err("folder is readonly".to_string())?;
                    }
                    if let Some(name) = change.name {
                        folder.name = name;
                    }
                    folder.status = status;
                    folder.updated_at = now_ms;
                    Ok(())
                }
            }
        })
    }

    pub fn update_file(
        change: UpdateFileInput,
        now_ms: u64,
        checker: impl FnOnce(&FileMetadata) -> Result<(), String>,
    ) -> Result<(), String> {
        FS_METADATA_STORE.with(|r| {
            let mut m = r.borrow_mut();
            match m.get(&change.id) {
                None => Err(format!("file not found: {}", change.id)),
                Some(mut file) => {
                    checker(&file)?;

                    if let Some(size) = change.size {
                        file.size = size;
                    }
                    if file.size == 0 {
                        file.size = file.filled;
                    }

                    let prev_hash = file.hash;
                    let status = change.status.unwrap_or(file.status);
                    if file.status > 0 && status > 0 {
                        Err("file is readonly".to_string())?;
                    }
                    if status == 1 && file.hash.is_none() && change.hash.is_none() {
                        Err("readonly file must have hash".to_string())?;
                    }
                    if status == 1 && file.size != file.filled {
                        Err("file not fully uploaded".to_string())?;
                    }

                    if file.size < file.filled {
                        // the file content will be deleted and should be refilled
                        file.filled = 0;
                        file.chunks = 0;
                        FS_CHUNKS_STORE.with(|r| {
                            let mut fs_data = r.borrow_mut();
                            for i in 0..file.chunks {
                                fs_data.remove(&FileId(change.id, i));
                            }
                        });
                    }

                    file.status = status;
                    if let Some(name) = change.name {
                        file.name = name;
                    }
                    if let Some(content_type) = change.content_type {
                        file.content_type = content_type;
                    }
                    if change.hash.is_some() {
                        file.hash = change.hash;
                    }
                    if change.custom.is_some() {
                        file.custom = change.custom;
                    }
                    file.updated_at = now_ms;

                    let enable_hash_index = state::with(|s| s.enable_hash_index);
                    if enable_hash_index && prev_hash != file.hash {
                        HASHS.with(|r| {
                            let mut hm = r.borrow_mut();
                            if let Some(ref hash) = file.hash {
                                if let Some(prev) = hm.get(hash) {
                                    Err(format!("file hash conflict, {}", prev))?;
                                }
                                hm.insert(*hash, change.id);
                            }
                            if let Some(prev_hash) = prev_hash {
                                hm.remove(&prev_hash);
                            }
                            Ok::<(), String>(())
                        })?;
                    }
                    m.insert(change.id, file);
                    Ok(())
                }
            }
        })
    }

    pub fn get_chunk(id: u32, chunk_index: u32) -> Option<FileChunk> {
        FS_CHUNKS_STORE.with(|r| {
            r.borrow()
                .get(&FileId(id, chunk_index))
                .map(|v| FileChunk(chunk_index, ByteBuf::from(v.0)))
        })
    }

    pub fn get_chunks(id: u32, chunk_index: u32, max_take: u32) -> Vec<FileChunk> {
        FS_CHUNKS_STORE.with(|r| {
            let mut buf: Vec<FileChunk> = Vec::with_capacity(max_take as usize);
            if max_take > 0 {
                let mut filled = 0usize;
                let m = r.borrow();
                for i in chunk_index..(chunk_index + max_take) {
                    if let Some(Chunk(chunk)) = m.get(&FileId(id, i)) {
                        filled += chunk.len();
                        if filled > MAX_FILE_SIZE_PER_CALL as usize {
                            break;
                        }

                        buf.push(FileChunk(i, ByteBuf::from(chunk)));
                        if filled == MAX_FILE_SIZE_PER_CALL as usize {
                            break;
                        }
                    }
                }
            }

            buf
        })
    }

    pub fn get_full_chunks(id: u32) -> Result<Vec<u8>, String> {
        let (size, chunks) = FS_METADATA_STORE.with(|r| match r.borrow().get(&id) {
            None => Err(format!("file not found: {}", id)),
            Some(file) => {
                if file.size != file.filled {
                    Err("file not fully uploaded".to_string())?;
                }
                Ok((file.size, file.chunks))
            }
        })?;

        if size > MAX_FILE_SIZE.min(usize::MAX as u64) {
            Err(format!(
                "file size exceeds limit: {}",
                MAX_FILE_SIZE.min(usize::MAX as u64)
            ))?;
        }

        FS_CHUNKS_STORE.with(|r| {
            let mut filled = 0usize;
            let mut buf = Vec::with_capacity(size as usize);
            if chunks == 0 {
                return Ok(buf);
            }

            let m = r.borrow();
            for i in 0..chunks {
                match m.get(&FileId(id, i)) {
                    None => Err(format!("file chunk not found: {}, {}", id, i))?,
                    Some(Chunk(chunk)) => {
                        filled += chunk.len();
                        buf.extend_from_slice(&chunk);
                    }
                }
            }

            if filled as u64 != size {
                Err(format!(
                    "file size mismatch, expected {}, got {}",
                    size, filled
                ))?;
            }
            Ok(buf)
        })
    }

    pub fn update_chunk(
        file_id: u32,
        chunk_index: u32,
        now_ms: u64,
        chunk: Vec<u8>,
        checker: impl FnOnce(&FileMetadata) -> Result<(), String>,
    ) -> Result<u64, String> {
        if chunk.is_empty() {
            Err("empty chunk".to_string())?;
        }

        if chunk.len() > CHUNK_SIZE as usize {
            Err(format!(
                "chunk size too large, max size is {} bytes",
                CHUNK_SIZE
            ))?;
        }

        let max = state::with(|s| s.max_file_size);
        FS_METADATA_STORE.with(|r| {
            let mut m = r.borrow_mut();
            match m.get(&file_id) {
                None => Err(format!("file not found: {}", file_id)),
                Some(mut file) => {
                    if file.status != 0 {
                        Err(format!("file {} is not writable", file_id))?;
                    }

                    checker(&file)?;
                    file.updated_at = now_ms;
                    file.filled += chunk.len() as u64;
                    if file.filled > max {
                        Err(format!("file size exceeds limit: {}", max))?;
                    }

                    match FS_CHUNKS_STORE.with(|r| {
                        r.borrow_mut()
                            .insert(FileId(file_id, chunk_index), Chunk(chunk))
                    }) {
                        None => {}
                        Some(old) => {
                            if chunk_index < file.chunks {
                                file.filled = file.filled.saturating_sub(old.0.len() as u64);
                            }
                        }
                    }

                    if file.chunks <= chunk_index {
                        file.chunks = chunk_index + 1;
                    }

                    let filled = file.filled;
                    if file.size > 0 && filled > file.size {
                        Err(format!(
                            "file size mismatch, expected {}, got {}",
                            file.size, filled
                        ))?;
                    }

                    m.insert(file_id, file);
                    Ok(filled)
                }
            }
        })
    }

    pub fn delete_folder(
        id: u32,
        now_ms: u64,
        checker: impl FnOnce(&FolderMetadata) -> Result<(), String>,
    ) -> Result<bool, String> {
        if id == 0 {
            Err("root folder cannot be deleted".to_string())?;
        }

        FOLDERS.with(|r| {
            let mut folders = r.borrow_mut();
            let folder = folders.parent_to_update(id)?;
            let files = folder.files.clone();
            checker(folder)?;

            FS_METADATA_STORE.with(|r| {
                let mut fs_metadata = r.borrow_mut();

                FS_CHUNKS_STORE.with(|r| {
                    let mut fs_data = r.borrow_mut();
                    for id in files {
                        match fs_metadata.get(&id) {
                            Some(file) => {
                                if file.status < 1 && fs_metadata.remove(&id).is_some() {
                                    folder.files.remove(&id);
                                    if let Some(hash) = file.hash {
                                        HASHS.with(|r| r.borrow_mut().remove(&hash));
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
                });
            });
            folders.delete_folder(id, now_ms)
        })
    }

    pub fn delete_file(
        id: u32,
        now_ms: u64,
        checker: impl FnOnce(&FileMetadata) -> Result<(), String>,
    ) -> Result<bool, String> {
        FS_METADATA_STORE.with(|r| {
            let mut m = r.borrow_mut();
            match m.get(&id) {
                Some(file) => {
                    if file.status > 0 {
                        Err("file is readonly".to_string())?;
                    }

                    checker(&file)?;

                    FOLDERS.with(|r| {
                        let mut m = r.borrow_mut();
                        let parent = m.parent_to_update(file.parent)?;
                        parent.files.remove(&id);
                        parent.updated_at = now_ms;
                        Ok::<(), String>(())
                    })?;

                    m.remove(&id);
                    if let Some(hash) = file.hash {
                        HASHS.with(|r| r.borrow_mut().remove(&hash));
                    }
                    FS_CHUNKS_STORE.with(|r| {
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

    pub fn batch_delete_subfiles(
        parent: u32,
        ids: BTreeSet<u32>,
        now_ms: u64,
    ) -> Result<Vec<u32>, String> {
        FOLDERS.with(|r| {
            let mut folders = r.borrow_mut();
            let folder = folders.parent_to_update(parent)?;

            FS_METADATA_STORE.with(|r| {
                let mut fs_metadata = r.borrow_mut();
                let mut removed = Vec::with_capacity(ids.len());

                FS_CHUNKS_STORE.with(|r| {
                    let mut fs_data = r.borrow_mut();
                    for id in ids {
                        if folder.files.contains(&id) {
                            match fs_metadata.get(&id) {
                                Some(file) => {
                                    if file.status < 1 && fs_metadata.remove(&id).is_some() {
                                        removed.push(id);
                                        folder.files.remove(&id);
                                        if let Some(hash) = file.hash {
                                            HASHS.with(|r| r.borrow_mut().remove(&hash));
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
                    folder.updated_at = now_ms;
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
    fn test_role() {
        assert!(Role::Manager > Role::Auditor);
        assert!(Role::Auditor > Role::User);
    }

    #[test]
    fn test_fs() {
        state::with_mut(|b| {
            b.enable_hash_index = true;
        });

        assert!(fs::get_file(0).is_none());
        assert!(fs::get_full_chunks(0).is_err());
        assert!(fs::get_full_chunks(1).is_err());

        let f1 = fs::add_file(FileMetadata {
            name: "f1.bin".to_string(),
            hash: Some(ByteArray::from([1u8; 32])),
            size: 0,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(f1, 0);

        let f1_data = fs::get_full_chunks(f1).unwrap();
        assert!(f1_data.is_empty());

        let f1_meta = fs::get_file(f1).unwrap();
        assert_eq!(f1_meta.name, "f1.bin");

        let _ = fs::update_chunk(f1, 0, 999, [0u8; 32].to_vec(), |_| Ok(())).unwrap();
        let _ = fs::update_chunk(f1, 1, 1000, [0u8; 32].to_vec(), |_| Ok(())).unwrap();
        let res = fs::get_full_chunks(f1);
        assert!(res.is_err());
        fs::update_file(
            UpdateFileInput {
                id: f1,
                size: Some(64),
                ..Default::default()
            },
            1000,
            |_| Ok(()),
        )
        .unwrap();
        let f1_data = fs::get_full_chunks(f1).unwrap();
        assert_eq!(f1_data, [0u8; 64]);

        let f1_meta = fs::get_file(f1).unwrap();
        assert_eq!(f1_meta.name, "f1.bin");
        assert_eq!(f1_meta.size, 64);
        assert_eq!(f1_meta.filled, 64);
        assert_eq!(f1_meta.chunks, 2);

        assert!(fs::add_file(FileMetadata {
            name: "f2.bin".to_string(),
            hash: Some(ByteArray::from([1u8; 32])),
            ..Default::default()
        })
        .is_err());

        let f2 = fs::add_file(FileMetadata {
            name: "f2.bin".to_string(),
            hash: Some(ByteArray::from([2u8; 32])),
            size: 48,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(f2, 1);
        fs::update_chunk(f2, 0, 999, [0u8; 16].to_vec(), |_| Ok(())).unwrap();
        fs::update_chunk(f2, 1, 1000, [1u8; 16].to_vec(), |_| Ok(())).unwrap();

        fs::update_file(
            UpdateFileInput {
                id: f1,
                size: Some(96),
                ..Default::default()
            },
            1000,
            |_| Ok(()),
        )
        .unwrap();
        fs::update_chunk(f1, 3, 1000, [1u8; 16].to_vec(), |_| Ok(())).unwrap();
        fs::update_chunk(f2, 2, 1000, [2u8; 16].to_vec(), |_| Ok(())).unwrap();
        fs::update_chunk(f1, 2, 1000, [2u8; 16].to_vec(), |_| Ok(())).unwrap();

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

        // folders
        let ctx = Context {
            caller: Principal::anonymous(),
            ps: Policies::default(),
            role: Role::Manager,
        };

        assert_eq!(
            fs::list_folders(&ctx, 0, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            Vec::<u32>::new()
        );

        assert_eq!(
            fs::list_files(&ctx, 0, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            vec![f2, f1]
        );

        assert_eq!(
            fs::add_folder(FolderMetadata {
                parent: 0,
                name: "fd1".to_string(),
                ..Default::default()
            })
            .unwrap(),
            1
        );

        assert_eq!(
            fs::add_folder(FolderMetadata {
                parent: 0,
                name: "fd2".to_string(),
                ..Default::default()
            })
            .unwrap(),
            2
        );

        assert_eq!(
            fs::list_folders(&ctx, 0, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );

        fs::move_file(f1, 0, 1, 1000).unwrap();
        assert_eq!(
            fs::get_file_ancestors(f1),
            vec![FolderName {
                id: 1,
                name: "fd1".to_string(),
            }]
        );
        assert_eq!(
            fs::list_files(&ctx, 0, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            vec![f2]
        );
        assert_eq!(
            fs::list_files(&ctx, 1, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            vec![f1]
        );

        fs::move_file(f2, 0, 2, 1000).unwrap();
        assert_eq!(
            fs::get_file_ancestors(f2),
            vec![FolderName {
                id: 2,
                name: "fd2".to_string(),
            }]
        );
        assert_eq!(
            fs::list_files(&ctx, 0, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            Vec::<u32>::new()
        );
        assert_eq!(
            fs::list_files(&ctx, 2, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            vec![f2]
        );

        fs::move_folder(2, 0, 1, 1000).unwrap();
        assert_eq!(
            fs::get_folder_ancestors(2),
            vec![FolderName {
                id: 1,
                name: "fd1".to_string(),
            }]
        );
        assert_eq!(
            fs::get_file_ancestors(f2),
            vec![
                FolderName {
                    id: 2,
                    name: "fd2".to_string(),
                },
                FolderName {
                    id: 1,
                    name: "fd1".to_string(),
                }
            ]
        );

        assert_eq!(
            fs::batch_delete_subfiles(0, BTreeSet::from([f1, f2]), 999).unwrap(),
            Vec::<u32>::new()
        );

        fs::move_file(f1, 1, 0, 1000).unwrap();
        fs::move_file(f2, 2, 0, 1000).unwrap();
        assert_eq!(
            fs::batch_delete_subfiles(0, BTreeSet::from([f2, f1]), 999).unwrap(),
            vec![f1, f2]
        );
        assert!(fs::delete_folder(1, 999, |_| Ok(())).is_err());
        assert!(fs::delete_folder(2, 999, |_| Ok(())).unwrap());
        assert!(fs::delete_folder(1, 999, |_| Ok(())).unwrap());
        assert!(fs::delete_folder(0, 999, |_| Ok(())).is_err());

        assert_eq!(FOLDERS.with(|r| r.borrow().len()), 1);
        assert_eq!(HASHS.with(|r| r.borrow().len()), 0);
        assert_eq!(FS_METADATA_STORE.with(|r| r.borrow().len()), 0);
        assert_eq!(FS_CHUNKS_STORE.with(|r| r.borrow().len()), 0);
    }

    #[test]
    fn test_folders_tree_depth() {
        let mut tree = FoldersTree::new();
        tree.add_folder(
            FolderMetadata {
                parent: 0,
                name: "fd1".to_string(),
                ..Default::default()
            },
            1,
            10,
            1000,
        )
        .unwrap();
        tree.add_folder(
            FolderMetadata {
                parent: 1,
                name: "fd2".to_string(),
                ..Default::default()
            },
            2,
            10,
            1000,
        )
        .unwrap();
        tree.add_folder(
            FolderMetadata {
                parent: 2,
                name: "fd3".to_string(),
                ..Default::default()
            },
            3,
            10,
            1000,
        )
        .unwrap();
        assert_eq!(tree.depth(0), 0);
        assert_eq!(tree.depth(1), 1);
        assert_eq!(tree.depth(3), 3);
        assert_eq!(tree.depth(99), 0);

        assert_eq!(tree.depth_or_is_ancestor(2, 0), (2, true));
        assert_eq!(tree.depth_or_is_ancestor(2, 1), (1, true));
        assert_eq!(tree.depth_or_is_ancestor(2, 2), (0, true));
        assert_eq!(tree.depth_or_is_ancestor(2, 3), (2, false));
        assert_eq!(tree.depth_or_is_ancestor(99, 0), (0, true));
        assert_eq!(tree.depth_or_is_ancestor(99, 1), (0, false));

        assert_eq!(tree.ancestors(0), Vec::<FolderName>::new());
        assert_eq!(
            tree.ancestors(1),
            vec![FolderName {
                id: 1,
                name: "fd1".to_string(),
            }]
        );
        assert_eq!(
            tree.ancestors(2),
            vec![
                FolderName {
                    id: 2,
                    name: "fd2".to_string(),
                },
                FolderName {
                    id: 1,
                    name: "fd1".to_string(),
                }
            ]
        );
        assert_eq!(tree.ancestors(99), Vec::<FolderName>::new());
    }

    #[test]
    fn test_folders_tree_list_folders() {
        let mut tree = FoldersTree::new();
        tree.add_folder(
            FolderMetadata {
                parent: 0,
                name: "fd1".to_string(),
                ..Default::default()
            },
            1,
            10,
            1000,
        )
        .unwrap();
        tree.add_folder(
            FolderMetadata {
                parent: 1,
                name: "fd2".to_string(),
                ..Default::default()
            },
            2,
            10,
            1000,
        )
        .unwrap();
        tree.add_folder(
            FolderMetadata {
                parent: 1,
                name: "fd3".to_string(),
                ..Default::default()
            },
            3,
            10,
            1000,
        )
        .unwrap();

        let ctx = Context {
            caller: Principal::anonymous(),
            ps: Policies::default(),
            role: Role::Manager,
        };

        assert_eq!(
            tree.list_folders(&ctx, 0, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            vec![1]
        );
        assert_eq!(
            tree.list_folders(&ctx, 1, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            vec![3, 2]
        );
        assert_eq!(
            tree.list_folders(&ctx, 99, 999, 999)
                .into_iter()
                .map(|v| v.id)
                .collect::<Vec<_>>(),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn test_folders_tree_add_folder() {
        let mut tree = FoldersTree::new();
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 1,
                    name: "fd1".to_string(),
                    ..Default::default()
                },
                1,
                10,
                1000,
            )
            .err()
            .unwrap()
            .contains("parent folder not found"));
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 0,
                    name: "fd1".to_string(),
                    ..Default::default()
                },
                1,
                1,
                1,
            )
            .is_ok());
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 0,
                    name: "fd2".to_string(),
                    ..Default::default()
                },
                1,
                1,
                1,
            )
            .err()
            .unwrap()
            .contains("folder id already exists"));
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 1,
                    name: "fd2".to_string(),
                    ..Default::default()
                },
                2,
                1,
                1,
            )
            .err()
            .unwrap()
            .contains("folder depth exceeds limit"));
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 0,
                    name: "fd2".to_string(),
                    ..Default::default()
                },
                2,
                1,
                1,
            )
            .is_ok());
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 1,
                    name: "fd2".to_string(),
                    ..Default::default()
                },
                3,
                2,
                1,
            )
            .is_ok());
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 1,
                    name: "fd2".to_string(),
                    ..Default::default()
                },
                4,
                2,
                1,
            )
            .err()
            .unwrap()
            .contains("children exceeds limit"));
        tree.get_mut(&0).unwrap().status = 1;
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 0,
                    name: "fd2".to_string(),
                    ..Default::default()
                },
                4,
                1,
                2,
            )
            .err()
            .unwrap()
            .contains("parent folder is not writable"));
        tree.get_mut(&0).unwrap().status = 0;
        assert!(tree
            .add_folder(
                FolderMetadata {
                    parent: 0,
                    name: "fd2".to_string(),
                    ..Default::default()
                },
                4,
                1,
                2,
            )
            .is_ok());
    }

    #[test]
    fn test_folders_tree_parent_to_add_file() {
        let mut tree = FoldersTree::new();
        assert!(tree
            .parent_to_add_file(1, 2)
            .err()
            .unwrap()
            .contains("parent folder not found"));
        tree.get_mut(&0).unwrap().status = 1;
        assert!(tree
            .parent_to_add_file(0, 2)
            .err()
            .unwrap()
            .contains("parent folder is not writable"));
        tree.get_mut(&0).unwrap().status = 0;
        assert!(tree.parent_to_add_file(0, 2).is_ok());
    }

    #[test]
    fn test_folders_tree_move_folder() {
        let mut tree = FoldersTree::new();
        assert!(tree
            .check_moving_folder(0, 1, 2, 10, 100)
            .err()
            .unwrap()
            .contains("root folder cannot be moved"));
        assert!(tree
            .check_moving_folder(1, 2, 2, 10, 100)
            .err()
            .unwrap()
            .contains("target parent folder should not be 2"));
        assert!(tree
            .check_moving_folder(1, 0, 2, 10, 100)
            .err()
            .unwrap()
            .contains("folder not found"));
        tree.add_folder(
            FolderMetadata {
                parent: 0,
                name: "fd1".to_string(),
                ..Default::default()
            },
            1,
            10,
            100,
        )
        .unwrap();
        assert!(tree
            .check_moving_folder(1, 2, 0, 10, 100)
            .err()
            .unwrap()
            .contains("is not in folder"));
        tree.get_mut(&1).unwrap().status = 1;
        assert!(tree
            .check_moving_folder(1, 0, 2, 10, 100)
            .err()
            .unwrap()
            .contains("is not writable"));

        tree.get_mut(&1).unwrap().status = 0;
        tree.get_mut(&0).unwrap().status = 1;
        assert!(tree
            .check_moving_folder(1, 0, 2, 10, 100)
            .err()
            .unwrap()
            .contains("is not writable"));
        tree.get_mut(&0).unwrap().status = 0;
        assert!(tree
            .check_moving_folder(1, 0, 2, 10, 100)
            .err()
            .unwrap()
            .contains("folder not found"));

        tree.add_folder(
            FolderMetadata {
                parent: 0,
                status: 1,
                name: "fd2".to_string(),
                ..Default::default()
            },
            2,
            10,
            100,
        )
        .unwrap();
        assert!(tree
            .check_moving_folder(1, 0, 2, 10, 100)
            .err()
            .unwrap()
            .contains("is not writable"));
        tree.get_mut(&2).unwrap().status = 0;
        assert!(tree
            .check_moving_folder(1, 0, 2, 10, 0)
            .err()
            .unwrap()
            .contains("children exceeds limit"));
        assert!(tree
            .check_moving_folder(1, 0, 2, 0, 100)
            .err()
            .unwrap()
            .contains("folder depth exceeds limit"));
        assert!(tree.check_moving_folder(1, 0, 2, 10, 100).is_ok());
        assert_eq!(tree.get_mut(&0).unwrap().folders, BTreeSet::from([1, 2]));
        assert_eq!(tree.get_mut(&2).unwrap().folders, BTreeSet::from([]));
        tree.move_folder(1, 0, 2, 999);
        assert_eq!(tree.get_mut(&0).unwrap().folders, BTreeSet::from([2]));
        assert_eq!(tree.get_mut(&2).unwrap().folders, BTreeSet::from([1]));
        assert!(tree
            .check_moving_folder(2, 0, 1, 10, 100)
            .err()
            .unwrap()
            .contains("folder cannot be moved to its sub folder"));
    }

    #[test]
    fn test_folders_tree_move_file() {
        let mut tree = FoldersTree::new();
        assert!(tree
            .check_moving_file(1, 1, 100)
            .err()
            .unwrap()
            .contains("target parent should not be"));
        assert!(tree
            .check_moving_file(1, 0, 100)
            .err()
            .unwrap()
            .contains("folder not found"));
        tree.get_mut(&0).unwrap().status = 1;
        assert!(tree
            .check_moving_file(0, 1, 100)
            .err()
            .unwrap()
            .contains("is not writable"));
        tree.get_mut(&0).unwrap().status = 0;
        assert!(tree
            .check_moving_file(0, 1, 100)
            .err()
            .unwrap()
            .contains("folder not found"));
        tree.add_folder(
            FolderMetadata {
                parent: 0,
                status: 1,
                name: "fd1".to_string(),
                ..Default::default()
            },
            1,
            10,
            100,
        )
        .unwrap();
        assert!(tree
            .check_moving_file(0, 1, 100)
            .err()
            .unwrap()
            .contains("is not writable"));
        tree.get_mut(&1).unwrap().status = 0;
        assert!(tree
            .check_moving_file(0, 1, 0)
            .err()
            .unwrap()
            .contains("children exceeds limit"));
        assert!(tree.check_moving_file(0, 1, 10).is_ok());
        tree.move_file(1, 0, 1, 999);
        assert_eq!(tree.get_mut(&1).unwrap().files, BTreeSet::from([1]));
        tree.move_file(1, 1, 0, 999);
        assert_eq!(tree.get_mut(&0).unwrap().files, BTreeSet::from([1]));
        assert_eq!(tree.get_mut(&1).unwrap().files, BTreeSet::new());
    }

    #[test]
    fn test_folders_delete_folder() {
        let mut tree = FoldersTree::new();
        assert!(tree
            .delete_folder(0, 99)
            .err()
            .unwrap()
            .contains("root folder cannot be deleted"));
        assert!(!tree.delete_folder(1, 99).unwrap());
        tree.add_folder(
            FolderMetadata {
                parent: 0,
                status: 1,
                name: "fd1".to_string(),
                files: BTreeSet::from([1]),
                ..Default::default()
            },
            1,
            10,
            100,
        )
        .unwrap();
        assert!(tree
            .delete_folder(1, 99)
            .err()
            .unwrap()
            .contains("folder is readonly"));
        tree.get_mut(&1).unwrap().status = 0;
        assert!(tree
            .delete_folder(1, 99)
            .err()
            .unwrap()
            .contains("folder is not empty"));
        tree.get_mut(&1).unwrap().files.clear();
        tree.get_mut(&0).unwrap().status = 1;
        assert!(tree
            .delete_folder(1, 99)
            .err()
            .unwrap()
            .contains("parent folder is not writable"));
        tree.get_mut(&0).unwrap().status = 0;
        assert!(tree.delete_folder(1, 99).unwrap());
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.get_mut(&0).unwrap().folders, BTreeSet::new());
        assert_eq!(tree.get_mut(&0).unwrap().updated_at, 99);
    }
}
