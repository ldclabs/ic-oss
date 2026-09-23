use aes_gcm::{aes::cipher::consts::U12, AeadInOut, Aes256Gcm, Key, Nonce, Tag};
use async_stream::try_stream;
use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use candid::{
    utils::{encode_args, ArgumentEncoder},
    CandidType, Decode, Principal,
};
use chrono::DateTime;
use futures::{stream::BoxStream, StreamExt};
use ic_agent::Agent;
use ic_cose_types::{BoxError, CanisterCaller};
use ic_oss_types::{format_error, object_store::*};
use object_store::Extensions;
use serde_bytes::{ByteArray, ByteBuf, Bytes};
use std::{collections::BTreeSet, ops::Range, sync::Arc};

pub use object_store::{
    self, path::Path, CopyMode, CopyOptions, DynObjectStore, MultipartUpload, ObjectStore,
    RenameOptions, RenameTargetMode,
};

use crate::rand_bytes;

pub static STORE_NAME: &str = "ICObjectStore";

/// Client for interacting with the IC Object Store canister.
///
/// Handles communication with the canister and optional AES-256 encryption.
///
/// # Fields
/// - `agent`: IC agent for making calls to the canister
/// - `canister`: Principal of the target canister
/// - `cipher`: Optional AES-256-GCM cipher for encryption/decryption
#[derive(Clone)]
pub struct Client {
    agent: Arc<Agent>,
    canister: Principal,
    cipher: Option<Arc<Aes256Gcm>>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:Client({})", STORE_NAME, self.canister)
    }
}

impl Client {
    /// Creates a new Client instance with optional AES-256 encryption
    pub fn new(agent: Arc<Agent>, canister: Principal, aes_secret: Option<[u8; 32]>) -> Client {
        use aes_gcm::KeyInit;

        let cipher = aes_secret.map(|secret| {
            let key = Key::<Aes256Gcm>::from(secret);
            Arc::new(Aes256Gcm::new(&key))
        });

        Client {
            agent,
            canister,
            cipher,
        }
    }
}

impl ObjectStoreSDK for Client {
    fn canister(&self) -> &Principal {
        &self.canister
    }

    fn cipher(&self) -> Option<Arc<Aes256Gcm>> {
        self.cipher.clone()
    }
}

impl CanisterCaller for Client {
    async fn canister_query<
        In: ArgumentEncoder + Send,
        Out: CandidType + for<'a> candid::Deserialize<'a>,
    >(
        &self,
        canister: &Principal,
        method: &str,
        args: In,
    ) -> Result<Out, BoxError> {
        let input = encode_args(args)?;
        let res = self
            .agent
            .query(canister, method)
            .with_arg(input)
            .call()
            .await?;
        let output = Decode!(res.as_slice(), Out)?;
        Ok(output)
    }

    async fn canister_update<
        In: ArgumentEncoder + Send,
        Out: CandidType + for<'a> candid::Deserialize<'a>,
    >(
        &self,
        canister: &Principal,
        method: &str,
        args: In,
    ) -> Result<Out, BoxError> {
        let input = encode_args(args)?;
        let res = self
            .agent
            .update(canister, method)
            .with_arg(input)
            .call_and_wait()
            .await?;
        let output = Decode!(res.as_slice(), Out)?;
        Ok(output)
    }
}

#[async_trait]
pub trait ObjectStoreSDK: CanisterCaller + Sized {
    fn canister(&self) -> &Principal;
    fn cipher(&self) -> Option<Arc<Aes256Gcm>>;

    /// Retrieves the current state of the object store
    async fn get_state(&self) -> Result<StateInfo, String> {
        self.canister_query(self.canister(), "get_state", ())
            .await
            .map_err(format_error)?
    }

    async fn is_member(&self, member_kind: &str, user: &Principal) -> Result<bool, String> {
        self.canister_query(self.canister(), "is_member", (member_kind, user))
            .await
            .map_err(format_error)?
    }

    /// Adds managers to the canister (requires controller privileges)
    async fn admin_add_managers(&self, args: &BTreeSet<Principal>) -> Result<(), String> {
        self.canister_update(self.canister(), "admin_add_managers", (args,))
            .await
            .map_err(format_error)?
    }

    /// Removes managers from the canister (requires controller privileges)
    async fn admin_remove_managers(&self, args: &BTreeSet<Principal>) -> Result<(), String> {
        self.canister_update(self.canister(), "admin_remove_managers", (args,))
            .await
            .map_err(format_error)?
    }

    /// Adds auditors to the canister (requires controller privileges)
    async fn admin_add_auditors(&self, args: &BTreeSet<Principal>) -> Result<(), String> {
        self.canister_update(self.canister(), "admin_add_auditors", (args,))
            .await
            .map_err(format_error)?
    }

    /// Removes auditors from the canister (requires controller privileges)
    async fn admin_remove_auditors(&self, args: &BTreeSet<Principal>) -> Result<(), String> {
        self.canister_update(self.canister(), "admin_remove_auditors", (args,))
            .await
            .map_err(format_error)?
    }

    /// Stores data at specified path with options
    async fn put_opts(&self, path: &Path, payload: &Bytes, opts: PutOptions) -> Result<PutResult> {
        if payload.len() > MAX_PAYLOAD_SIZE as usize {
            return Err(Error::Precondition {
                path: path.as_ref().to_string(),
                error: format!(
                    "payload size {} exceeds max size {}",
                    payload.len(),
                    MAX_PAYLOAD_SIZE
                ),
            });
        }

        self.canister_update(self.canister(), "put_opts", (path.as_ref(), payload, opts))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Deletes data at specified path
    async fn delete(&self, path: &Path) -> Result<()> {
        self.canister_update(self.canister(), "delete", (path.as_ref(),))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Copies data from one path to another
    async fn copy(&self, from: &Path, to: &Path) -> Result<()> {
        self.canister_update(self.canister(), "copy", (from.as_ref(), to.as_ref()))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Copies data only if destination doesn't exist
    async fn copy_if_not_exists(&self, from: &Path, to: &Path) -> Result<()> {
        self.canister_update(
            self.canister(),
            "copy_if_not_exists",
            (from.as_ref(), to.as_ref()),
        )
        .await
        .map_err(|error| Error::Generic {
            error: format_error(error),
        })?
    }

    /// Renames/moves data from one path to another
    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        self.canister_update(self.canister(), "rename", (from.as_ref(), to.as_ref()))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Renames/moves data only if destination doesn't exist
    async fn rename_if_not_exists(&self, from: &Path, to: &Path) -> Result<()> {
        self.canister_update(
            self.canister(),
            "rename_if_not_exists",
            (from.as_ref(), to.as_ref()),
        )
        .await
        .map_err(|error| Error::Generic {
            error: format_error(error),
        })?
    }

    /// Initiates a multipart upload
    async fn create_multipart(&self, path: &Path) -> Result<MultipartId> {
        self.canister_update(self.canister(), "create_multipart", (path.as_ref(),))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Uploads a part in a multipart upload
    async fn put_part(
        &self,
        path: &Path,
        id: &MultipartId,
        part_idx: u64,
        payload: &Bytes,
    ) -> Result<PartId> {
        self.canister_update(
            self.canister(),
            "put_part",
            (path.as_ref(), id, part_idx, payload),
        )
        .await
        .map_err(|error| Error::Generic {
            error: format_error(error),
        })?
    }

    /// Completes a multipart upload
    async fn complete_multipart(
        &self,
        path: &Path,
        id: &MultipartId,
        opts: &PutMultipartOptions,
    ) -> Result<PutResult> {
        self.canister_update(
            self.canister(),
            "complete_multipart",
            (path.as_ref(), id, opts),
        )
        .await
        .map_err(|error| Error::Generic {
            error: format_error(error),
        })?
    }

    /// Aborts a multipart upload
    async fn abort_multipart(&self, path: &Path, id: &MultipartId) -> Result<()> {
        self.canister_update(self.canister(), "abort_multipart", (path.as_ref(), id))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Retrieves a specific part of data
    async fn get_part(&self, path: &Path, part_idx: u64) -> Result<ByteBuf> {
        self.canister_query(self.canister(), "get_part", (path.as_ref(), part_idx))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Retrieves data with options (range, if_match, etc.)
    async fn get_opts(&self, path: &Path, opts: GetOptions) -> Result<GetResult> {
        self.canister_query(self.canister(), "get_opts", (path.as_ref(), opts))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Retrieves multiple ranges of data
    async fn get_ranges(&self, path: &Path, ranges: &[(u64, u64)]) -> Result<Vec<ByteBuf>> {
        if ranges.is_empty() {
            return Ok(Vec::new());
        }

        self.canister_query(self.canister(), "get_ranges", (path.as_ref(), ranges))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Retrieves metadata for a path
    async fn head(&self, path: &Path) -> Result<ObjectMeta> {
        self.canister_query(self.canister(), "head", (path.as_ref(),))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Lists objects under a prefix
    async fn list(&self, prefix: Option<&Path>) -> Result<Vec<ObjectMeta>> {
        self.canister_query(self.canister(), "list", (prefix.map(|p| p.as_ref()),))
            .await
            .map_err(|error| Error::Generic {
                error: format_error(error),
            })?
    }

    /// Lists objects with an offset
    async fn list_with_offset(
        &self,
        prefix: Option<&Path>,
        offset: &Path,
    ) -> Result<Vec<ObjectMeta>> {
        self.canister_query(
            self.canister(),
            "list_with_offset",
            (prefix.map(|p| p.as_ref()), offset.as_ref()),
        )
        .await
        .map_err(|error| Error::Generic {
            error: format_error(error),
        })?
    }

    /// Lists objects with directory delimiter
    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
        self.canister_query(
            self.canister(),
            "list_with_delimiter",
            (prefix.map(|p| p.as_ref()),),
        )
        .await
        .map_err(|error| Error::Generic {
            error: format_error(error),
        })?
    }
}

/// Handles multipart upload operations
#[derive(Debug)]
pub struct MultipartUploader {
    part_idx: u64,
    parts_cache: BytesMut,
    opts: PutMultipartOptions,
    state: Arc<UploadState>,
}

/// Internal state for tracking upload progress
struct UploadState {
    client: Arc<Client>,
    path: Path,
    id: MultipartId,
}

impl std::fmt::Debug for UploadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:UploadState({}, {})", STORE_NAME, self.path, self.id)
    }
}

#[async_trait]
impl MultipartUpload for MultipartUploader {
    /// Adds a part to the upload, buffering until chunk size is reached
    fn put_part(&mut self, payload: object_store::PutPayload) -> object_store::UploadPart {
        let payload = bytes::Bytes::from(payload);
        self.parts_cache.extend_from_slice(&payload);
        if self.parts_cache.len() < CHUNK_SIZE as usize {
            return Box::pin(futures::future::ready(Ok(())));
        }

        let mut parts: Vec<object_store::UploadPart> = Vec::new();
        while self.parts_cache.len() >= CHUNK_SIZE as usize {
            let state = self.state.clone();
            let mut chunk = self.parts_cache.split_to(CHUNK_SIZE as usize);

            if let Some(cipher) = &self.state.client.cipher {
                let nonce = derive_gcm_nonce(
                    self.opts.aes_nonce.as_ref().as_ref().unwrap(),
                    self.part_idx,
                );
                match encrypt_chunk(cipher, &Nonce::from(nonce), &mut chunk, &state.path) {
                    Ok(tag) => {
                        self.opts.aes_tags.as_mut().unwrap().push(tag);
                    }
                    Err(err) => {
                        return Box::pin(futures::future::ready(Err(err)));
                    }
                }
            }

            let part_idx = self.part_idx;
            self.part_idx += 1;
            parts.push(Box::pin(async move {
                let _ = state
                    .client
                    .put_part(&state.path, &state.id, part_idx, Bytes::new(&chunk))
                    .await
                    .map_err(from_error)?;
                Ok(())
            }))
        }

        Box::pin(async move {
            for part in parts {
                part.await?;
            }

            Ok(())
        })
    }

    /// Finalizes the multipart upload and returns result
    async fn complete(&mut self) -> object_store::Result<object_store::PutResult> {
        for part in self.parts_cache.chunks_mut(CHUNK_SIZE as usize) {
            let part_idx = self.part_idx;
            self.part_idx += 1;

            if let Some(cipher) = &self.state.client.cipher {
                let nonce =
                    derive_gcm_nonce(self.opts.aes_nonce.as_ref().as_ref().unwrap(), part_idx);
                match encrypt_chunk(cipher, &Nonce::from(nonce), part, &self.state.path) {
                    Ok(tag) => {
                        self.opts.aes_tags.as_mut().unwrap().push(tag);
                    }
                    Err(err) => {
                        return Err(err);
                    }
                }
            }

            let _ = self
                .state
                .client
                .put_part(&self.state.path, &self.state.id, part_idx, Bytes::new(part))
                .await
                .map_err(from_error)?;
        }

        self.parts_cache.clear();
        let res = self
            .state
            .client
            .complete_multipart(&self.state.path, &self.state.id, &self.opts)
            .await
            .map_err(from_error)?;
        Ok(object_store::PutResult {
            e_tag: res.e_tag,
            version: res.version,
            extensions: Extensions::default(),
        })
    }

    /// Aborts the multipart upload and cleans up resources
    async fn abort(&mut self) -> object_store::Result<()> {
        self.state
            .client
            .abort_multipart(&self.state.path, &self.state.id)
            .await
            .map_err(from_error)
    }
}

/// Main client for interacting with the object store
#[derive(Clone)]
pub struct ObjectStoreClient {
    client: Arc<Client>,
}

impl ObjectStoreClient {
    pub fn new(client: Arc<Client>) -> ObjectStoreClient {
        ObjectStoreClient { client }
    }

    pub async fn get_state(&self) -> Result<StateInfo, String> {
        self.client.get_state().await
    }

    async fn get_opts_inner(
        &self,
        path: &Path,
        opts: object_store::GetOptions,
    ) -> object_store::Result<object_store::GetResult> {
        let options = GetOptions {
            if_match: opts.if_match,
            if_none_match: opts.if_none_match,
            if_modified_since: opts.if_modified_since.map(|v| v.timestamp_millis() as u64),
            if_unmodified_since: opts
                .if_unmodified_since
                .map(|v| v.timestamp_millis() as u64),
            range: opts.range.clone().map(to_get_range),
            version: opts.version,
            head: opts.head,
        };

        let res: GetResult = self
            .client
            .get_opts(path, options)
            .await
            .map_err(from_error)?;

        // 请求的 range
        let rr = if let Some(r) = &opts.range {
            r.as_range(res.meta.size)
                .map_err(|err| object_store::Error::Generic {
                    store: STORE_NAME,
                    source: err.into(),
                })?
        } else {
            0..res.meta.size
        };
        // 第一次请求返回的 range
        let range = res.range.0..res.range.1;
        let meta = from_object_meta(res.meta);
        let attributes: object_store::Attributes = res
            .attributes
            .into_iter()
            .map(|(k, v)| (from_attribute(k), v))
            .collect();
        let data = bytes::Bytes::from(res.payload.into_vec());
        if opts.head || rr == range {
            let stream = futures::stream::once(futures::future::ready(Ok(data)));
            return Ok(object_store::GetResult {
                payload: object_store::GetResultPayload::Stream(stream.boxed()),
                meta,
                range,
                attributes,
                extensions: Extensions::default(),
            });
        }

        let stream = create_get_range_stream(
            self.client.clone(),
            path.clone(),
            rr.clone(),
            range,
            data,
            meta.e_tag
                .clone()
                .ok_or_else(|| invalid_response("missing object ETag"))?,
        );
        Ok(object_store::GetResult {
            payload: object_store::GetResultPayload::Stream(stream),
            meta,
            range: rr,
            attributes,
            extensions: Extensions::default(),
        })
    }
}

impl std::fmt::Display for ObjectStoreClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:ObjectStoreClient", STORE_NAME)
    }
}

impl std::fmt::Debug for ObjectStoreClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:ObjectStoreClient", STORE_NAME)
    }
}

#[async_trait]
impl ObjectStore for ObjectStoreClient {
    /// Uploads an object with options
    async fn put_opts(
        &self,
        path: &Path,
        payload: object_store::PutPayload,
        opts: object_store::PutOptions,
    ) -> object_store::Result<object_store::PutResult> {
        let data = bytes::Bytes::from(payload);
        let mut opts = to_put_options(&opts);
        let payload: Vec<u8> = if let Some(cipher) = &self.client.cipher {
            let base_nonce: [u8; 12] = rand_bytes();
            let mut data: Vec<u8> = data.into();
            let mut aes_tags: Vec<ByteArray<16>> = Vec::new();
            for (i, chunk) in data.chunks_mut(CHUNK_SIZE as usize).enumerate() {
                let nonce = derive_gcm_nonce(&base_nonce, i as u64);
                let tag = encrypt_chunk(cipher, &Nonce::from(nonce), chunk, path)?;
                aes_tags.push(tag);
            }
            opts.aes_nonce = Some(base_nonce.into());
            opts.aes_tags = Some(aes_tags);
            data
        } else {
            data.into()
        };

        let res = self
            .client
            .put_opts(path, Bytes::new(&payload), opts)
            .await
            .map_err(from_error)?;
        Ok(object_store::PutResult {
            e_tag: res.e_tag,
            version: res.version,
            extensions: Extensions::default(),
        })
    }

    /// Initiates a multipart upload with options
    async fn put_multipart_opts(
        &self,
        path: &Path,
        opts: object_store::PutMultipartOptions,
    ) -> object_store::Result<Box<dyn object_store::MultipartUpload>> {
        let upload_id = self
            .client
            .create_multipart(path)
            .await
            .map_err(from_error)?;
        let mut opts = PutMultipartOptions {
            tags: opts.tags.encoded().to_string(),
            attributes: opts
                .attributes
                .iter()
                .map(|(k, v)| (to_attribute(k), v.to_string()))
                .collect(),
            ..Default::default()
        };

        if self.client.cipher.is_some() {
            opts.aes_nonce = Some(rand_bytes().into());
            opts.aes_tags = Some(Vec::new());
        }

        Ok(Box::new(MultipartUploader {
            part_idx: 0,
            parts_cache: BytesMut::new(),
            opts,
            state: Arc::new(UploadState {
                client: self.client.clone(),
                path: path.clone(),
                id: upload_id,
            }),
        }))
    }

    async fn get_opts(
        &self,
        location: &Path,
        mut opts: object_store::GetOptions,
    ) -> object_store::Result<object_store::GetResult> {
        if opts.head {
            return self.get_opts_inner(location, opts).await;
        }
        if let Some(cipher) = self.client.cipher() {
            let meta = self
                .client
                .get_opts(
                    location,
                    GetOptions {
                        head: true,
                        if_match: opts.if_match.clone(),
                        if_none_match: opts.if_none_match.clone(),
                        if_modified_since: opts
                            .if_modified_since
                            .map(|value| value.timestamp_millis() as u64),
                        if_unmodified_since: opts
                            .if_unmodified_since
                            .map(|value| value.timestamp_millis() as u64),
                        version: opts.version.clone(),
                        ..Default::default()
                    },
                )
                .await
                .map_err(from_error)?
                .meta;
            opts.if_match = Some(
                meta.e_tag
                    .clone()
                    .ok_or_else(|| invalid_response("missing object ETag"))?,
            );

            // 原始 range
            let range = if let Some(r) = &opts.range {
                r.as_range(meta.size)
                    .map_err(|err| object_store::Error::Generic {
                        store: STORE_NAME,
                        source: err.into(),
                    })?
            } else {
                0..meta.size
            };

            // 调整 range，确保读取到包含原始 range 的完整的 chunks，用于解密
            let rr = (range.start / CHUNK_SIZE) * CHUNK_SIZE
                ..meta
                    .size
                    .min((1 + range.end.saturating_sub(1) / CHUNK_SIZE) * CHUNK_SIZE);

            if rr.end > rr.start {
                opts.range = Some(object_store::GetRange::Bounded(rr.clone()));
            }

            let res = self.get_opts_inner(location, opts).await?;
            let obj = res.meta.clone();

            let attributes = res.attributes.clone();
            let start_idx = rr.start / CHUNK_SIZE;
            let start_offset = (range.start - rr.start) as usize;
            let size = (range.end - range.start) as usize;

            let aes_tags = meta.aes_tags.ok_or_else(|| object_store::Error::Generic {
                store: STORE_NAME,
                source: format!("missing AES256 tags for path {location}").into(),
            })?;
            let base_nonce = meta.aes_nonce.ok_or_else(|| object_store::Error::Generic {
                store: STORE_NAME,
                source: format!("missing AES256 nonce for path {location}").into(),
            })?;

            let stream = create_decryption_stream(
                res,
                cipher,
                aes_tags,
                *base_nonce,
                location.clone(),
                start_idx as usize,
                start_offset,
                size,
            );

            return Ok(object_store::GetResult {
                payload: object_store::GetResultPayload::Stream(stream),
                meta: obj,
                range,
                attributes,
                extensions: Extensions::default(),
            });
        }

        self.get_opts_inner(location, opts).await
    }

    /// Retrieves multiple byte ranges from an object
    async fn get_ranges(
        &self,
        location: &Path,
        ranges: &[Range<u64>],
    ) -> object_store::Result<Vec<bytes::Bytes>> {
        if ranges.is_empty() {
            return Ok(Vec::new());
        }

        let meta = self.client.head(location).await.map_err(from_error)?;
        ranges_is_valid(ranges, meta.size)?;
        let etag = meta
            .e_tag
            .ok_or_else(|| invalid_response("missing object ETag"))?;
        // Share the same bounded, version-checked read path for encrypted and
        // plain objects. The library preserves input ordering and merges overlap.
        object_store::coalesce_ranges(
            ranges,
            |range| {
                let etag = etag.clone();
                async move {
                    self.get_opts(
                        location,
                        object_store::GetOptions {
                            range: Some(object_store::GetRange::Bounded(range)),
                            if_match: Some(etag),
                            ..Default::default()
                        },
                    )
                    .await?
                    .bytes()
                    .await
                }
            },
            0,
        )
        .await
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, object_store::Result<Path>>,
    ) -> BoxStream<'static, object_store::Result<Path>> {
        let client = self.client.clone();
        locations
            .map(move |location| {
                let _client = client.clone();
                async move {
                    let location = location?;
                    match _client.delete(&location).await.map_err(from_error) {
                        Ok(_) => Ok(location),
                        Err(err) => Err(err),
                    }
                }
            })
            .buffered(8)
            .boxed()
    }

    /// Lists objects under a prefix
    fn list(
        &self,
        prefix: Option<&Path>,
    ) -> BoxStream<'static, object_store::Result<object_store::ObjectMeta>> {
        let prefix = prefix.cloned();
        let client = self.client.clone();
        list_stream(client, prefix, None).boxed()
    }

    /// Lists objects starting from an offset
    fn list_with_offset(
        &self,
        prefix: Option<&Path>,
        offset: &Path,
    ) -> BoxStream<'static, object_store::Result<object_store::ObjectMeta>> {
        let prefix = prefix.cloned();
        let offset = offset.clone();
        let client = self.client.clone();
        list_stream(client, prefix, Some(offset)).boxed()
    }

    /// Lists objects with directory delimiter
    async fn list_with_delimiter(
        &self,
        prefix: Option<&Path>,
    ) -> object_store::Result<object_store::ListResult> {
        collect_with_delimiter(self.list(prefix), prefix).await
    }

    async fn copy_opts(
        &self,
        from: &Path,
        to: &Path,
        options: CopyOptions,
    ) -> object_store::Result<()> {
        match options.mode {
            CopyMode::Overwrite => self.client.copy(from, to).await.map_err(from_error),
            CopyMode::Create => self
                .client
                .copy_if_not_exists(from, to)
                .await
                .map_err(from_error),
        }
    }

    async fn rename_opts(
        &self,
        from: &Path,
        to: &Path,
        options: RenameOptions,
    ) -> object_store::Result<()> {
        match options.target_mode {
            RenameTargetMode::Overwrite => self.client.rename(from, to).await.map_err(from_error),
            RenameTargetMode::Create => self
                .client
                .rename_if_not_exists(from, to)
                .await
                .map_err(from_error),
        }
    }
}

fn encrypt_chunk(
    cipher: &Aes256Gcm,
    nonce: &Nonce<U12>,
    chunk: &mut [u8],
    path: &Path,
) -> Result<ByteArray<16>, object_store::Error> {
    let tag = cipher
        .encrypt_inout_detached(nonce, &[], chunk.into())
        .map_err(|err| object_store::Error::Generic {
            store: STORE_NAME,
            source: format!("AES256 encrypt failed for path {path}: {err:?}").into(),
        })?;
    let tag: [u8; 16] = tag.into();
    Ok(tag.into())
}

fn decrypt_chunk(
    cipher: &Aes256Gcm,
    nonce: &Nonce<U12>,
    chunk: &mut [u8],
    tag: &ByteArray<16>,
    path: &Path,
) -> Result<(), object_store::Error> {
    cipher
        .decrypt_inout_detached(nonce, &[], chunk.into(), &Tag::from(**tag))
        .map_err(|err| object_store::Error::Generic {
            store: STORE_NAME,
            source: format!("AES256 decrypt failed for path {path}: {err:?}").into(),
        })
}

async fn collect_with_delimiter(
    mut stream: BoxStream<'static, object_store::Result<object_store::ObjectMeta>>,
    prefix: Option<&Path>,
) -> object_store::Result<object_store::ListResult> {
    // Derive directory entries from the paginated flat listing. This also
    // avoids truncating a directory when it contains more than 1,000 entries.
    let start = prefix
        .filter(|p| !p.as_ref().is_empty())
        .map_or_else(String::new, |p| format!("{p}/"));
    let mut objects = Vec::new();
    let mut common_prefixes = BTreeSet::new();
    while let Some(meta) = stream.next().await {
        let meta = meta?;
        let path = meta.location.as_ref();
        if let Some(relative) = path.strip_prefix(&start) {
            if let Some(separator) = relative.find('/') {
                common_prefixes.insert(Path::parse(&path[..start.len() + separator])?);
            } else {
                objects.push(meta);
            }
        }
    }
    Ok(object_store::ListResult {
        objects,
        common_prefixes: common_prefixes.into_iter().collect(),
        extensions: Extensions::default(),
    })
}

fn invalid_response(message: &str) -> object_store::Error {
    object_store::Error::Generic {
        store: STORE_NAME,
        source: message.to_string().into(),
    }
}

fn list_stream<C: ObjectStoreSDK + Send + Sync + 'static>(
    client: Arc<C>,
    prefix: Option<Path>,
    mut offset: Option<Path>,
) -> BoxStream<'static, object_store::Result<object_store::ObjectMeta>> {
    try_stream! {
        loop {
            let page = match &offset {
                Some(offset) => client.list_with_offset(prefix.as_ref(), offset).await,
                None => client.list(prefix.as_ref()).await,
            }.map_err(from_error)?;
            let Some(last) = page.last() else { break };
            let next = Path::parse(&last.location)?;
            if offset.as_ref().is_some_and(|prev| next <= *prev) {
                Err(invalid_response("listing cursor did not advance"))?;
            }
            offset = Some(next);
            for object in page { yield from_object_meta(object); }
        }
    }
    .boxed()
}

async fn fetch_range<C: ObjectStoreSDK + Sync>(
    client: &C,
    location: &Path,
    range: Range<u64>,
    etag: &str,
) -> object_store::Result<bytes::Bytes> {
    let res = client
        .get_opts(
            location,
            GetOptions {
                range: Some(GetRange::Bounded(range.start, range.end)),
                if_match: Some(etag.to_string()),
                ..Default::default()
            },
        )
        .await
        .map_err(from_error)?;
    if res.range != (range.start, range.end)
        || res.payload.len() as u64 != range.end - range.start
        || res.meta.e_tag.as_deref() != Some(etag)
    {
        return Err(invalid_response(
            "range response does not match the requested object version and bytes",
        ));
    }
    Ok(bytes::Bytes::from(res.payload.into_vec()))
}

fn create_get_range_stream<C: ObjectStoreSDK + Send + Sync + 'static>(
    client: Arc<C>,
    location: Path,
    request_range: Range<u64>,
    first_range: Range<u64>,
    first_payload: bytes::Bytes,
    etag: String,
) -> BoxStream<'static, object_store::Result<bytes::Bytes>> {
    try_stream! {
        yield first_payload;
        const FETCH_SIZE: u64 = (MAX_PAYLOAD_SIZE / CHUNK_SIZE) * CHUNK_SIZE;
        let mut current = first_range.end;
        while current < request_range.end {
            let end = (current + FETCH_SIZE).min(request_range.end);
            yield fetch_range(client.as_ref(), &location, current..end, &etag).await?;
            current = end;
        }
    }
    .boxed()
}

#[allow(clippy::too_many_arguments)]
fn create_decryption_stream(
    res: object_store::GetResult,
    cipher: Arc<Aes256Gcm>,
    aes_tags: Vec<ByteArray<16>>,
    base_nonce: [u8; 12],
    location: Path,
    start_idx: usize,
    start_offset: usize,
    size: usize,
) -> BoxStream<'static, object_store::Result<bytes::Bytes>> {
    try_stream! {
        let mut stream = res.into_stream();
        // 预分配足够大的缓冲区以减少重新分配次数
        let mut buf = BytesMut::with_capacity(CHUNK_SIZE as usize * 2);
        let mut idx = start_idx;
        let mut remaining = size;

        while let Some(data) = stream.next().await {
            let data = data?;
            if remaining == 0 {
                // 已满足请求大小，提前结束
                break;
            }
            buf.extend_from_slice(&data);

            while remaining > 0 && buf.len() >= CHUNK_SIZE as usize {
                let mut chunk = buf.split_to(CHUNK_SIZE as usize);

                let tag = aes_tags.get(idx).ok_or_else(|| object_store::Error::Generic {
                    store: STORE_NAME,
                    source: format!("missing AES256 tag for chunk {idx} for path {location}").into(),
                })?;

                let nonce = derive_gcm_nonce(&base_nonce, idx as u64);
                decrypt_chunk(&cipher, &Nonce::from(nonce), &mut chunk, tag, &location)?;
                // 首块去掉起始偏移
                if idx == start_idx && start_offset > 0 {
                    chunk.advance(start_offset);
                }

                if chunk.len() > remaining {
                    chunk.truncate(remaining);
                }

                remaining = remaining.saturating_sub(chunk.len());
                yield chunk.freeze();

                idx += 1;
                if remaining == 0 {
                    // 已满足请求大小，提前结束
                    return;
                }
            }
        }

        if remaining > 0 && !buf.is_empty() {
            let tag = aes_tags.get(idx).ok_or_else(|| object_store::Error::Generic {
                store: STORE_NAME,
                source: format!("missing AES256 tag for chunk {idx} for path {location}").into(),
            })?;
            let nonce = derive_gcm_nonce(&base_nonce, idx as u64);
            decrypt_chunk(&cipher, &Nonce::from(nonce), &mut buf, tag, &location)?;
            if idx == start_idx && start_offset > 0 {
                buf.advance(start_offset);
            }

            buf.truncate(remaining);
            yield buf.freeze();
        }
    }.boxed()
}

/// Converts custom Error type to object_store::Error
///
/// Maps each error variant to its corresponding object_store error,
/// preserving relevant context like path and error message.
pub fn from_error(err: Error) -> object_store::Error {
    match err {
        Error::Generic { error } => object_store::Error::Generic {
            store: STORE_NAME,
            source: error.into(),
        },
        Error::NotFound { ref path } => object_store::Error::NotFound {
            path: path.clone(),
            source: Box::new(err),
        },
        Error::InvalidPath { path } => object_store::Error::InvalidPath {
            source: object_store::path::Error::InvalidPath { path: path.into() },
        },
        Error::NotSupported { error } => object_store::Error::NotSupported {
            source: error.into(),
        },
        Error::AlreadyExists { ref path } => object_store::Error::AlreadyExists {
            path: path.clone(),
            source: err.into(),
        },
        Error::Precondition { path, error } => object_store::Error::Precondition {
            path,
            source: error.into(),
        },
        Error::NotModified { path, error } => object_store::Error::NotModified {
            path,
            source: error.into(),
        },
        Error::NotImplemented {
            operation,
            implementer,
        } => object_store::Error::NotImplemented {
            operation,
            implementer,
        },
        Error::PermissionDenied { path, error } => object_store::Error::PermissionDenied {
            path,
            source: error.into(),
        },
        Error::Unauthenticated { path, error } => object_store::Error::Unauthenticated {
            path,
            source: error.into(),
        },
        Error::UnknownConfigurationKey { key } => object_store::Error::UnknownConfigurationKey {
            store: STORE_NAME,
            key,
        },
        _ => object_store::Error::Generic {
            store: STORE_NAME,
            source: Box::new(err),
        },
    }
}

/// Converts internal ObjectMeta to object_store::ObjectMeta
///
/// # Arguments
/// * `val` - The source ObjectMeta to convert
///
/// # Returns
/// Converted object_store::ObjectMeta with equivalent fields
pub fn from_object_meta(val: ObjectMeta) -> object_store::ObjectMeta {
    object_store::ObjectMeta {
        // this metadata is not authenticated, so fall back rather than panic
        location: Path::parse(&val.location).unwrap_or_else(|_| Path::from(val.location.as_str())),
        last_modified: DateTime::from_timestamp_millis(val.last_modified as i64)
            .unwrap_or_default(),
        size: val.size,
        e_tag: val.e_tag,
        version: val.version,
    }
}

/// Converts object_store::GetRange to internal GetRange format
///
/// # Arguments
/// * `val` - The source GetRange to convert
///
/// # Returns
/// Converted GetRange with equivalent range type and values
pub fn to_get_range(val: object_store::GetRange) -> GetRange {
    match val {
        object_store::GetRange::Bounded(v) => GetRange::Bounded(v.start, v.end),
        object_store::GetRange::Offset(v) => GetRange::Offset(v),
        object_store::GetRange::Suffix(v) => GetRange::Suffix(v),
    }
}

/// Converts internal Attribute to object_store::Attribute
///
/// Maps each attribute variant to its corresponding object_store attribute,
/// handling metadata conversion as well.
pub fn from_attribute(val: Attribute) -> object_store::Attribute {
    match val {
        Attribute::ContentDisposition => object_store::Attribute::ContentDisposition,
        Attribute::ContentEncoding => object_store::Attribute::ContentEncoding,
        Attribute::ContentLanguage => object_store::Attribute::ContentLanguage,
        Attribute::ContentType => object_store::Attribute::ContentType,
        Attribute::CacheControl => object_store::Attribute::CacheControl,
        Attribute::Metadata(v) => object_store::Attribute::Metadata(v.into()),
    }
}

/// Converts object_store::Attribute to internal Attribute type
///
/// Maps standard object store attributes to internal representation,
/// handling metadata conversion as well.
///
/// # Panics
/// Will panic if an unexpected attribute variant is encountered
pub fn to_attribute(val: &object_store::Attribute) -> Attribute {
    match val {
        object_store::Attribute::ContentDisposition => Attribute::ContentDisposition,
        object_store::Attribute::ContentEncoding => Attribute::ContentEncoding,
        object_store::Attribute::ContentLanguage => Attribute::ContentLanguage,
        object_store::Attribute::ContentType => Attribute::ContentType,
        object_store::Attribute::CacheControl => Attribute::CacheControl,
        object_store::Attribute::Metadata(v) => Attribute::Metadata(v.to_string()),
        _ => panic!("unexpected attribute"),
    }
}

/// Converts object_store::PutOptions to internal PutOptions format
///
/// Maps standard object store put options to internal representation,
/// handling mode, tags, and attributes conversion.
pub fn to_put_options(opts: &object_store::PutOptions) -> PutOptions {
    let mode: PutMode = match opts.mode {
        object_store::PutMode::Overwrite => PutMode::Overwrite,
        object_store::PutMode::Create => PutMode::Create,
        object_store::PutMode::Update(ref v) => PutMode::Update(UpdateVersion {
            e_tag: v.e_tag.clone(),
            version: v.version.clone(),
        }),
    };
    PutOptions {
        mode,
        tags: opts.tags.encoded().to_string(),
        attributes: opts
            .attributes
            .iter()
            .map(|(k, v)| (to_attribute(k), v.to_string()))
            .collect(),
        ..Default::default()
    }
}

fn ranges_is_valid(ranges: &[Range<u64>], len: u64) -> object_store::Result<()> {
    for range in ranges {
        if range.start >= len {
            return Err(object_store::Error::Generic {
                store: STORE_NAME,
                source: format!("start {} is larger than length {}", range.start, len).into(),
            });
        }
        if range.end <= range.start {
            return Err(object_store::Error::Generic {
                store: STORE_NAME,
                source: format!("end {} is less than start {}", range.end, range.start).into(),
            });
        }
        if range.end > len {
            return Err(object_store::Error::Generic {
                store: STORE_NAME,
                source: format!("end {} is larger than length {}", range.end, len).into(),
            });
        }
    }
    Ok(())
}

// 为每个分块从基准 nonce 派生唯一的 GCM nonce（后 8 字节作为计数器）
fn derive_gcm_nonce(base: &[u8; 12], idx: u64) -> [u8; 12] {
    let mut nonce = *base;
    let mut ctr = [0u8; 8];
    ctr.copy_from_slice(&nonce[4..12]);
    let c = u64::from_le_bytes(ctr).wrapping_add(idx);
    nonce[4..12].copy_from_slice(&c.to_le_bytes());
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::build_agent;
    use ic_agent::{identity::BasicIdentity, Identity};
    use ic_cose_types::cose::sha3_256;
    use object_store::{integration::*, ObjectStoreExt};

    #[test]
    fn test_encrypt_decrypt_chunk() {
        use aes_gcm::KeyInit;

        let secret = [8u8; 32];
        let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(secret));
        let path = Path::from("test/hello.txt");
        let base_nonce: [u8; 12] = rand_bytes();
        let plain = b"Hello Anda!".to_vec();

        let mut chunk = plain.clone();
        let nonce = derive_gcm_nonce(&base_nonce, 1);
        let tag = encrypt_chunk(&cipher, &Nonce::from(nonce), &mut chunk, &path).unwrap();
        assert_ne!(chunk, plain);

        decrypt_chunk(&cipher, &Nonce::from(nonce), &mut chunk, &tag, &path).unwrap();
        assert_eq!(chunk, plain);

        let bad_nonce = derive_gcm_nonce(&base_nonce, 2);
        let mut chunk = plain.clone();
        let tag = encrypt_chunk(&cipher, &Nonce::from(nonce), &mut chunk, &path).unwrap();
        assert!(decrypt_chunk(&cipher, &Nonce::from(bad_nonce), &mut chunk, &tag, &path).is_err());
    }

    #[test]
    fn test_from_object_meta_tolerates_bad_metadata() {
        // this metadata is not authenticated, so it must never panic the caller
        let meta = ObjectMeta {
            location: "a/b.txt".to_string(),
            last_modified: 1_700_000_000_123,
            size: 10,
            e_tag: None,
            version: None,
            aes_nonce: None,
            aes_tags: None,
        };
        let out = from_object_meta(meta.clone());
        assert_eq!(out.location, Path::from("a/b.txt"));
        assert_eq!(out.last_modified.timestamp_millis(), 1_700_000_000_123);

        // a timestamp DateTime cannot represent falls back instead of aborting.
        // note u64::MAX casts to -1, which is a perfectly valid instant, so this
        // needs a value beyond the ~+262,000 year limit
        assert!(DateTime::from_timestamp_millis((1u64 << 62) as i64).is_none());
        let out = from_object_meta(ObjectMeta {
            last_modified: 1u64 << 62,
            ..meta.clone()
        });
        assert_eq!(out.last_modified, DateTime::<chrono::Utc>::default());

        // so does a location Path::parse rejects
        let out = from_object_meta(ObjectMeta {
            location: "a//b\\c".to_string(),
            ..meta
        });
        assert!(!out.location.as_ref().is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore]
    async fn test_client() {
        let secret = [8u8; 32];
        let canister = Principal::from_text("6at64-oyaaa-aaaap-anvza-cai").unwrap();
        let id = BasicIdentity::from_raw_key(&secret);
        println!("id: {:?}", id.sender().unwrap().to_text());
        // jjn6g-sh75l-r3cxb-wxrkl-frqld-6p6qq-d4ato-wske5-op7s5-n566f-bqe

        let agent = build_agent("http://localhost:4943", Arc::new(id))
            .await
            .unwrap();
        let cli = Arc::new(Client::new(Arc::new(agent), canister, Some(secret)));
        let oc = ObjectStoreClient::new(cli.clone());

        let path = Path::from("test/hello.txt");
        let payload = "Hello Anda!".as_bytes().to_vec();
        let res = oc
            .put_opts(&path, payload.clone().into(), Default::default())
            .await
            .unwrap();
        println!("put result: {:?}", res);

        let res = oc.get_opts(&path, Default::default()).await.unwrap();
        assert_eq!(res.meta.size as usize, payload.len());
        let res = res.bytes().await.unwrap();
        assert_eq!(res.to_vec(), payload);

        let res = cli.get_opts(&path, Default::default()).await.unwrap();
        assert_eq!(res.meta.size as usize, payload.len());
        assert_ne!(&res.payload, &payload);
        let aes_nonce = res.meta.aes_nonce.unwrap();
        assert_eq!(aes_nonce.len(), 12);
        let aes_tags = res.meta.aes_tags.unwrap();
        assert_eq!(aes_tags.len(), 1);

        let now = chrono::Utc::now();
        let path = Path::from(format!("test/{}.bin", now.timestamp_millis()));
        let count = 20000u64;
        let len = count * 32;
        let mut payload = Vec::with_capacity(len as usize);
        {
            let mut uploder = oc
                .put_multipart_opts(&path, Default::default())
                .await
                .unwrap();

            for i in 0..count {
                let data = sha3_256(&i.to_be_bytes()).to_vec();
                payload.extend_from_slice(&data);
                uploder
                    .put_part(object_store::PutPayload::from(data))
                    .await
                    .unwrap();
            }

            uploder.complete().await.unwrap();
        }
        let res = oc.get_opts(&path, Default::default()).await.unwrap();
        assert_eq!(res.meta.size as usize, payload.len());
        let res = res.bytes().await.unwrap();
        assert_eq!(res.to_vec(), payload);

        let res = cli.get_opts(&path, Default::default()).await.unwrap();
        assert_eq!(res.meta.size as usize, payload.len());
        assert_ne!(&res.payload, &payload);
        let aes_nonce = res.meta.aes_nonce.unwrap();
        assert_eq!(aes_nonce.len(), 12);
        let aes_tags = res.meta.aes_tags.unwrap();
        assert_eq!(aes_tags.len(), len.div_ceil(CHUNK_SIZE) as usize);

        let ranges = vec![0u64..1000, 100..100000, len - CHUNK_SIZE - 1..len];

        let rt = oc.get_ranges(&path, &ranges).await.unwrap();
        assert_eq!(rt.len(), ranges.len());

        for (i, Range { start, end }) in ranges.into_iter().enumerate() {
            let res = oc
                .get_opts(
                    &path,
                    object_store::GetOptions {
                        range: Some(object_store::GetRange::Bounded(start..end)),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            assert_eq!(res.meta.location, path);
            assert_eq!(res.meta.size as usize, payload.len());
            let data = res.bytes().await.unwrap();
            assert_eq!(rt[i].len(), data.len());
            assert_eq!(&data, &payload[start as usize..end as usize]);
        }
    }

    const NON_EXISTENT_NAME: &str = "nonexistentname";

    #[tokio::test]
    #[ignore]
    async fn integration_test() {
        // Should be run in a clean environment
        // dfx canister call ic_object_store_canister admin_clear '()'
        let secret = [8u8; 32];
        let canister = Principal::from_text("6at64-oyaaa-aaaap-anvza-cai").unwrap();
        let id = BasicIdentity::from_raw_key(&secret);
        println!("id: {:?}", id.sender().unwrap().to_text());
        // jjn6g-sh75l-r3cxb-wxrkl-frqld-6p6qq-d4ato-wske5-op7s5-n566f-bqe
        // # Add managers
        // dfx canister call ic_object_store_canister admin_add_managers "(vec {principal \"jjn6g-sh75l-r3cxb-wxrkl-frqld-6p6qq-d4ato-wske5-op7s5-n566f-bqe\"})"

        // It will take a long time to run this test.
        // test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 396.77s

        let agent = build_agent("http://localhost:4943", Arc::new(id))
            .await
            .unwrap();
        let cli = Arc::new(Client::new(Arc::new(agent), canister, Some(secret)));
        let storage = ObjectStoreClient::new(cli.clone());

        let location = Path::from(NON_EXISTENT_NAME);

        let err = get_nonexistent_object(&storage, Some(location))
            .await
            .unwrap_err();
        if let object_store::Error::NotFound { path, .. } = err {
            assert!(path.ends_with(NON_EXISTENT_NAME));
        } else {
            panic!("unexpected error type: {err:?}");
        }

        put_get_delete_list(&storage).await;
        put_get_attributes(&storage).await;
        get_opts(&storage).await;
        put_opts(&storage, true).await;
        list_uses_directories_correctly(&storage).await;
        list_with_delimiter(&storage).await;
        rename_and_copy(&storage).await;
        copy_if_not_exists(&storage).await;
        copy_rename_nonexistent_object(&storage).await;
        // multipart_race_condition(&storage, true).await; // TODO: fix this test?
        multipart_out_of_order(&storage).await;

        let objs = storage.list(None).collect::<Vec<_>>().await;
        for obj in objs {
            let obj = obj.unwrap();
            storage
                .delete(&obj.location)
                .await
                .expect("failed to delete object");
        }
        stream_get(&storage).await;
    }
    struct MockReader {
        principal: Principal,
        version: std::sync::atomic::AtomicU64,
        data: Vec<u8>,
        list_calls: std::sync::atomic::AtomicUsize,
        requests: std::sync::Mutex<Vec<Range<u64>>>,
    }

    impl MockReader {
        fn new(size: usize) -> Self {
            Self {
                principal: Principal::anonymous(),
                version: std::sync::atomic::AtomicU64::new(1),
                data: (0..size).map(|i| (i % 251) as u8).collect(),
                list_calls: std::sync::atomic::AtomicUsize::new(0),
                requests: std::sync::Mutex::new(Vec::new()),
            }
        }
        fn meta(&self, location: String) -> ObjectMeta {
            ObjectMeta {
                location,
                last_modified: 0,
                size: self.data.len() as u64,
                e_tag: Some(
                    self.version
                        .load(std::sync::atomic::Ordering::SeqCst)
                        .to_string(),
                ),
                version: None,
                aes_nonce: None,
                aes_tags: None,
            }
        }
    }

    impl CanisterCaller for MockReader {
        async fn canister_query<
            In: ArgumentEncoder + Send,
            Out: CandidType + for<'a> candid::Deserialize<'a>,
        >(
            &self,
            _: &Principal,
            method: &str,
            args: In,
        ) -> Result<Out, BoxError> {
            let args = encode_args(args)?;
            let encoded = match method {
                "get_opts" => {
                    let (path, opts): (String, GetOptions) = candid::decode_args(&args)?;
                    let meta = self.meta(path);
                    let result = opts.check_preconditions(&meta).map(|()| {
                        let range = opts.range.unwrap().into_range(meta.size).unwrap();
                        assert!(range.end - range.start <= MAX_PAYLOAD_SIZE);
                        self.requests.lock().unwrap().push(range.clone());
                        GetResult {
                            payload: self.data[range.start as usize..range.end as usize]
                                .to_vec()
                                .into(),
                            meta,
                            range: (range.start, range.end),
                            attributes: Default::default(),
                        }
                    });
                    candid::encode_one(result)?
                }
                "list" | "list_with_offset" => {
                    self.list_calls
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let offset = if method == "list_with_offset" {
                        candid::decode_args::<(Option<String>, String)>(&args)?.1
                    } else {
                        String::new()
                    };
                    let page = (0..1001)
                        .map(|i| format!("dir/{i:04}"))
                        .filter(|path| path > &offset)
                        .take(1000)
                        .map(|path| self.meta(path))
                        .collect::<Vec<_>>();
                    candid::encode_one(Ok::<_, Error>(page))?
                }
                _ => panic!("unexpected query {method}"),
            };
            Ok(candid::decode_one(&encoded)?)
        }
        async fn canister_update<
            In: ArgumentEncoder + Send,
            Out: CandidType + for<'a> candid::Deserialize<'a>,
        >(
            &self,
            _: &Principal,
            _: &str,
            _: In,
        ) -> Result<Out, BoxError> {
            panic!("unexpected update")
        }
    }
    impl ObjectStoreSDK for MockReader {
        fn canister(&self) -> &Principal {
            &self.principal
        }
        fn cipher(&self) -> Option<Arc<Aes256Gcm>> {
            None
        }
    }

    #[tokio::test]
    async fn paginated_list_and_delimiter_include_more_than_one_page() {
        use futures::TryStreamExt;
        let client = Arc::new(MockReader::new(0));
        let entries: Vec<_> = list_stream(client.clone(), None, None)
            .try_collect()
            .await
            .unwrap();
        assert_eq!(entries.len(), 1001);
        assert_eq!(entries.last().unwrap().location.as_ref(), "dir/1000");
        let prefix = Path::from("dir");
        let directory = collect_with_delimiter(
            list_stream(client.clone(), Some(prefix.clone()), None),
            Some(&prefix),
        )
        .await
        .unwrap();
        assert_eq!(directory.objects.len(), 1001);
        let root = collect_with_delimiter(list_stream(client.clone(), None, None), None)
            .await
            .unwrap();
        assert_eq!(root.common_prefixes, vec![prefix]);
        assert!(root.objects.is_empty());
    }

    #[tokio::test]
    async fn streaming_reads_are_bounded_and_reject_an_object_replacement() {
        use futures::TryStreamExt;
        let client = Arc::new(MockReader::new(4 * 1024 * 1024));
        let first_end = MAX_PAYLOAD_SIZE as usize;
        let stream = || {
            create_get_range_stream(
                client.clone(),
                Path::from("object"),
                0..client.data.len() as u64,
                0..first_end as u64,
                bytes::Bytes::copy_from_slice(&client.data[..first_end]),
                "1".into(),
            )
        };
        let chunks: Vec<_> = stream().try_collect().await.unwrap();
        assert_eq!(chunks.concat(), client.data);
        assert_eq!(client.requests.lock().unwrap().len(), 2);
        let mut stream = stream();
        assert!(stream.next().await.unwrap().is_ok());
        client.version.store(2, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            stream.next().await.unwrap(),
            Err(object_store::Error::Precondition { .. })
        ));
    }

    #[tokio::test]
    async fn decryption_handles_unaligned_network_chunks_and_requested_range() {
        use aes_gcm::KeyInit;
        use futures::TryStreamExt;
        let cipher = Arc::new(Aes256Gcm::new(&Key::<Aes256Gcm>::from([4; 32])));
        let plain: Vec<_> = (0..(CHUNK_SIZE as usize * 9 + 47))
            .map(|i| (i % 251) as u8)
            .collect();
        let mut encrypted = plain.clone();
        let base_nonce = [6; 12];
        let location = Path::from("encrypted");
        let tags = encrypted
            .chunks_mut(CHUNK_SIZE as usize)
            .enumerate()
            .map(|(i, chunk)| {
                encrypt_chunk(
                    &cipher,
                    &Nonce::from(derive_gcm_nonce(&base_nonce, i as u64)),
                    chunk,
                    &location,
                )
                .unwrap()
            })
            .collect();
        let packets = encrypted
            .chunks(70_001)
            .map(bytes::Bytes::copy_from_slice)
            .map(Ok)
            .collect::<Vec<_>>();
        let meta = from_object_meta(MockReader::new(0).meta("encrypted".into()));
        let result = object_store::GetResult {
            payload: object_store::GetResultPayload::Stream(futures::stream::iter(packets).boxed()),
            meta,
            range: 0..plain.len() as u64,
            attributes: Default::default(),
            extensions: Default::default(),
        };
        let chunks: Vec<_> = create_decryption_stream(
            result,
            cipher,
            tags,
            base_nonce,
            location,
            0,
            13,
            plain.len() - 30,
        )
        .try_collect()
        .await
        .unwrap();
        assert_eq!(chunks.concat(), plain[13..plain.len() - 17]);
    }
}
