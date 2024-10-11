use bytes::{Bytes, BytesMut};
use candid::{CandidType, Principal};
use ic_agent::Agent;
use ic_oss_types::{bucket::*, file::*, folder::*, format_error};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use sha3::{Digest, Sha3_256};
use std::{collections::BTreeSet, sync::Arc};
use tokio::io::AsyncRead;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, FramedRead};

use crate::agent::{query_call, update_call};

#[derive(Clone)]
pub struct Client {
    concurrency: u8,
    agent: Arc<Agent>,
    bucket: Principal,
    set_readonly: bool,
    access_token: Option<ByteBuf>,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct UploadFileChunksResult {
    pub id: u32,
    pub filled: u64,
    pub uploaded_chunks: BTreeSet<u32>,
    pub error: Option<String>, // if any error occurs during upload
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Progress {
    pub filled: u64,
    pub size: Option<u64>, // total size of file, may be unknown
    pub chunk_index: u32,
    pub concurrency: u8,
}

impl Client {
    pub fn new(agent: Arc<Agent>, bucket: Principal) -> Client {
        Client {
            concurrency: 16,
            agent,
            bucket,
            set_readonly: false,
            access_token: None,
        }
    }

    pub fn set_concurrency(&mut self, concurrency: u8) {
        if concurrency > 0 && concurrency <= 64 {
            self.concurrency = concurrency;
        }
    }

    pub fn set_readonly(&mut self, readonly: bool) {
        self.set_readonly = readonly;
    }

    /// the caller of agent should be canister controller
    pub async fn admin_set_managers(&self, args: BTreeSet<Principal>) -> Result<(), String> {
        update_call(&self.agent, &self.bucket, "admin_set_managers", (args,)).await?
    }

    /// the caller of agent should be canister controller
    pub async fn admin_set_auditors(&self, args: BTreeSet<Principal>) -> Result<(), String> {
        update_call(&self.agent, &self.bucket, "admin_set_auditors", (args,)).await?
    }

    /// the caller of agent should be canister controller
    pub async fn admin_update_bucket(&self, args: UpdateBucketInput) -> Result<(), String> {
        update_call(&self.agent, &self.bucket, "admin_update_bucket", (args,)).await?
    }

    pub async fn get_bucket_info(&self) -> Result<BucketInfo, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "get_bucket_info",
            (&self.access_token,),
        )
        .await?
    }

    pub async fn get_file_info(&self, id: u32) -> Result<FileInfo, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "get_file_info",
            (id, &self.access_token),
        )
        .await?
    }

    pub async fn get_file_info_by_hash(&self, hash: ByteArray<32>) -> Result<FileInfo, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "get_file_info_by_hash",
            (hash, &self.access_token),
        )
        .await?
    }

    pub async fn get_file_ancestors(&self, id: u32) -> Result<Vec<FolderName>, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "get_file_ancestors",
            (id, &self.access_token),
        )
        .await?
    }

    pub async fn get_file_chunks(
        &self,
        id: u32,
        index: u32,
        take: Option<u32>,
    ) -> Result<Vec<FileChunk>, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "get_file_chunks",
            (id, index, take, &self.access_token),
        )
        .await?
    }

    pub async fn list_files(
        &self,
        parent: u32,
        prev: Option<u32>,
        take: Option<u32>,
    ) -> Result<Vec<FileInfo>, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "list_files",
            (parent, prev, take, &self.access_token),
        )
        .await?
    }

    pub async fn get_folder_info(&self, id: u32) -> Result<FolderInfo, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "get_folder_info",
            (id, &self.access_token),
        )
        .await?
    }

    pub async fn get_folder_ancestors(&self, id: u32) -> Result<Vec<FolderName>, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "get_folder_ancestors",
            (id, &self.access_token),
        )
        .await?
    }

    pub async fn list_folders(
        &self,
        parent: u32,
        prev: Option<u32>,
        take: Option<u32>,
    ) -> Result<Vec<FolderInfo>, String> {
        query_call(
            &self.agent,
            &self.bucket,
            "list_folders",
            (parent, prev, take, &self.access_token),
        )
        .await?
    }

    pub async fn create_file(&self, file: CreateFileInput) -> Result<CreateFileOutput, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "create_file",
            (file, &self.access_token),
        )
        .await?
    }

    pub async fn update_file_chunk(
        &self,
        input: UpdateFileChunkInput,
    ) -> Result<UpdateFileChunkOutput, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "update_file_chunk",
            (input, &self.access_token),
        )
        .await?
    }

    pub async fn update_file_info(
        &self,
        input: UpdateFileInput,
    ) -> Result<UpdateFileOutput, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "update_file_info",
            (input, &self.access_token),
        )
        .await?
    }

    pub async fn move_file(&self, input: MoveInput) -> Result<UpdateFileOutput, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "move_file",
            (input, &self.access_token),
        )
        .await?
    }

    pub async fn delete_file(&self, id: u32) -> Result<bool, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "delete_file",
            (id, &self.access_token),
        )
        .await?
    }

    pub async fn batch_delete_subfiles(
        &self,
        parent: u32,
        ids: BTreeSet<u32>,
    ) -> Result<Vec<u32>, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "batch_delete_subfiles",
            (parent, ids, &self.access_token),
        )
        .await?
    }

    pub async fn create_folder(
        &self,
        input: CreateFolderInput,
    ) -> Result<CreateFolderOutput, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "create_folder",
            (input, &self.access_token),
        )
        .await?
    }

    pub async fn update_folder_info(
        &self,
        input: UpdateFolderInput,
    ) -> Result<UpdateFolderOutput, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "update_folder_info",
            (input, &self.access_token),
        )
        .await?
    }

    pub async fn move_folder(&self, input: MoveInput) -> Result<UpdateFolderOutput, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "move_folder",
            (input, &self.access_token),
        )
        .await?
    }

    pub async fn delete_folder(&self, id: u32) -> Result<bool, String> {
        update_call(
            &self.agent,
            &self.bucket,
            "delete_folder",
            (id, &self.access_token),
        )
        .await?
    }

    pub async fn upload<T, F>(
        &self,
        stream: T,
        mut file: CreateFileInput,
        on_progress: F,
    ) -> Result<UploadFileChunksResult, String>
    where
        T: AsyncRead,
        F: Fn(Progress),
    {
        if let Some(size) = file.size {
            if size <= MAX_FILE_SIZE_PER_CALL {
                // upload a small file in one request
                let content = try_read_all(stream, size as u32).await?;
                if file.hash.is_none() {
                    let mut hasher = Sha3_256::new();
                    hasher.update(&content);
                    let hash: [u8; 32] = hasher.finalize().into();
                    file.hash = Some(hash.into());
                }
                file.content = Some(ByteBuf::from(content.to_vec()));
                file.status = if self.set_readonly { Some(1) } else { None };
                let res = self.create_file(file).await?;

                on_progress(Progress {
                    filled: size,
                    size: Some(size),
                    chunk_index: 0,
                    concurrency: 1,
                });
                return Ok(UploadFileChunksResult {
                    id: res.id,
                    filled: size,
                    uploaded_chunks: BTreeSet::new(),
                    error: None,
                });
            }
        }

        // create file
        let hash = file.hash;
        let size = file.size;
        let res = self.create_file(file).await?;
        let res = self
            .upload_chunks(stream, res.id, size, hash, &BTreeSet::new(), on_progress)
            .await;
        Ok(res)
    }

    pub async fn upload_chunks<T, F>(
        &self,
        stream: T,
        id: u32,
        size: Option<u64>,
        hash: Option<ByteArray<32>>,
        exclude_chunks: &BTreeSet<u32>,
        on_progress: F,
    ) -> UploadFileChunksResult
    where
        T: AsyncRead,
        F: Fn(Progress),
    {
        // upload chunks
        let bucket = self.bucket;
        let has_hash = hash.is_some();
        let mut frames = Box::pin(FramedRead::new(stream, ChunksCodec::new(CHUNK_SIZE)));
        let (tx, mut rx) = mpsc::channel::<Result<Progress, String>>(self.concurrency as usize);
        let output = Arc::new(RwLock::new(UploadFileChunksResult {
            id,
            filled: 0,
            uploaded_chunks: exclude_chunks.clone(),
            error: None,
        }));

        let uploading_loop = async {
            let mut index = 0;
            let mut hasher = Sha3_256::new();
            let semaphore = Arc::new(Semaphore::new(self.concurrency as usize));

            loop {
                let access_token = self.access_token.clone();
                let tx1 = tx.clone();
                let output = output.clone();
                let permit = semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(format_error)?;
                let concurrency = (self.concurrency as usize - semaphore.available_permits()) as u8;

                match frames.next().await {
                    None => {
                        drop(tx);
                        semaphore.close();
                        return Ok(Into::<[u8; 32]>::into(hasher.finalize()));
                    }
                    Some(Err(err)) => {
                        drop(tx);
                        semaphore.close();
                        return Err(err.to_string());
                    }
                    Some(Ok(chunk)) => {
                        let chunk_index = index;
                        index += 1;
                        let chunk_len = chunk.len() as u32;

                        if !has_hash {
                            hasher.update(&chunk);
                        }

                        if exclude_chunks.contains(&chunk_index) {
                            let mut r = output.write().await;
                            r.filled += chunk_len as u64;
                            on_progress(Progress {
                                filled: r.filled,
                                size,
                                chunk_index,
                                concurrency: 0,
                            });
                            drop(permit);
                            continue;
                        }

                        let agent = self.agent.clone();
                        tokio::spawn(async move {
                            let res = async {
                                let out: Result<UpdateFileChunkOutput, String> = update_call(
                                    &agent,
                                    &bucket,
                                    "update_file_chunk",
                                    (
                                        UpdateFileChunkInput {
                                            id,
                                            chunk_index,
                                            content: ByteBuf::from(chunk.to_vec()),
                                        },
                                        &access_token,
                                    ),
                                )
                                .await?;
                                let out = out?;
                                Ok(Progress {
                                    filled: out.filled,
                                    size,
                                    chunk_index,
                                    concurrency,
                                })
                            }
                            .await;

                            if res.is_ok() {
                                let mut r = output.write().await;
                                r.filled += chunk_len as u64;
                                r.uploaded_chunks.insert(chunk_index);
                                drop(permit);
                            }
                            let _ = tx1.send(res).await;
                        });
                    }
                }
            }
        };

        let uploading_result = async {
            while let Some(res) = rx.recv().await {
                match res {
                    Ok(progress) => {
                        on_progress(progress);
                    }
                    Err(err) => return Err(err),
                }
            }

            Ok(())
        };

        let result = async {
            let (hash_new, _) = futures::future::try_join(uploading_loop, uploading_result).await?;

            // commit file
            let _ = self
                .update_file_info(UpdateFileInput {
                    id,
                    hash: Some(hash.unwrap_or(hash_new.into())),
                    status: if self.set_readonly { Some(1) } else { None },
                    size,
                    ..Default::default()
                })
                .await?;
            Ok::<(), String>(())
        }
        .await;

        let mut output = output.read().await.to_owned();
        if let Err(err) = result {
            output.error = Some(err);
        }

        output
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ChunksCodec(u32);

impl ChunksCodec {
    pub fn new(len: u32) -> ChunksCodec {
        ChunksCodec(len)
    }
}

impl Decoder for ChunksCodec {
    type Item = Bytes;
    type Error = tokio::io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.len() >= self.0 as usize {
            Ok(Some(BytesMut::freeze(buf.split_to(self.0 as usize))))
        } else {
            Ok(None)
        }
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.is_empty() {
            Ok(None)
        } else {
            let len = buf.len();
            Ok(Some(BytesMut::freeze(buf.split_to(len))))
        }
    }
}

async fn try_read_all<T: AsyncRead>(stream: T, size: u32) -> Result<Bytes, String> {
    let mut frames = Box::pin(FramedRead::new(stream, ChunksCodec::new(size)));

    let res = frames.next().await.ok_or("no bytes to read".to_string())?;
    if frames.next().await.is_some() {
        return Err("too many bytes to read".to_string());
    }
    let res = res.map_err(format_error)?;
    if res.len() != size as usize {
        return Err("insufficient bytes to read".to_string());
    }
    Ok(res)
}
