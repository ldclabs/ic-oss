use candid::{CandidType, Nat, Principal};
use ciborium::{from_reader, into_writer};
use ic_http_certification::{
    cel::{create_cel_expr, DefaultCelBuilder},
    HttpCertification, HttpCertificationPath, HttpCertificationTree, HttpCertificationTreeEntry,
};
use ic_oss_types::{
    file::{FileChunk, FileInfo, MAX_CHUNK_SIZE, MAX_FILE_SIZE, MAX_FILE_SIZE_PER_CALL},
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
use serde_bytes::ByteBuf;
use std::{borrow::Cow, cell::RefCell, collections::BTreeSet, ops};

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Clone, Default, Deserialize, Serialize)]
pub struct Bucket {
    pub name: String,
    pub file_count: u64,
    pub file_id: u32,
    pub max_file_size: u64,
    pub max_dir_depth: u8,
    pub max_children: u16,
    pub status: i8,     // -1: archived; 0: readable and writable; 1: readonly
    pub visibility: u8, // 0: private; 1: public
    #[serde(default)]
    pub max_memo_size: u16,
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
    pub memo: Option<MapValue>,  // memo for the file
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
            size: Nat::from(self.size),
            filled: Nat::from(self.filled),
            created_at: Nat::from(self.created_at),
            updated_at: Nat::from(self.updated_at),
            chunks: self.chunks,
            status: self.status,
            hash: self.hash,
            memo: self.memo,
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

// directory
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct DirectoryMetadata {
    pub parent: u32, // 0: root
    pub name: String,
    pub ancestors: Vec<u32>,  // parent, [parent's upper layer, ...], root
    pub files: BTreeSet<u32>, // length <= max_children
    pub directories: BTreeSet<u32>, // length <= max_children
    pub created_at: u64,      // unix timestamp in milliseconds
    pub updated_at: u64,      // unix timestamp in milliseconds
    pub status: i8,           // -1: archived; 0: readable and writable; 1: readonly
}

impl Storable for DirectoryMetadata {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode DirectoryMetadata data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode DirectoryMetadata data")
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct RootChildren {
    pub files: BTreeSet<u32>,
    pub directories: BTreeSet<u32>,
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
const ROOT_CHILDREN_MEMORY_ID: MemoryId = MemoryId::new(1);
const DIR_METADATA_MEMORY_ID: MemoryId = MemoryId::new(2);
const FS_METADATA_MEMORY_ID: MemoryId = MemoryId::new(3);
const FS_DATA_MEMORY_ID: MemoryId = MemoryId::new(4);
const HASH_INDEX_MEMORY_ID: MemoryId = MemoryId::new(5);

thread_local! {
    static HTTP_TREE: RefCell<HttpCertificationTree> = RefCell::new(HttpCertificationTree::default());
    static BUCKET_HEAP: RefCell<Bucket> = RefCell::new(Bucket::default());
    static ROOT_CHILDREN_HEAP: RefCell<RootChildren> = RefCell::new(RootChildren::default());

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static BUCKET: RefCell<StableCell<Bucket, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(BUCKET_MEMORY_ID)),
            Bucket::default()
        ).expect("failed to init BUCKET store")
    );

    static ROOT_CHILDREN: RefCell<StableCell<RootChildren, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(ROOT_CHILDREN_MEMORY_ID)),
            RootChildren::default()
        ).expect("failed to init ROOT_CHILDREN store")
    );

    static DIR_METADATA: RefCell<StableBTreeMap<u32, DirectoryMetadata, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(DIR_METADATA_MEMORY_ID)),
        )
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

    static HASH_INDEX: RefCell<StableBTreeMap<[u8; 32], u32, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(HASH_INDEX_MEMORY_ID)),
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

    pub fn max_file_size() -> u64 {
        BUCKET_HEAP.with(|r| r.borrow().max_file_size)
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
        ROOT_CHILDREN.with(|r| {
            let s = r.borrow().get().clone();
            ROOT_CHILDREN_HEAP.with(|h| {
                *h.borrow_mut() = s;
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

        ROOT_CHILDREN_HEAP.with(|h| {
            ROOT_CHILDREN.with(|r| {
                r.borrow_mut()
                    .set(h.borrow().clone())
                    .expect("failed to set ROOT_CHILDREN data");
            });
        });
    }
}

pub mod fs {
    use super::*;

    pub fn get_file_id(hash: &[u8; 32]) -> Option<u32> {
        HASH_INDEX.with(|r| r.borrow().get(hash))
    }

    pub fn get_file(id: u32) -> Option<FileMetadata> {
        FS_METADATA.with(|r| r.borrow().get(&id))
    }

    pub fn list_files(parent: u32, prev: u32, take: u32) -> Vec<FileInfo> {
        if parent != 0 {
            return Vec::new();
        }

        FS_METADATA.with(|r| {
            let m = r.borrow();
            let mut res = Vec::with_capacity(take as usize);
            let mut id = prev.saturating_sub(1);
            while id > 0 {
                if let Some(meta) = m.get(&id) {
                    res.push(meta.into_info(id));
                    if res.len() >= take as usize {
                        break;
                    }
                }
                id = id.saturating_sub(1);
            }
            res
        })
    }

    pub fn add_file(meta: FileMetadata) -> Result<u32, String> {
        state::with_mut(|s| {
            let id = s.file_id.saturating_add(1);
            if id == u32::MAX {
                return Err("file id overflow".to_string());
            }

            if let Some(ref hash) = meta.hash {
                HASH_INDEX.with(|r| {
                    let mut m = r.borrow_mut();
                    if let Some(prev) = m.get(hash) {
                        return Err(format!("file hash conflict, {}", prev));
                    }

                    m.insert(**hash, id);
                    Ok(())
                })?;
            }

            s.file_id = id;
            ROOT_CHILDREN_HEAP.with(|r| r.borrow_mut().files.insert(id));
            FS_METADATA.with(|r| r.borrow_mut().insert(id, meta));
            Ok(id)
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

                    if prev_hash != metadata.hash {
                        HASH_INDEX.with(|r| {
                            let mut hm = r.borrow_mut();
                            if let Some(ref hash) = metadata.hash {
                                if let Some(prev) = hm.get(hash) {
                                    return Err(format!("file hash conflict, {}", prev));
                                }
                                hm.insert(**hash, id);
                            }
                            if let Some(prev_hash) = prev_hash {
                                hm.remove(&prev_hash);
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

        let max = state::max_file_size();
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

    pub fn delete_file(id: u32) -> Result<(), String> {
        let metadata = FS_METADATA.with(|r| {
            let mut m = r.borrow_mut();
            if let Some(metadata) = m.get(&id) {
                if metadata.status > 0 {
                    return Err("file is readonly".to_string());
                }
                m.remove(&id);
                Ok(metadata)
            } else {
                Err(format!("file not found: {}", id))
            }
        })?;

        ROOT_CHILDREN_HEAP.with(|r| r.borrow_mut().files.remove(&id));
        if let Some(hash) = metadata.hash {
            HASH_INDEX.with(|r| r.borrow_mut().remove(&hash));
        }
        FS_DATA.with(|r| {
            for chunk_index in 0..metadata.chunks {
                r.borrow_mut().remove(&FileId(id, chunk_index));
            }
        });
        Ok(())
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
            b.max_dir_depth = 10;
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
