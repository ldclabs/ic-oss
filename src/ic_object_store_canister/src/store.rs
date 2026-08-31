use candid::Principal;
use cbor2::{from_reader, to_writer};
use ic_oss_types::object_store::{Attribute, Error, Result, CHUNK_SIZE, MAX_PAYLOAD_SIZE};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, Storable,
};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    ops,
};

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub name: String,
    pub managers: BTreeSet<Principal>,
    pub auditors: BTreeSet<Principal>,
    pub governance_canister: Option<Principal>,
    pub locations: BTreeMap<String, (u64, i64)>, // path -> (etag, size)
    pub next_etag: u64,
    /// Logical object etag -> immutable data etag. Only aliases are stored.
    #[serde(rename = "da")]
    data_aliases: BTreeMap<u64, u64>,
    /// Data etag -> logical reference count. A missing entry means one reference.
    #[serde(rename = "dr")]
    data_refcounts: BTreeMap<u64, u64>,
    /// Tracks newly-created multipart uploads without rereading their chunk payloads.
    /// Uploads created by an older canister version have no entry and use the
    /// compatibility path in `complete_multipart`.
    #[serde(rename = "mu")]
    multipart_uploads: BTreeMap<u64, MultipartUpload>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct MultipartUpload {
    /// MAX_PARTS is 1024, so 16 words cover every possible part index.
    #[serde(rename = "p")]
    present: [u64; 16],
    /// Only non-full chunks need their length recorded.
    #[serde(rename = "s")]
    short_parts: BTreeMap<u32, u32>,
}

impl MultipartUpload {
    fn record(&mut self, part_idx: u32, size: u32) {
        let (word, bit) = ((part_idx / 64) as usize, part_idx % 64);
        self.present[word] |= 1 << bit;
        if size == CHUNK_SIZE as u32 {
            self.short_parts.remove(&part_idx);
        } else {
            self.short_parts.insert(part_idx, size);
        }
    }

    fn completed_size(&self, path: &str, parts: u32) -> Result<u64> {
        if parts == 0 {
            return Ok(0);
        }

        for idx in 0..parts {
            let (word, bit) = ((idx / 64) as usize, idx % 64);
            if self.present[word] & (1 << bit) == 0 {
                return Err(Error::Precondition {
                    path: path.to_string(),
                    error: format!("missing part {idx}"),
                });
            }
        }

        if let Some((&idx, &size)) = self.short_parts.range(..parts - 1).next() {
            return Err(Error::Precondition {
                path: path.to_string(),
                error: format!("invalid part size {size} at {idx}"),
            });
        }

        let last_idx = parts - 1;
        let last_size = self
            .short_parts
            .get(&last_idx)
            .copied()
            .unwrap_or(CHUNK_SIZE as u32);
        if last_size == 0 {
            return Err(Error::Precondition {
                path: path.to_string(),
                error: format!("invalid part size 0 at {last_idx}"),
            });
        }

        Ok((parts as u64 - 1) * CHUNK_SIZE + last_size as u64)
    }
}

/// The metadata that describes an object.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ObjectMetadata {
    /// The last modified time, unix timestamp in milliseconds
    #[serde(rename = "m")]
    last_modified: u64,
    #[serde(rename = "s")]
    size: u64,
    #[serde(rename = "t")]
    tags: String,
    #[serde(rename = "a")]
    attributes: BTreeMap<Attribute, String>,
    #[serde(rename = "v")]
    version: Option<String>,
    #[serde(rename = "an")]
    aes_nonce: Option<ByteArray<12>>,
    #[serde(rename = "at")]
    aes_tags: Option<Vec<ByteArray<16>>>,
}

impl Storable for ObjectMetadata {
    const BOUND: Bound = Bound::Unbounded;

    fn into_bytes(self) -> Vec<u8> {
        let mut buf = vec![];
        to_writer(&self, &mut buf).expect("failed to encode ObjectMetadata data");
        buf
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut buf = vec![];
        to_writer(self, &mut buf).expect("failed to encode ObjectMetadata data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode ObjectMetadata data")
    }
}

// FileId: (object id, chunk id)
// a object is a collection of chunks.
#[derive(Clone, Debug, Default, Deserialize, Serialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct ObjectId(pub u64, pub u32);
impl Storable for ObjectId {
    const BOUND: Bound = Bound::Bounded {
        max_size: 15,
        is_fixed_size: false,
    };

    fn into_bytes(self) -> Vec<u8> {
        encode_object_id(self.0, self.1)
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Owned(encode_object_id(self.0, self.1))
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let bytes = bytes.as_ref();
        assert_eq!(bytes.first(), Some(&0x82), "invalid ObjectId CBOR array");
        let (etag, etag_len) = decode_cbor_uint(&bytes[1..]);
        let (part_idx, part_idx_len) = decode_cbor_uint(&bytes[1 + etag_len..]);
        assert_eq!(bytes.len(), 1 + etag_len + part_idx_len);
        Self(
            etag,
            u32::try_from(part_idx).expect("ObjectId part index exceeds u32"),
        )
    }
}

fn encode_object_id(etag: u64, part_idx: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(15);
    bytes.push(0x82); // CBOR array(2), matching the legacy serde encoding.
    encode_cbor_uint(&mut bytes, etag);
    encode_cbor_uint(&mut bytes, part_idx as u64);
    bytes
}

fn encode_cbor_uint(bytes: &mut Vec<u8>, value: u64) {
    match value {
        0..=23 => bytes.push(value as u8),
        24..=255 => bytes.extend_from_slice(&[0x18, value as u8]),
        256..=65_535 => {
            bytes.push(0x19);
            bytes.extend_from_slice(&(value as u16).to_be_bytes());
        }
        65_536..=4_294_967_295 => {
            bytes.push(0x1a);
            bytes.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            bytes.push(0x1b);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
}

fn decode_cbor_uint(bytes: &[u8]) -> (u64, usize) {
    match bytes.first().copied().expect("missing ObjectId integer") {
        value @ 0..=23 => (value as u64, 1),
        0x18 => (bytes[1] as u64, 2),
        0x19 => (
            u16::from_be_bytes(bytes[1..3].try_into().unwrap()) as u64,
            3,
        ),
        0x1a => (
            u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as u64,
            5,
        ),
        0x1b => (u64::from_be_bytes(bytes[1..9].try_into().unwrap()), 9),
        value => panic!("invalid ObjectId CBOR integer header {value:#x}"),
    }
}

#[derive(Clone, Default)]
pub struct Chunk(pub Vec<u8>);

impl Storable for Chunk {
    const BOUND: Bound = Bound::Bounded {
        max_size: CHUNK_SIZE as u32,
        is_fixed_size: false,
    };

    fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&self.0)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        #[cfg(test)]
        CHUNK_DECODES.with(|count| count.set(count.get() + 1));
        Self(bytes.to_vec())
    }
}

const STATE_MEMORY_ID: MemoryId = MemoryId::new(0);
const OBJECT_META_MEMORY_ID: MemoryId = MemoryId::new(1);
const OBJECT_DATA_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static STATE_STORE: RefCell<StableCell<Vec<u8>, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(STATE_MEMORY_ID)),
            Vec::new()
        )
    );

    static OBJECT_META: RefCell<StableBTreeMap<u64, ObjectMetadata, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(OBJECT_META_MEMORY_ID)),
        )
    );

    static OBJECT_DATA: RefCell<StableBTreeMap<ObjectId, Chunk, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(OBJECT_DATA_MEMORY_ID)),
        )
    );
}

#[cfg(test)]
thread_local! {
    static CHUNK_DECODES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

pub mod state {
    use super::*;

    pub fn with<R>(f: impl FnOnce(&State) -> R) -> R {
        STATE.with_borrow(f)
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut State) -> R) -> R {
        STATE.with_borrow_mut(f)
    }

    pub fn is_controller(caller: &Principal) -> bool {
        STATE.with_borrow(|s| s.governance_canister.as_ref() == Some(caller))
    }

    pub fn is_writer(caller: &Principal) -> bool {
        STATE.with_borrow(|s| s.managers.contains(caller))
    }

    pub fn is_reader(caller: &Principal) -> bool {
        STATE.with_borrow(|s| s.managers.contains(caller) || s.auditors.contains(caller))
    }

    pub fn load() {
        STATE_STORE.with_borrow(|r| {
            STATE.with_borrow_mut(|h| {
                let v: State =
                    from_reader(&r.get()[..]).expect("failed to decode STATE_STORE data");
                *h = v;
            });
        });
    }

    pub fn save() {
        STATE.with_borrow(|h| {
            STATE_STORE.with_borrow_mut(|r| {
                let mut buf = vec![];
                to_writer(h, &mut buf).expect("failed to encode STATE_STORE data");
                r.set(buf);
            });
        });
    }

    pub fn clear() {
        OBJECT_META.with_borrow_mut(|om| om.clear_new());
        OBJECT_DATA.with_borrow_mut(|od| od.clear_new());
        STATE.with_borrow_mut(|s| {
            s.locations.clear();
            s.next_etag = 0;
            s.data_aliases.clear();
            s.data_refcounts.clear();
            s.multipart_uploads.clear();
        });
        save();
    }
}

pub mod object {
    use super::*;
    use ic_oss_types::object_store::*;

    /// Number of chunks stored for a location, for both encodings of `size`:
    /// a completed object stores `size` bytes in `size / CHUNK_SIZE` chunks,
    /// while an upload in flight encodes its highest part index as
    /// `-2 - part_idx` (or -1 when no part has been uploaded yet).
    fn chunks_count(size: i64) -> u64 {
        if size < 0 {
            (-1 - size) as u64
        } else {
            (size as u64).div_ceil(CHUNK_SIZE)
        }
    }

    fn next_etag(state: &mut State) -> Result<u64> {
        let etag = state.next_etag;
        state.next_etag = etag.checked_add(1).ok_or_else(|| Error::Generic {
            error: "object etag space exhausted".to_string(),
        })?;
        Ok(etag)
    }

    pub(super) fn etag_matches(value: &str, etag: u64) -> bool {
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return false;
        }
        value.parse::<u64>() == Ok(etag)
    }

    fn data_etag(state: &State, etag: u64) -> u64 {
        state.data_aliases.get(&etag).copied().unwrap_or(etag)
    }

    fn retain_object_data(state: &mut State, etag: u64) -> u64 {
        let data_etag = data_etag(state, etag);
        let refs = state.data_refcounts.get(&data_etag).copied().unwrap_or(1);
        let refs = refs
            .checked_add(1)
            .expect("object data reference count exhausted");
        state.data_refcounts.insert(data_etag, refs);
        data_etag
    }

    fn remove_object(state: &mut State, etag: u64, size: i64) {
        let data_etag = state.data_aliases.remove(&etag).unwrap_or(etag);
        state.multipart_uploads.remove(&etag);
        OBJECT_META.with_borrow_mut(|metadata| metadata.remove(&etag));

        let remaining = state.data_refcounts.get_mut(&data_etag).map(|refs| {
            debug_assert!(*refs > 1);
            *refs -= 1;
            *refs
        });
        match remaining {
            Some(1) => {
                state.data_refcounts.remove(&data_etag);
            }
            Some(_) => {}
            None => delete_object_data(data_etag, size),
        }
    }

    fn put_object_data(etag: u64, payload: ByteBuf) {
        OBJECT_DATA.with_borrow_mut(|od| {
            let payload = payload.into_vec();
            if payload.len() <= CHUNK_SIZE as usize {
                if !payload.is_empty() {
                    od.insert(ObjectId(etag, 0), Chunk(payload));
                }
                return;
            }

            for (idx, chunk) in payload.chunks(CHUNK_SIZE as usize).enumerate() {
                od.insert(ObjectId(etag, idx as u32), Chunk(chunk.to_owned()));
            }
        });
    }

    fn get_object_ranges(etag: u64, ranges: &[(u64, u64)]) -> Result<Vec<ByteBuf>> {
        OBJECT_DATA.with_borrow(|od| {
            let mut result = Vec::with_capacity(ranges.len());
            let mut chunk_cache: Option<(u32, Chunk)> = None; // cache the last chunk read

            for &(start, end) in ranges {
                let mut buf = Vec::with_capacity((end - start) as usize);

                // Calculate the chunk indices we need to read
                let start_chunk = (start / CHUNK_SIZE) as u32;
                let end_chunk = ((end - 1) / CHUNK_SIZE) as u32;

                for idx in start_chunk..=end_chunk {
                    // Calculate the byte range within this chunk
                    let chunk_start = if idx == start_chunk {
                        (start % CHUNK_SIZE) as usize
                    } else {
                        0
                    };

                    let chunk_end = if idx == end_chunk {
                        ((end - 1) % CHUNK_SIZE + 1) as usize
                    } else {
                        CHUNK_SIZE as usize
                    };

                    match &chunk_cache {
                        Some((cached_idx, cached_chunk)) if *cached_idx == idx => {
                            buf.extend_from_slice(&cached_chunk.0[chunk_start..chunk_end]);
                        }
                        _ => {
                            let chunk =
                                od.get(&ObjectId(etag, idx)).ok_or(Error::Precondition {
                                    path: "".to_string(),
                                    error: format!("missing part {} at {}", idx, etag),
                                })?;
                            buf.extend_from_slice(&chunk.0[chunk_start..chunk_end]);
                            chunk_cache = Some((idx, chunk));
                        }
                    }
                }

                result.push(ByteBuf::from(buf));
            }

            Ok(result)
        })
    }

    fn delete_object_data(etag: u64, size: i64) {
        OBJECT_DATA.with_borrow_mut(|od| {
            for idx in 0..chunks_count(size) {
                od.remove(&ObjectId(etag, idx as u32));
            }
        });
    }

    fn legacy_multipart_size(etag: u64, parts: u32, path: &str) -> Result<u64> {
        OBJECT_DATA.with_borrow(|data| {
            let mut size = 0u64;
            for idx in 0..parts {
                let chunk = data
                    .get(&ObjectId(etag, idx))
                    .ok_or_else(|| Error::Precondition {
                        path: path.to_string(),
                        error: format!("missing part {idx}"),
                    })?;
                if chunk.0.is_empty() || (idx + 1 != parts && chunk.0.len() != CHUNK_SIZE as usize)
                {
                    return Err(Error::Precondition {
                        path: path.to_string(),
                        error: format!("invalid part size {} at {idx}", chunk.0.len()),
                    });
                }
                size += chunk.0.len() as u64;
            }
            Ok(size)
        })
    }

    pub fn put_opts(
        path: String,
        payload: ByteBuf,
        opts: PutOptions,
        now_ms: u64,
    ) -> Result<PutResult> {
        STATE.with_borrow_mut(|s| {
            let mut meta = ObjectMetadata {
                last_modified: now_ms,
                size: payload.len() as u64,
                tags: opts.tags,
                attributes: opts.attributes,
                aes_nonce: opts.aes_nonce,
                aes_tags: opts.aes_tags,
                ..Default::default()
            };

            if let Some(tags) = &meta.aes_tags {
                let parts = payload.len().div_ceil(CHUNK_SIZE as usize);
                if tags.len() != parts {
                    return Err(Error::Precondition {
                        path,
                        error: format!(
                            "aes_tags size {} does not match parts {}",
                            tags.len(),
                            parts
                        ),
                    });
                }
            }

            let (etag, version) = match opts.mode {
                PutMode::Overwrite => {
                    let etag = next_etag(s)?;
                    let prev = s.locations.insert(path, (etag, meta.size as i64));
                    if let Some((prev_etag, prev_size)) = prev {
                        remove_object(s, prev_etag, prev_size);
                    }
                    OBJECT_META.with_borrow_mut(|om| om.insert(etag, meta));
                    put_object_data(etag, payload);
                    (etag, None)
                }
                PutMode::Create => {
                    if s.locations.contains_key(&path) {
                        return Err(Error::AlreadyExists { path });
                    }

                    let etag = next_etag(s)?;
                    s.locations.insert(path, (etag, meta.size as i64));
                    OBJECT_META.with_borrow_mut(|om| om.insert(etag, meta));
                    put_object_data(etag, payload);
                    (etag, None)
                }
                PutMode::Update(v) => match s.locations.get(&path) {
                    None => Err(Error::Precondition {
                        path,
                        error: "NotFound: object not found".into(),
                    })?,
                    Some((etag, size)) => {
                        let prev_etag = *etag;
                        let prev_size = *size;
                        if prev_size < 0 {
                            return Err(Error::Precondition {
                                path,
                                error: "upload not completed".to_string(),
                            });
                        }
                        let expected = v.e_tag.ok_or(Error::Generic {
                            error: "e_tag required for conditional update".to_string(),
                        })?;
                        if !etag_matches(&expected, prev_etag) {
                            return Err(Error::Precondition {
                                path,
                                error: format!("{prev_etag} does not match {expected}"),
                            });
                        }

                        let etag = next_etag(s)?;
                        s.locations.insert(path, (etag, meta.size as i64));
                        meta.version = v.version.clone();
                        OBJECT_META.with_borrow_mut(|om| om.insert(etag, meta));
                        remove_object(s, prev_etag, prev_size);
                        put_object_data(etag, payload);
                        (etag, v.version)
                    }
                },
            };

            Ok(PutResult {
                e_tag: Some(etag.to_string()),
                version,
            })
        })
    }

    pub fn delete(path: String) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            if let Some((etag, size)) = s.locations.remove(&path) {
                remove_object(s, etag, size);
            }
            Ok(())
        })
    }

    pub fn copy(from: String, to: String) -> Result<()> {
        copy_impl(from, to, true)
    }

    pub fn copy_if_not_exists(from: String, to: String) -> Result<()> {
        copy_impl(from, to, false)
    }

    fn copy_impl(from: String, to: String, overwrite: bool) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            if from == to {
                return Err(Error::Precondition {
                    path: from,
                    error: "location 'to' is equal to 'from'".to_string(),
                });
            }

            let (source_etag, size) = {
                let (etag, size) = s
                    .locations
                    .get(&from)
                    .ok_or(Error::NotFound { path: from.clone() })?;
                if *size < 0 {
                    return Err(Error::Precondition {
                        path: from,
                        error: "upload not completed".to_string(),
                    });
                }
                (*etag, *size)
            };

            if !overwrite && s.locations.contains_key(&to) {
                return Err(Error::AlreadyExists { path: to });
            }

            let metadata = OBJECT_META.with_borrow(|om| {
                om.get(&source_etag)
                    .expect("completed object is missing metadata")
            });
            let etag = next_etag(s)?;
            let data_etag = retain_object_data(s, source_etag);

            if let Some((previous_etag, previous_size)) = s.locations.insert(to, (etag, size)) {
                remove_object(s, previous_etag, previous_size);
            }
            s.data_aliases.insert(etag, data_etag);
            OBJECT_META.with_borrow_mut(|om| om.insert(etag, metadata));
            Ok(())
        })
    }

    pub fn rename(from: String, to: String) -> Result<()> {
        rename_impl(from, to, true)
    }

    pub fn rename_if_not_exists(from: String, to: String) -> Result<()> {
        rename_impl(from, to, false)
    }

    fn rename_impl(from: String, to: String, overwrite: bool) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            if from == to {
                return Err(Error::Precondition {
                    path: from,
                    error: "location 'to' is equal to 'from'".to_string(),
                });
            }

            let source = s
                .locations
                .get(&from)
                .copied()
                .ok_or(Error::NotFound { path: from.clone() })?;
            if source.1 < 0 {
                return Err(Error::Precondition {
                    path: from,
                    error: "upload not completed".to_string(),
                });
            }

            if !overwrite && s.locations.contains_key(&to) {
                return Err(Error::AlreadyExists { path: to });
            }

            s.locations.remove(&from);
            if let Some((previous_etag, previous_size)) = s.locations.insert(to, source) {
                remove_object(s, previous_etag, previous_size);
            }
            Ok(())
        })
    }

    pub fn create_multipart(path: String) -> Result<MultipartId> {
        STATE.with_borrow_mut(|s| {
            let etag = next_etag(s)?;
            if let Some((prev_etag, prev_size)) = s.locations.insert(path, (etag, -1)) {
                remove_object(s, prev_etag, prev_size);
            }
            s.multipart_uploads.insert(etag, MultipartUpload::default());
            Ok(etag.to_string())
        })
    }

    pub fn put_part(
        path: String,
        id: MultipartId,
        part_idx: u32,
        payload: ByteBuf,
    ) -> Result<PartId> {
        if part_idx >= MAX_PARTS as u32 {
            return Err(Error::Precondition {
                path,
                error: format!("part index {part_idx} exceeds max index {}", MAX_PARTS - 1),
            });
        }
        if payload.is_empty() || payload.len() > CHUNK_SIZE as usize {
            return Err(Error::Precondition {
                path,
                error: format!(
                    "part size {} is outside the allowed range 1..={CHUNK_SIZE}",
                    payload.len()
                ),
            });
        }

        STATE.with_borrow_mut(|s| {
            let (etag, size) = s
                .locations
                .get_mut(&path)
                .ok_or(Error::NotFound { path: path.clone() })?;
            if !etag_matches(&id, *etag) {
                return Err(Error::Precondition {
                    path,
                    error: "NotFound: upload not found".to_string(),
                });
            }
            if *size >= 0 {
                return Err(Error::Precondition {
                    path,
                    error: "upload already completed".to_string(),
                });
            }
            let iparts = -2 - part_idx as i64;
            if *size > iparts {
                // record the parts number
                *size = iparts;
            }
            let etag = *etag;

            if let Some(upload) = s.multipart_uploads.get_mut(&etag) {
                upload.record(part_idx, payload.len() as u32);
            }
            OBJECT_DATA.with_borrow_mut(|od| {
                od.insert(ObjectId(etag, part_idx), Chunk(payload.into_vec()));
            });

            Ok(PartId {
                content_id: format!("{id}-{part_idx}"),
            })
        })
    }

    pub fn complete_multipart(
        path: String,
        id: MultipartId,
        opts: PutMultipartOptions,
        now_ms: u64,
    ) -> Result<PutResult> {
        STATE.with_borrow_mut(|s| {
            let (etag, parts) = {
                let (etag, size) = s
                    .locations
                    .get(&path)
                    .ok_or(Error::NotFound { path: path.clone() })?;
                if !etag_matches(&id, *etag) {
                    return Err(Error::Precondition {
                        path,
                        error: "NotFound: upload not found".to_string(),
                    });
                }
                if *size >= 0 {
                    return Err(Error::Precondition {
                        path,
                        error: "upload already completed".to_string(),
                    });
                }

                (*etag, (-1 - *size) as u32)
            };

            if let Some(tags) = &opts.aes_tags {
                if tags.len() as u32 != parts {
                    return Err(Error::Precondition {
                        path,
                        error: format!(
                            "aes_tags size {} does not match parts {}",
                            tags.len(),
                            parts
                        ),
                    });
                }
            }

            let size = match s.multipart_uploads.get(&etag) {
                Some(upload) => upload.completed_size(&path, parts)?,
                None => legacy_multipart_size(etag, parts, &path)?,
            };

            OBJECT_META.with_borrow_mut(|om| {
                om.insert(
                    etag,
                    ObjectMetadata {
                        last_modified: now_ms,
                        size,
                        tags: opts.tags,
                        attributes: opts.attributes,
                        aes_nonce: opts.aes_nonce,
                        aes_tags: opts.aes_tags,
                        version: None,
                    },
                )
            });
            s.locations.insert(path, (etag, size as i64));
            s.multipart_uploads.remove(&etag);

            Ok(PutResult {
                e_tag: Some(etag.to_string()),
                version: None,
            })
        })
    }

    pub fn abort_multipart(path: String, id: MultipartId) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            let (etag, size) = {
                let (etag, size) = s
                    .locations
                    .get(&path)
                    .ok_or(Error::NotFound { path: path.clone() })?;
                if !etag_matches(&id, *etag) {
                    return Err(Error::Precondition {
                        path,
                        error: "NotFound: upload not found".to_string(),
                    });
                }
                if *size >= 0 {
                    return Err(Error::Precondition {
                        path,
                        error: "upload already completed".to_string(),
                    });
                }
                (*etag, *size)
            };

            s.locations.remove(&path);
            remove_object(s, etag, size);

            Ok(())
        })
    }

    pub fn get_part(path: String, part_idx: u32) -> Result<ByteBuf> {
        STATE.with_borrow(|s| {
            let (etag, size) = s
                .locations
                .get(&path)
                .ok_or(Error::NotFound { path: path.clone() })?;

            if *size < 0 {
                return Err(Error::Precondition {
                    path,
                    error: "upload not completed".to_string(),
                });
            }

            if *size == 0 && part_idx == 0 {
                return Ok(ByteBuf::new());
            }

            let data_etag = data_etag(s, *etag);
            OBJECT_DATA.with_borrow(|od| {
                let chunk = od
                    .get(&ObjectId(data_etag, part_idx))
                    .ok_or(Error::Precondition {
                        path: "".to_string(),
                        error: format!("missing part {part_idx} at {data_etag}"),
                    })?;
                Ok(ByteBuf::from(chunk.0))
            })
        })
    }

    pub fn get_opts(path: String, opts: GetOptions) -> Result<GetResult> {
        STATE.with_borrow(|s| {
            let (etag, size) = s
                .locations
                .get(&path)
                .ok_or(Error::NotFound { path: path.clone() })?;

            if *size < 0 {
                return Err(Error::Precondition {
                    path,
                    error: "upload not completed".to_string(),
                });
            }

            let me = OBJECT_META.with_borrow(|om| om.get(etag).unwrap());
            let meta = ObjectMeta {
                location: path.clone(),
                last_modified: me.last_modified,
                size: me.size,
                e_tag: Some(etag.to_string()),
                version: me.version,
                aes_nonce: me.aes_nonce,
                aes_tags: me.aes_tags,
            };
            // should check preconditions before returning head
            opts.check_preconditions(&meta)?;
            if opts.head {
                return Ok(GetResult {
                    range: (0, 0),
                    meta,
                    attributes: me.attributes,
                    payload: ByteBuf::new(),
                });
            }

            let r = match opts.range {
                Some(range) => range
                    .into_range(me.size)
                    .map_err(|error| Error::Precondition {
                        path: path.clone(),
                        error,
                    })?,
                None => 0..me.size,
            };

            let range = (r.start, r.end.min(r.start + MAX_PAYLOAD_SIZE));
            let payload = if range.1 == range.0 {
                ByteBuf::new()
            } else {
                get_object_ranges(data_etag(s, *etag), &[range])?
                    .pop()
                    .unwrap()
            };
            Ok(GetResult {
                range,
                meta,
                attributes: me.attributes,
                payload,
            })
        })
    }

    pub fn get_ranges(path: String, ranges: Vec<(u64, u64)>) -> Result<Vec<ByteBuf>> {
        STATE.with_borrow(|s| {
            let (etag, size) = s
                .locations
                .get(&path)
                .ok_or(Error::NotFound { path: path.clone() })?;
            if *size < 0 {
                return Err(Error::Precondition {
                    path,
                    error: "upload not completed".to_string(),
                });
            }

            let size = *size as usize;
            let mut total = 0;
            for &(start, end) in &ranges {
                let start = start as usize;
                let end = end as usize;
                if start >= end || end > size {
                    return Err(Error::Precondition {
                        path: path.clone(),
                        error: format!("invalid range ({start}, {end})"),
                    });
                }
                total += end - start;
            }

            if total > MAX_PAYLOAD_SIZE as usize {
                return Err(Error::Precondition {
                    path,
                    error: "payload size exceeds max size".to_string(),
                });
            }

            get_object_ranges(data_etag(s, *etag), &ranges)
        })
    }

    pub fn head(path: String) -> Result<ObjectMeta> {
        STATE.with_borrow(|s| {
            let (etag, size) = s
                .locations
                .get(&path)
                .ok_or(Error::NotFound { path: path.clone() })?;
            if *size < 0 {
                // upload not completed
                return Err(Error::NotFound { path });
            }

            let me = OBJECT_META.with_borrow(|om| om.get(etag).unwrap());
            Ok(to_object_meta(path, *etag, me))
        })
    }

    const MAX_LIST_LIMIT: usize = 1000;

    fn to_object_meta(location: String, etag: u64, metadata: ObjectMetadata) -> ObjectMeta {
        ObjectMeta {
            location,
            last_modified: metadata.last_modified,
            size: metadata.size,
            e_tag: Some(etag.to_string()),
            version: metadata.version,
            aes_nonce: metadata.aes_nonce,
            aes_tags: metadata.aes_tags,
        }
    }

    fn descendant_start(prefix: &str) -> String {
        if prefix.is_empty() {
            String::new()
        } else {
            let mut start = String::with_capacity(prefix.len() + 1);
            start.push_str(prefix);
            start.push('/');
            start
        }
    }

    fn canonical_path(path: &str) -> &str {
        let path = path.strip_prefix('/').unwrap_or(path);
        path.strip_suffix('/').unwrap_or(path)
    }

    /// `BTreeMap::range` bounds that borrow `start`, so scanning `locations`
    /// never has to clone the key it starts from.
    fn range_from(start: &str) -> (ops::Bound<&str>, ops::Bound<&str>) {
        (ops::Bound::Included(start), ops::Bound::Unbounded)
    }

    pub fn list(prefix: String) -> Result<Vec<ObjectMeta>> {
        STATE.with_borrow(|s| {
            OBJECT_META.with_borrow(|om| {
                let start = descendant_start(&prefix);
                let mut objects = vec![];
                for (path, (etag, size)) in s.locations.range::<str, _>(range_from(&start)) {
                    if !path.starts_with(&start) {
                        break;
                    }
                    if canonical_path(path).len() <= prefix.len() || *size < 0 {
                        continue;
                    }

                    let metadata = om.get(etag).unwrap();
                    objects.push(to_object_meta(path.clone(), *etag, metadata));
                    if objects.len() >= MAX_LIST_LIMIT {
                        break;
                    }
                }
                Ok(objects)
            })
        })
    }

    pub fn list_with_offset(prefix: String, offset: String) -> Result<Vec<ObjectMeta>> {
        STATE.with_borrow(|s| {
            OBJECT_META.with_borrow(|om| {
                let prefix_start = descendant_start(&prefix);
                // Old versions accepted and stored a leading slash. Such keys sort
                // before a canonical offset, so retain the compatibility scan only
                // when one is actually present at the root.
                let has_leading_slash = prefix.is_empty()
                    && s.locations
                        .range::<str, _>(range_from("/"))
                        .next()
                        .is_some_and(|(path, _)| path.starts_with('/'));
                let start: &str = if !has_leading_slash && offset > prefix_start {
                    &offset
                } else {
                    &prefix_start
                };
                let mut objects = vec![];
                for (path, (etag, size)) in s.locations.range::<str, _>(range_from(start)) {
                    if !path.starts_with(&prefix_start) {
                        break;
                    }
                    let canonical_path = canonical_path(path);
                    if canonical_path.len() <= prefix.len()
                        || canonical_path <= offset.as_str()
                        || *size < 0
                    {
                        continue;
                    }

                    let metadata = om.get(etag).unwrap();
                    objects.push(to_object_meta(path.clone(), *etag, metadata));
                    if objects.len() >= MAX_LIST_LIMIT {
                        break;
                    }
                }
                Ok(objects)
            })
        })
    }

    pub fn list_with_delimiter(prefix: String) -> Result<ListResult> {
        STATE.with_borrow(|s| {
            OBJECT_META.with_borrow(|om| {
                let start = descendant_start(&prefix);
                let mut common_prefixes: BTreeSet<String> = BTreeSet::new();

                // Only objects in this base level should be returned in the
                // response. Otherwise, we just collect the common prefixes.
                let mut objects = vec![];
                for (path, (etag, size)) in s.locations.range::<str, _>(range_from(&start)) {
                    if !path.starts_with(&start) {
                        break;
                    }
                    let canonical_path = canonical_path(path);
                    if canonical_path.len() <= prefix.len() || *size < 0 {
                        continue;
                    }

                    let relative = &canonical_path[start.len()..];
                    if let Some(separator) = relative.find('/') {
                        common_prefixes
                            .insert(canonical_path[..start.len() + separator].to_string());
                    } else {
                        let metadata = om.get(etag).unwrap();
                        objects.push(to_object_meta(path.clone(), *etag, metadata));
                    }

                    if objects.len() >= MAX_LIST_LIMIT || common_prefixes.len() >= MAX_LIST_LIMIT {
                        break;
                    }
                }

                Ok(ListResult {
                    objects,
                    common_prefixes: common_prefixes.into_iter().collect(),
                })
            })
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ic_oss_types::{object_store::*, sha3_256};
    use object_store::path::Path;

    #[test]
    fn test_bound_max_size() {
        let etags = [
            0,
            23,
            24,
            255,
            256,
            65_535,
            65_536,
            u32::MAX as u64,
            u32::MAX as u64 + 1,
            u64::MAX,
        ];
        let part_indices = [0, 23, 24, 255, 256, 65_535, 65_536, u32::MAX];

        for etag in etags {
            for part_idx in part_indices {
                let object_id = ObjectId(etag, part_idx);
                let mut legacy_bytes = Vec::new();
                to_writer(&object_id, &mut legacy_bytes).unwrap();
                assert_eq!(object_id.to_bytes().as_ref(), legacy_bytes);
                assert_eq!(ObjectId::from_bytes(Cow::Owned(legacy_bytes)), object_id);
            }
        }

        assert_eq!(ObjectId(u64::MAX, u32::MAX).to_bytes().len(), 15);
        assert_eq!(ObjectId(0, 0).to_bytes().len(), 3);

        assert!(object::etag_matches("0", 0));
        assert!(object::etag_matches(&u64::MAX.to_string(), u64::MAX));
        for noncanonical in ["", "00", "01", "+1", " 1", "1 "] {
            assert!(!object::etag_matches(noncanonical, 1));
        }
    }

    #[test]
    fn test_legacy_state_decode() {
        #[derive(Serialize)]
        struct LegacyState {
            name: String,
            managers: BTreeSet<Principal>,
            auditors: BTreeSet<Principal>,
            governance_canister: Option<Principal>,
            locations: BTreeMap<String, (u64, i64)>,
            next_etag: u64,
        }

        let legacy = LegacyState {
            name: "legacy".to_string(),
            managers: BTreeSet::new(),
            auditors: BTreeSet::new(),
            governance_canister: None,
            locations: BTreeMap::from([("a.txt".to_string(), (7, 3))]),
            next_etag: 8,
        };
        let mut bytes = Vec::new();
        to_writer(&legacy, &mut bytes).unwrap();
        let state: State = from_reader(bytes.as_slice()).unwrap();
        assert_eq!(state.name, "legacy");
        assert_eq!(state.locations["a.txt"], (7, 3));
        assert!(state.data_aliases.is_empty());
        assert!(state.data_refcounts.is_empty());
        assert!(state.multipart_uploads.is_empty());
    }

    #[test]
    fn test_copy_shares_and_reclaims_object_data() {
        let source = "shared/source.bin".to_string();
        let first_copy = "shared/first.bin".to_string();
        let second_copy = "shared/second.bin".to_string();
        let payload = ByteBuf::from(vec![9; CHUNK_SIZE as usize + 17]);

        object::put_opts(source.clone(), payload.clone(), PutOptions::default(), 0).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|data| data.len()), 2);

        object::copy(source.clone(), first_copy.clone()).unwrap();
        object::copy(first_copy.clone(), second_copy.clone()).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|data| data.len()), 2);
        STATE.with_borrow(|state| {
            assert_eq!(state.data_aliases.get(&1), Some(&0));
            assert_eq!(state.data_aliases.get(&2), Some(&0));
            assert_eq!(state.data_refcounts.get(&0), Some(&3));
        });

        state::save();
        STATE.with_borrow_mut(|state| *state = State::default());
        state::load();
        STATE.with_borrow(|state| {
            assert_eq!(state.data_aliases.get(&1), Some(&0));
            assert_eq!(state.data_aliases.get(&2), Some(&0));
            assert_eq!(state.data_refcounts.get(&0), Some(&3));
        });

        object::delete(source).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|data| data.len()), 2);
        assert_eq!(
            object::get_opts(second_copy.clone(), GetOptions::default())
                .unwrap()
                .payload,
            payload
        );

        object::put_opts(
            first_copy.clone(),
            ByteBuf::from("replacement"),
            PutOptions::default(),
            0,
        )
        .unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|data| data.len()), 3);
        STATE.with_borrow(|state| assert!(!state.data_refcounts.contains_key(&0)));

        object::delete(second_copy).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|data| data.len()), 1);
        object::delete(first_copy).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|data| data.len()), 0);
    }

    #[test]
    fn test_multipart_completion_avoids_chunk_decoding() {
        let path = "tracked/multipart.bin".to_string();
        let id = object::create_multipart(path.clone()).unwrap();
        object::put_part(
            path.clone(),
            id.clone(),
            0,
            ByteBuf::from(vec![1; CHUNK_SIZE as usize]),
        )
        .unwrap();
        object::put_part(path.clone(), id.clone(), 1, ByteBuf::from(vec![2; 37])).unwrap();

        CHUNK_DECODES.with(|count| count.set(0));
        object::complete_multipart(path, id, PutMultipartOptions::default(), 0).unwrap();
        CHUNK_DECODES.with(|count| assert_eq!(count.get(), 0));

        let legacy_path = "legacy/multipart.bin".to_string();
        let legacy_id = object::create_multipart(legacy_path.clone()).unwrap();
        object::put_part(
            legacy_path.clone(),
            legacy_id.clone(),
            0,
            ByteBuf::from(vec![3; 11]),
        )
        .unwrap();
        let legacy_etag = legacy_id.parse::<u64>().unwrap();
        STATE.with_borrow_mut(|state| {
            state.multipart_uploads.remove(&legacy_etag);
        });

        CHUNK_DECODES.with(|count| count.set(0));
        object::complete_multipart(legacy_path, legacy_id, PutMultipartOptions::default(), 0)
            .unwrap();
        CHUNK_DECODES.with(|count| assert_eq!(count.get(), 1));
    }

    #[test]
    fn test_objects() {
        // Test basic put/get
        let path = "test/a.txt".to_string();
        let payload = ByteBuf::from("hello world");
        let opts = PutOptions {
            mode: PutMode::Create,
            ..Default::default()
        };

        // Put object
        let res = object::put_opts(path.clone(), payload.clone(), opts.clone(), 0).unwrap();
        assert_eq!(res.e_tag, Some("0".to_string()));

        // Get object
        let res = object::get_opts(path.clone(), GetOptions::default()).unwrap();
        assert_eq!(res.payload, payload);

        // Test head
        let meta = object::head(path.clone()).unwrap();
        assert_eq!(meta.size as usize, payload.len());
        assert_eq!(meta.e_tag, Some("0".to_string()));

        // Test create again
        assert!(object::put_opts(path.clone(), payload.clone(), opts, 0).is_err());

        // Test overwrite
        let payload = ByteBuf::from("hello Anda");
        let res = object::put_opts(
            path.clone(),
            payload.clone(),
            PutOptions {
                mode: PutMode::Overwrite,
                ..Default::default()
            },
            0,
        )
        .unwrap();
        assert_eq!(res.e_tag, Some("1".to_string()));

        let res = object::get_opts(path.clone(), GetOptions::default()).unwrap();
        assert_eq!(res.payload, payload);
        assert_eq!(res.meta.size as usize, payload.len());

        // Test update
        let payload = ByteBuf::from("hello Anda 2");
        let res = object::put_opts(
            path.clone(),
            payload.clone(),
            PutOptions {
                mode: PutMode::Update(UpdateVersion {
                    e_tag: Some("0".to_string()),
                    version: Some("0".to_string()),
                }),
                ..Default::default()
            },
            0,
        );
        assert!(res.is_err());
        let res = object::put_opts(
            path.clone(),
            payload.clone(),
            PutOptions {
                mode: PutMode::Update(UpdateVersion {
                    e_tag: Some("1".to_string()),
                    version: Some("0".to_string()),
                }),
                ..Default::default()
            },
            0,
        )
        .unwrap();
        assert_eq!(res.e_tag, Some("2".to_string()));
        assert_eq!(res.version, Some("0".to_string()));
        let res = object::get_opts(path.clone(), GetOptions::default()).unwrap();
        assert_eq!(res.payload, payload);
        assert_eq!(res.meta.location, path);
        assert_eq!(res.meta.e_tag, Some("2".to_string()));
        assert_eq!(res.meta.size as usize, payload.len());
        assert_eq!(res.meta.version, Some("0".to_string()));

        // Test copy
        let to = "test/b.txt".to_string();
        let res = object::copy(to.clone(), path.clone());
        assert!(res.is_err());
        object::copy(path.clone(), to.clone()).unwrap();
        let res = object::copy_if_not_exists(path.clone(), to.clone());
        assert!(res.is_err());

        // Test delete
        object::delete(path.clone()).unwrap();
        assert!(object::get_opts(path.clone(), GetOptions::default()).is_err());

        let res = object::get_opts(to.clone(), GetOptions::default()).unwrap();
        assert_eq!(res.payload, payload);
        assert_eq!(res.meta.location, to);
        assert_eq!(res.meta.e_tag, Some("3".to_string()));
        assert_eq!(res.meta.size as usize, payload.len());
        assert_eq!(res.meta.version, Some("0".to_string()));

        object::copy_if_not_exists(to.clone(), path.clone()).unwrap();
        let res = object::get_opts(path.clone(), GetOptions::default()).unwrap();
        assert_eq!(res.payload, payload);
        assert_eq!(res.meta.location, path);
        assert_eq!(res.meta.e_tag, Some("4".to_string()));
        assert_eq!(res.meta.size as usize, payload.len());
        assert_eq!(res.meta.version, Some("0".to_string()));

        // Test rename
        let rename = "test/c.txt".to_string();
        object::rename(to.clone(), rename.clone()).unwrap();
        assert!(object::get_opts(to.clone(), GetOptions::default()).is_err());
        assert!(object::rename(to.clone(), rename.clone()).is_err());
        let res = object::get_opts(rename.clone(), GetOptions::default()).unwrap();
        assert_eq!(res.payload, payload);
        assert_eq!(res.meta.location, rename);
        assert_eq!(res.meta.e_tag, Some("3".to_string()));
        assert_eq!(res.meta.size as usize, payload.len());
        assert_eq!(res.meta.version, Some("0".to_string()));

        assert!(object::rename_if_not_exists(path.clone(), rename.clone()).is_err());
        let rename = "test/d.txt".to_string();
        object::rename_if_not_exists(path.clone(), rename.clone()).unwrap();
        assert!(object::get_opts(path.clone(), GetOptions::default()).is_err());
        let res = object::get_opts(rename.clone(), GetOptions::default()).unwrap();
        assert_eq!(res.payload, payload);
        assert_eq!(res.meta.location, rename);
        assert_eq!(res.meta.e_tag, Some("4".to_string()));
        assert_eq!(res.meta.size as usize, payload.len());
        assert_eq!(res.meta.version, Some("0".to_string()));

        // Test rename with overwrite
        let path = "test/c.txt".to_string();
        object::rename(path.clone(), rename.clone()).unwrap();
        assert!(object::get_opts(path.clone(), GetOptions::default()).is_err());
        let res = object::get_opts(rename.clone(), GetOptions::default()).unwrap();
        assert_eq!(res.payload, payload);
        assert_eq!(res.meta.location, rename);
        assert_eq!(res.meta.e_tag, Some("3".to_string()));
        assert_eq!(res.meta.size as usize, payload.len());
        assert_eq!(res.meta.version, Some("0".to_string()));
    }

    #[test]
    fn test_list() {
        let paths = vec![
            "a/1.txt".to_string(),
            "a/1.txt/1.txt".to_string(),
            "aa/1.txt".to_string(),
            "b/1.txt".to_string(),
            "a/2.txt".to_string(),
            "b/2.txt".to_string(),
            "a/3.txt".to_string(),
            Path::from_iter(["a", "b/c", "foo.file"]).to_string(),
        ];
        let mut pahts_sorted = paths.clone();
        pahts_sorted.sort();
        assert_ne!(&paths, &pahts_sorted);
        let opts = PutOptions {
            mode: PutMode::Create,
            ..Default::default()
        };
        for path in paths.iter() {
            object::put_opts(
                path.clone(),
                ByteBuf::from(path.as_bytes()),
                opts.clone(),
                0,
            )
            .unwrap();
        }
        let res = object::list(String::new()).unwrap();
        let list_paths: Vec<String> = res.iter().map(|x| x.location.clone()).collect();
        assert_eq!(list_paths, pahts_sorted);

        let res = object::list("a".to_string()).unwrap();
        let list_paths: Vec<String> = res.iter().map(|x| x.location.clone()).collect();
        assert_eq!(
            list_paths,
            vec![
                "a/1.txt".to_string(),
                "a/1.txt/1.txt".to_string(),
                "a/2.txt".to_string(),
                "a/3.txt".to_string(),
                "a/b%2Fc/foo.file".to_string()
            ]
        );

        let res = object::list("a/1".to_string()).unwrap();
        assert!(res.is_empty());
        let res = object::list("a/1.txt".to_string()).unwrap();
        let list_paths: Vec<String> = res.iter().map(|x| x.location.clone()).collect();
        assert_eq!(list_paths, vec!["a/1.txt/1.txt".to_string()]);

        let res = object::list_with_offset("a".to_string(), "a/1.txt/1.txt".to_string()).unwrap();
        let list_paths: Vec<String> = res.iter().map(|x| x.location.clone()).collect();
        assert_eq!(
            list_paths,
            vec![
                "a/2.txt".to_string(),
                "a/3.txt".to_string(),
                "a/b%2Fc/foo.file".to_string()
            ]
        );

        let res = object::list_with_delimiter(String::new()).unwrap();
        assert_eq!(
            res.common_prefixes,
            vec!["a".to_string(), "aa".to_string(), "b".to_string()]
        );
        assert!(res.objects.is_empty());

        let res = object::list_with_delimiter("a".to_string()).unwrap();
        assert_eq!(
            res.common_prefixes,
            vec!["a/1.txt".to_string(), "a/b%2Fc".to_string()]
        );
        let list_paths: Vec<String> = res.objects.iter().map(|x| x.location.clone()).collect();
        assert_eq!(
            list_paths,
            vec![
                "a/1.txt".to_string(),
                "a/2.txt".to_string(),
                "a/3.txt".to_string()
            ]
        );
    }

    #[test]
    fn test_noncanonical_listing_compatibility() {
        for path in ["/leading.txt", "trailing/"] {
            object::put_opts(
                path.to_string(),
                ByteBuf::from(path),
                PutOptions::default(),
                0,
            )
            .unwrap();
        }

        let expected = vec!["/leading.txt".to_string(), "trailing/".to_string()];
        let objects = object::list(String::new()).unwrap();
        assert_eq!(
            objects
                .into_iter()
                .map(|object| object.location)
                .collect::<Vec<_>>(),
            expected
        );

        let result = object::list_with_delimiter(String::new()).unwrap();
        assert!(result.common_prefixes.is_empty());
        assert_eq!(
            result
                .objects
                .into_iter()
                .map(|object| object.location)
                .collect::<Vec<_>>(),
            expected
        );

        let objects = object::list_with_offset(String::new(), "a".to_string()).unwrap();
        assert_eq!(
            objects
                .into_iter()
                .map(|object| object.location)
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn test_large_objects() {
        // Test basic put/get
        let path = "test/a.bin".to_string();
        let count = 10000u64;
        let len = count * 32;
        let mut payload = Vec::with_capacity(len as usize);
        for i in 0..count {
            payload.extend_from_slice(sha3_256(&i.to_be_bytes()).as_slice());
        }
        assert_eq!(payload.len(), len as usize);

        object::put_opts(
            path.clone(),
            ByteBuf::from(payload.to_vec()),
            PutOptions {
                mode: PutMode::Create,
                ..Default::default()
            },
            0,
        )
        .unwrap();
        let res = object::get_opts(path.clone(), GetOptions::default()).unwrap();
        assert_eq!(&res.payload, &payload);
        assert_eq!(res.meta.location, path);
        assert_eq!(res.meta.size as usize, payload.len());

        let res = object::get_part(path.clone(), 0).unwrap();
        assert_eq!(res, payload[0..CHUNK_SIZE as usize]);
        let res = object::get_part(path.clone(), 1).unwrap();
        assert_eq!(res, payload[CHUNK_SIZE as usize..]);
        assert!(object::get_part(path.clone(), 2).is_err());

        let ranges = vec![(0u64, 1000), (10, 10000), (100, len)];
        let rt = object::get_ranges(path.clone(), ranges.clone()).unwrap();
        assert_eq!(rt.len(), ranges.len());
        for (i, (start, end)) in ranges.into_iter().enumerate() {
            let res = object::get_opts(
                path.clone(),
                GetOptions {
                    range: Some(GetRange::Bounded(start, end)),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(rt[i], &res.payload);
            assert_eq!(&res.payload, &payload[start as usize..end as usize]);
            assert_eq!(res.meta.location, path);
            assert_eq!(res.meta.size as usize, payload.len());
        }

        assert!(object::get_opts(
            path.clone(),
            GetOptions {
                range: Some(GetRange::Bounded(100, 100)),
                ..Default::default()
            }
        )
        .is_err());
        assert!(object::get_opts(
            path.clone(),
            GetOptions {
                range: Some(GetRange::Bounded(len, len + 1)),
                ..Default::default()
            }
        )
        .is_err());
        let res = object::get_opts(
            path.clone(),
            GetOptions {
                range: Some(GetRange::Bounded(1, len + 1)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(&res.payload, &payload[1..]);
    }

    #[test]
    fn test_pending_multipart_is_reclaimed() {
        let path = "test/pending.bin".to_string();
        let part = ByteBuf::from(vec![7u8; CHUNK_SIZE as usize]);

        let empty_id = object::create_multipart(path.clone()).unwrap();
        assert!(object::put_part(path.clone(), empty_id.clone(), 0, ByteBuf::new()).is_err());
        object::complete_multipart(path.clone(), empty_id, PutMultipartOptions::default(), 0)
            .unwrap();
        assert_eq!(object::get_part(path.clone(), 0).unwrap(), ByteBuf::new());
        object::delete(path.clone()).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|od| od.len()), 0);

        // deleting a path with an upload in flight must reclaim its parts
        let id = object::create_multipart(path.clone()).unwrap();
        for idx in 0..4u32 {
            object::put_part(path.clone(), id.clone(), idx, part.clone()).unwrap();
        }
        assert_eq!(OBJECT_DATA.with_borrow(|od| od.len()), 4);
        object::delete(path.clone()).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|od| od.len()), 0);

        // and so must overwriting it
        let id = object::create_multipart(path.clone()).unwrap();
        for idx in 0..3u32 {
            object::put_part(path.clone(), id.clone(), idx, part.clone()).unwrap();
        }
        assert_eq!(OBJECT_DATA.with_borrow(|od| od.len()), 3);
        object::put_opts(
            path.clone(),
            ByteBuf::from("small"),
            PutOptions::default(),
            0,
        )
        .unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|od| od.len()), 1);

        object::delete(path.clone()).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|od| od.len()), 0);

        // an upload in flight is not an object PutMode::Update can act on,
        // even though its id is the etag the check compares against
        let id = object::create_multipart(path.clone()).unwrap();
        object::put_part(path.clone(), id.clone(), 0, part.clone()).unwrap();
        assert!(object::put_opts(
            path.clone(),
            ByteBuf::from("x"),
            PutOptions {
                mode: PutMode::Update(UpdateVersion {
                    e_tag: Some(id.clone()),
                    version: None,
                }),
                ..Default::default()
            },
            0,
        )
        .is_err());

        object::abort_multipart(path.clone(), id).unwrap();
        assert_eq!(OBJECT_DATA.with_borrow(|od| od.len()), 0);
    }

    #[test]
    fn test_multipart() {
        // Test basic put/get
        let path = "test/b.bin".to_string();
        let count = 100000u64;
        let len = count * 32;
        let mut payload = Vec::with_capacity(len as usize);
        for i in 0..count {
            payload.extend_from_slice(sha3_256(&i.to_be_bytes()).as_slice());
        }
        assert_eq!(payload.len(), len as usize);

        let id = object::create_multipart(path.clone()).unwrap();

        let chunks: Vec<&[u8]> = payload.chunks(CHUNK_SIZE as usize).collect();
        for (i, chunk) in chunks.iter().enumerate().skip(1) {
            object::put_part(
                path.clone(),
                id.clone(),
                i as u32,
                ByteBuf::from(chunk.to_vec()),
            )
            .unwrap();
        }

        // not completed
        assert!(object::complete_multipart(
            path.clone(),
            id.clone(),
            PutMultipartOptions::default(),
            0
        )
        .is_err());

        object::put_part(
            path.clone(),
            id.clone(),
            0,
            ByteBuf::from(chunks[0].to_vec()),
        )
        .unwrap();

        object::complete_multipart(path.clone(), id.clone(), PutMultipartOptions::default(), 0)
            .unwrap();

        let ranges = vec![(0u64, 1000), (100, 100000), (len - CHUNK_SIZE * 2, len)];
        let rt = object::get_ranges(path.clone(), ranges.clone()).unwrap();
        assert_eq!(rt.len(), ranges.len());
        for (i, (start, end)) in ranges.into_iter().enumerate() {
            let res = object::get_opts(
                path.clone(),
                GetOptions {
                    range: Some(GetRange::Bounded(start, end)),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(rt[i], &res.payload);
            assert_eq!(&res.payload, &payload[start as usize..end as usize]);
            assert_eq!(res.meta.location, path);
            assert_eq!(res.meta.size as usize, payload.len());
        }
    }
}
