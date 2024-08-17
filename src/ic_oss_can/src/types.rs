use candid::Principal;
use ciborium::{from_reader, into_writer};
use ic_oss_types::file::*;
use ic_stable_structures::{storable::Bound, Storable};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteArray;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    ops,
};

pub const MILLISECONDS: u64 = 1_000_000_000;

#[derive(Clone, Deserialize, Serialize)]
pub struct Files {
    pub file_id: u32,
    pub max_file_size: u64,
    pub visibility: u8,                // 0: private; 1: public
    pub managers: BTreeSet<Principal>, // managers can read and write
    pub files: BTreeMap<u32, FileMetadata>,
}

impl Files {
    pub fn list_files(&self, prev: u32, take: u32) -> Vec<FileInfo> {
        let mut res = Vec::with_capacity(take as usize);
        for (file_id, file) in self
            .files
            .range(ops::Range {
                start: 1,
                end: prev,
            })
            .rev()
        {
            res.push(file.clone().into_info(*file_id));
            if res.len() >= take as usize {
                break;
            }
        }
        res
    }
}

impl Default for Files {
    fn default() -> Self {
        Self {
            file_id: 1, // 0 is reserved for the Files data itself
            max_file_size: MAX_FILE_SIZE,
            visibility: 0,
            managers: BTreeSet::new(),
            files: BTreeMap::new(),
        }
    }
}

impl Storable for Files {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode Files data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Files data")
    }
}

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
    pub name: String,
    pub content_type: String, // MIME types
    pub size: u64,
    pub filled: u64,
    pub created_at: u64, // unix timestamp in milliseconds
    pub updated_at: u64, // unix timestamp in milliseconds
    pub chunks: u32,
    pub hash: Option<ByteArray<32>>, // recommend sha3 256
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
            name: self.name,
            content_type: self.content_type,
            size: self.size,
            filled: self.filled,
            created_at: self.created_at,
            updated_at: self.updated_at,
            chunks: self.chunks,
            hash: self.hash,
            ..Default::default()
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
