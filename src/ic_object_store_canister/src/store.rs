use candid::Principal;
use ciborium::{from_reader, into_writer};
use ic_oss_types::object_store::{Attribute, CHUNK_SIZE, MAX_PAYLOAD_SIZE};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, Storable,
};
use object_store::path::Path;
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
};

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct State {
    pub name: String,
    pub managers: BTreeSet<Principal>,
    pub auditors: BTreeSet<Principal>,
    pub governance_canister: Option<Principal>,
    pub locations: BTreeMap<String, (u64, i64)>, // path -> (etag, size)
    pub next_etag: u64,
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
        into_writer(&self, &mut buf).expect("failed to encode ObjectMetadata data");
        buf
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode ObjectMetadata data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode ObjectMetadata data")
    }
}

// FileId: (object id, chunk id)
// a object is a collection of chunks.
#[derive(Clone, Default, Deserialize, Serialize, Ord, PartialOrd, Eq, PartialEq)]
pub struct ObjectId(pub u64, pub u32);
impl Storable for ObjectId {
    const BOUND: Bound = Bound::Bounded {
        max_size: 15,
        is_fixed_size: false,
    };

    fn into_bytes(self) -> Vec<u8> {
        let mut buf = vec![];
        into_writer(&self, &mut buf).expect("failed to encode ObjectId data");
        buf
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode ObjectId data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode ObjectId data")
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
        Self(bytes.to_vec())
    }
}

const STATE_MEMORY_ID: MemoryId = MemoryId::new(0);
const OBJECT_META_MEMORY_ID: MemoryId = MemoryId::new(1);
const OBJECT_DATA_MEMORY_ID: MemoryId = MemoryId::new(2);

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static MULTIPART_UPLOAD : RefCell<HashMap<u64, Vec<Option<ByteBuf>>>> = RefCell::new(HashMap::new());

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
                into_writer(h, &mut buf).expect("failed to encode STATE_STORE data");
                r.set(buf);
            });
        });
    }

    pub fn clear() {
        MULTIPART_UPLOAD.with_borrow_mut(|mu| mu.clear());
        OBJECT_META.with_borrow_mut(|om| om.clear_new());
        OBJECT_DATA.with_borrow_mut(|od| od.clear_new());
        STATE.with_borrow_mut(|s| {
            s.locations.clear();
            s.next_etag = 0;
        });
        save();
    }
}

pub mod object {
    use super::*;
    use ic_oss_types::object_store::*;

    fn put_object_data(etag: u64, payload: ByteBuf, prev: Option<(u64, i64)>) {
        OBJECT_DATA.with_borrow_mut(|od| {
            let payload = payload.into_vec();
            if let Some((etag, size)) = prev {
                let size = size.max(0) as u64;
                for idx in 0..size.div_ceil(CHUNK_SIZE) {
                    od.remove(&ObjectId(etag, idx as u32));
                }
            }

            for (idx, chunk) in payload.chunks(CHUNK_SIZE as usize).enumerate() {
                od.insert(ObjectId(etag, idx as u32), Chunk(chunk.to_owned()));
            }
        });
    }

    fn copy_object_data(from: u64, to: u64, size: usize) {
        OBJECT_DATA.with_borrow_mut(|od| {
            for idx in 0..size.div_ceil(CHUNK_SIZE as usize) {
                if let Some(chunk) = od.get(&ObjectId(from, idx as u32)) {
                    od.insert(ObjectId(to, idx as u32), chunk);
                }
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
            let size = size.max(0) as u64;
            for idx in 0..size.div_ceil(CHUNK_SIZE) {
                od.remove(&ObjectId(etag, idx as u32));
            }
        });
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
                    let etag = s.next_etag;
                    s.next_etag += 1;
                    let prev = s.locations.insert(path, (etag, meta.size as i64));

                    OBJECT_META.with_borrow_mut(|om| {
                        if let Some((prev_etag, _)) = prev {
                            om.remove(&prev_etag);
                        }
                        om.insert(etag, meta);
                    });

                    put_object_data(etag, payload, prev);
                    (etag, None)
                }
                PutMode::Create => {
                    if s.locations.contains_key(&path) {
                        return Err(Error::AlreadyExists { path });
                    }

                    let etag = s.next_etag;
                    s.locations.insert(path, (etag, meta.size as i64));
                    s.next_etag += 1;
                    OBJECT_META.with_borrow_mut(|om| om.insert(etag, meta));
                    put_object_data(etag, payload, None);
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
                        let expected = v.e_tag.ok_or(Error::Generic {
                            error: "e_tag required for conditional update".to_string(),
                        })?;
                        if prev_etag.to_string() != expected {
                            return Err(Error::Precondition {
                                path,
                                error: format!("{prev_etag} does not match {expected}"),
                            });
                        }

                        let etag = s.next_etag;
                        s.next_etag += 1;
                        s.locations.insert(path, (etag, meta.size as i64));
                        meta.version = v.version.clone();
                        OBJECT_META.with_borrow_mut(|om| om.insert(etag, meta));
                        put_object_data(etag, payload, Some((prev_etag, prev_size)));
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
                OBJECT_META.with_borrow_mut(|om| om.remove(&etag));
                delete_object_data(etag, size);
            }
            Ok(())
        })
    }

    pub fn copy(from: String, to: String) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            let (from, size) = {
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

            let etag = s.next_etag;
            s.next_etag += 1;
            if let Some((prev_etag, prev_size)) = s.locations.insert(to, (etag, size)) {
                // delete the existing 'to' object data
                OBJECT_META.with_borrow_mut(|om| om.remove(&prev_etag));
                delete_object_data(prev_etag, prev_size);
            }

            OBJECT_META.with_borrow_mut(|om| om.insert(etag, om.get(&from).unwrap()));
            copy_object_data(from, etag, size as usize);
            Ok(())
        })
    }

    pub fn copy_if_not_exists(from: String, to: String) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            let (from, size) = {
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
            // check if the destination already exists after checking the source
            if s.locations.contains_key(&to) {
                return Err(Error::AlreadyExists { path: to });
            }

            let etag = s.next_etag;
            s.next_etag += 1;
            s.locations.insert(to, (etag, size));

            OBJECT_META.with_borrow_mut(|om| om.insert(etag, om.get(&from).unwrap()));
            copy_object_data(from, etag, size as usize);
            Ok(())
        })
    }

    pub fn rename(from: String, to: String) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            {
                let (_, size) = s
                    .locations
                    .get(&from)
                    .ok_or(Error::NotFound { path: from.clone() })?;
                if *size < 0 {
                    return Err(Error::Precondition {
                        path: from,
                        error: "upload not completed".to_string(),
                    });
                }
            };

            let (from, size) = s.locations.remove(&from).unwrap();
            if let Some((prev_etag, prev_size)) = s.locations.insert(to, (from, size)) {
                // delete the existing 'to' object data
                OBJECT_META.with_borrow_mut(|om| om.remove(&prev_etag));
                delete_object_data(prev_etag, prev_size);
            }
            Ok(())
        })
    }

    pub fn rename_if_not_exists(from: String, to: String) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            {
                let (_, size) = s
                    .locations
                    .get(&from)
                    .ok_or(Error::NotFound { path: from.clone() })?;
                if *size < 0 {
                    return Err(Error::Precondition {
                        path: from,
                        error: "upload not completed".to_string(),
                    });
                }
            };
            // check if the destination already exists after checking the source
            if s.locations.contains_key(&to) {
                return Err(Error::AlreadyExists { path: to });
            }

            let (etag, size) = s.locations.remove(&from).unwrap();
            s.locations.insert(to, (etag, size));
            Ok(())
        })
    }

    pub fn create_multipart(path: String) -> Result<MultipartId> {
        STATE.with_borrow_mut(|s| {
            // allow overwrite existing object
            // if s.locations.contains_key(&path) {
            //     return Err(Error::AlreadyExists { path });
            // }

            let etag = s.next_etag;
            s.next_etag += 1;
            if let Some((prev_etag, prev_size)) = s.locations.insert(path, (etag, -1)) {
                OBJECT_META.with_borrow_mut(|om| {
                    om.remove(&prev_etag);
                });
                delete_object_data(prev_etag, prev_size);
            }
            Ok(etag.to_string())
        })
    }

    pub fn put_part(
        path: String,
        id: MultipartId,
        part_idx: u32,
        payload: ByteBuf,
    ) -> Result<PartId> {
        STATE.with_borrow_mut(|s| {
            let (etag, size) = s
                .locations
                .get_mut(&path)
                .ok_or(Error::NotFound { path: path.clone() })?;
            if etag.to_string() != id {
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

            OBJECT_DATA.with_borrow_mut(|od| {
                od.insert(ObjectId(*etag, part_idx), Chunk(payload.into_vec()));
            });

            Ok(PartId {
                content_id: format!("{}-{}", id, part_idx),
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
                if etag.to_string() != id {
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

            OBJECT_DATA.with_borrow_mut(|od| {
                let mut size = 0;
                for idx in 0..parts {
                    if let Some(chunk) = od.get(&ObjectId(etag, idx)) {
                        if idx != parts - 1 && chunk.0.len() != CHUNK_SIZE as usize {
                            return Err(Error::Precondition {
                                path,
                                error: format!("invalid part size {} at {}", chunk.0.len(), idx),
                            });
                        }
                        size += chunk.0.len();
                    } else {
                        return Err(Error::Precondition {
                            path,
                            error: format!("missing part {}", idx),
                        });
                    }
                }

                OBJECT_META.with_borrow_mut(|om| {
                    om.insert(
                        etag,
                        ObjectMetadata {
                            last_modified: now_ms,
                            size: size as u64,
                            tags: opts.tags,
                            attributes: opts.attributes,
                            aes_nonce: opts.aes_nonce,
                            aes_tags: opts.aes_tags,
                            version: None,
                        },
                    )
                });
                s.locations.insert(path, (etag, size as i64));
                Ok(())
            })?;

            Ok(PutResult {
                e_tag: Some(etag.to_string()),
                version: None,
            })
        })
    }

    pub fn abort_multipart(path: String, id: MultipartId) -> Result<()> {
        STATE.with_borrow_mut(|s| {
            let (etag, parts) = {
                let (etag, size) = s
                    .locations
                    .get(&path)
                    .ok_or(Error::NotFound { path: path.clone() })?;
                if etag.to_string() != id {
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

            s.locations.remove(&path);
            OBJECT_META.with_borrow_mut(|om| om.remove(&etag));
            if parts > 0 {
                OBJECT_DATA.with_borrow_mut(|od| {
                    for idx in 0..parts {
                        od.remove(&ObjectId(etag, idx));
                    }
                });
            }

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

            OBJECT_DATA.with_borrow(|od| {
                let chunk = od
                    .get(&ObjectId(*etag, part_idx))
                    .ok_or(Error::Precondition {
                        path: "".to_string(),
                        error: format!("missing part {} at {}", part_idx, etag),
                    })?;
                Ok(ByteBuf::from(chunk.0.clone()))
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
                get_object_ranges(*etag, &[range])?.pop().unwrap()
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

            get_object_ranges(*etag, &ranges)
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
            Ok(ObjectMeta {
                location: path.clone(),
                last_modified: me.last_modified,
                size: me.size,
                e_tag: Some(etag.to_string()),
                version: me.version,
                aes_nonce: me.aes_nonce,
                aes_tags: me.aes_tags,
            })
        })
    }

    const MAX_LIST_LIMIT: usize = 1000;
    pub fn list(prefix: Path) -> Result<Vec<ObjectMeta>> {
        STATE.with_borrow(|s| {
            OBJECT_META.with_borrow(|om| {
                let start = prefix.to_string();
                let mut objects = vec![];
                for (path, (etag, size)) in s.locations.range(start.clone()..) {
                    if !path.starts_with(&start) {
                        break;
                    }
                    if *size >= 0 {
                        let key = Path::parse(path).unwrap();
                        if key
                            .prefix_match(&prefix)
                            .map(|mut x| x.next().is_some())
                            .unwrap_or(false)
                        {
                            let me = om.get(etag).unwrap();
                            objects.push(ObjectMeta {
                                location: path.clone(),
                                last_modified: me.last_modified,
                                size: me.size,
                                e_tag: Some(etag.to_string()),
                                version: me.version,
                                aes_nonce: me.aes_nonce,
                                aes_tags: me.aes_tags,
                            });
                            if objects.len() >= MAX_LIST_LIMIT {
                                break;
                            }
                        }
                    }
                }
                Ok(objects)
            })
        })
    }

    pub fn list_with_offset(prefix: Path, offset: Path) -> Result<Vec<ObjectMeta>> {
        STATE.with_borrow(|s| {
            OBJECT_META.with_borrow(|om| {
                let start = prefix.to_string();
                let offset = offset;
                let mut objects = vec![];
                for (path, (etag, size)) in s.locations.range(start.clone()..) {
                    if !path.starts_with(&start) {
                        break;
                    }

                    if *size >= 0 {
                        let key = Path::parse(path).unwrap();
                        if key
                            .prefix_match(&prefix)
                            .map(|mut x| x.next().is_some())
                            .unwrap_or(false)
                        {
                            if key <= offset {
                                continue;
                            }
                            let me = om.get(etag).unwrap();
                            objects.push(ObjectMeta {
                                location: path.clone(),
                                last_modified: me.last_modified,
                                size: me.size,
                                e_tag: Some(etag.to_string()),
                                version: me.version,
                                aes_nonce: me.aes_nonce,
                                aes_tags: me.aes_tags,
                            });
                            if objects.len() >= MAX_LIST_LIMIT {
                                break;
                            }
                        }
                    }
                }
                Ok(objects)
            })
        })
    }

    pub fn list_with_delimiter(prefix: Path) -> Result<ListResult> {
        STATE.with_borrow(|s| {
            OBJECT_META.with_borrow(|om| {
                let start = prefix.to_string();
                let mut common_prefixes: BTreeSet<String> = BTreeSet::new();

                // Only objects in this base level should be returned in the
                // response. Otherwise, we just collect the common prefixes.
                let mut objects = vec![];
                for (path, (etag, size)) in s.locations.range(start.clone()..) {
                    if !path.starts_with(&start) {
                        break;
                    }

                    if *size >= 0 {
                        let key = Path::parse(path).unwrap();
                        let mut parts = match key.prefix_match(&prefix) {
                            Some(parts) => parts,
                            None => continue,
                        };

                        // Pop first element
                        let common_prefix = match parts.next() {
                            Some(p) => p,
                            // Should only return children of the prefix
                            None => continue,
                        };

                        if parts.next().is_some() {
                            common_prefixes.insert(prefix.child(common_prefix).to_string());
                        } else {
                            let me = om.get(etag).unwrap();
                            objects.push(ObjectMeta {
                                location: path.clone(),
                                last_modified: me.last_modified,
                                size: me.size,
                                e_tag: Some(etag.to_string()),
                                version: me.version,
                                aes_nonce: me.aes_nonce,
                                aes_tags: me.aes_tags,
                            });
                            if objects.len() >= MAX_LIST_LIMIT {
                                break;
                            }
                        }
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

    #[test]
    fn test_bound_max_size() {
        let v = ObjectId(u64::MAX, u32::MAX);
        let v = v.to_bytes();
        println!("ObjectId max_size: {:?}", v.len());

        let v = ObjectId(0u64, 0u32);
        let v = v.to_bytes();
        println!("ObjectId min_size: {:?}", v.len());
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
        let res = object::list(Path::default()).unwrap();
        let list_paths: Vec<String> = res.iter().map(|x| x.location.clone()).collect();
        assert_eq!(list_paths, pahts_sorted);

        let res = object::list("a".to_string().into()).unwrap();
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

        let res = object::list("a/1".to_string().into()).unwrap();
        assert!(res.is_empty());
        let res = object::list("a/1.txt".to_string().into()).unwrap();
        let list_paths: Vec<String> = res.iter().map(|x| x.location.clone()).collect();
        assert_eq!(list_paths, vec!["a/1.txt/1.txt".to_string()]);

        let res =
            object::list_with_offset("a".to_string().into(), "a/1.txt/1.txt".to_string().into())
                .unwrap();
        let list_paths: Vec<String> = res.iter().map(|x| x.location.clone()).collect();
        assert_eq!(
            list_paths,
            vec![
                "a/2.txt".to_string(),
                "a/3.txt".to_string(),
                "a/b%2Fc/foo.file".to_string()
            ]
        );

        let res = object::list_with_delimiter(Path::default()).unwrap();
        assert_eq!(
            res.common_prefixes,
            vec!["a".to_string(), "aa".to_string(), "b".to_string()]
        );
        assert!(res.objects.is_empty());

        let res = object::list_with_delimiter("a".to_string().into()).unwrap();
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
