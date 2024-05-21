use bytes::{Bytes, BytesMut};
use candid::{CandidType, Decode, Encode, Principal};
use ic_agent::Agent;
use ic_oss_types::{crc32_with_initial, file::*, format_error, nat_to_u64};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha3::{Digest, Sha3_256};
use std::{collections::BTreeSet, sync::Arc};
use tokio::io::AsyncRead;
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, FramedRead};

#[derive(Clone)]
pub struct Client {
    chunk_size: u32,
    concurrency: u8,
    agent: Arc<Agent>,
    bucket: Principal,
    access_token: Option<ByteBuf>,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct UploadFileChunksResult {
    pub id: u32,
    pub uploaded: usize,
    pub uploaded_chunks: BTreeSet<u32>,
    pub error: Option<String>, // if any error occurs during upload
}

impl Client {
    pub fn new(agent: Arc<Agent>, bucket: Principal) -> Client {
        Client {
            chunk_size: MAX_CHUNK_SIZE,
            concurrency: 16,
            agent,
            bucket,
            access_token: None,
        }
    }

    pub fn set_chunk_size(&mut self, chunk_size: u32) {
        if chunk_size > 1024 && chunk_size <= MAX_CHUNK_SIZE {
            self.chunk_size = chunk_size;
        }
    }

    pub fn set_concurrency(&mut self, concurrency: u8) {
        if concurrency > 0 && concurrency <= 64 {
            self.concurrency = concurrency;
        }
    }

    pub async fn upload<T, F>(
        &self,
        ar: T,
        file: CreateFileInput,
        progress: F,
    ) -> Result<UploadFileChunksResult, String>
    where
        T: AsyncRead,
        F: Fn(usize),
    {
        if let Some(ref size) = file.size {
            let size = nat_to_u64(size);
            if size < 1024 * 1800 {
                // upload a small file in one request
                let content = try_read_full(ar, size as u32).await?;
                let mut hasher = Sha3_256::new();
                hasher.update(&content);
                let file = CreateFileInput {
                    content: Some(ByteBuf::from(content.to_vec())),
                    hash: Some(ByteBuf::from(hasher.finalize().to_vec())),
                    ..file
                };
                let res = self
                    .agent
                    .update(&self.bucket, "create_file")
                    .with_arg(Encode!(&file, &self.access_token).map_err(format_error)?)
                    .call_and_wait()
                    .await
                    .map_err(format_error)?;
                let file_output = Decode!(res.as_slice(), Result<CreateFileOutput, String>)
                    .map_err(format_error)??;
                progress(size as usize);
                return Ok(UploadFileChunksResult {
                    id: file_output.id,
                    uploaded: size as usize,
                    uploaded_chunks: BTreeSet::new(),
                    error: None,
                });
            }
        }

        // create file
        let res = self
            .agent
            .update(&self.bucket, "create_file")
            .with_arg(Encode!(&file, &self.access_token).map_err(format_error)?)
            .call_and_wait()
            .await
            .map_err(format_error)?;
        let file_output =
            Decode!(res.as_slice(), Result<CreateFileOutput, String>).map_err(format_error)??;
        let res = self
            .upload_chunks(ar, file_output.id, &BTreeSet::new(), progress)
            .await;
        Ok(res)
    }

    pub async fn upload_chunks<T, F>(
        &self,
        ar: T,
        id: u32,
        exclude_chunks: &BTreeSet<u32>,
        progress: F,
    ) -> UploadFileChunksResult
    where
        T: AsyncRead,
        F: Fn(usize),
    {
        // upload chunks
        let bucket = self.bucket;
        let mut frames = Box::pin(FramedRead::new(ar, ChunksCodec::new(self.chunk_size)));
        let (tx, mut rx) = mpsc::channel::<Result<(), String>>(self.concurrency as usize);
        let output = Arc::new(RwLock::new(UploadFileChunksResult {
            id,
            uploaded: 0usize,
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
                        hasher.update(&chunk);

                        if exclude_chunks.contains(&chunk_index) {
                            let mut r = output.write().await;
                            r.uploaded += chunk_len as usize;
                            progress(r.uploaded);
                            drop(permit);
                            continue;
                        }

                        let agent = self.agent.clone();
                        tokio::spawn(async move {
                            let res = async {
                                let checksum = crc32_with_initial(chunk_index, &chunk);
                                let args = Encode!(
                                    &UpdateFileChunkInput {
                                        id,
                                        chunk_index,
                                        content: ByteBuf::from(chunk.to_vec()),
                                    },
                                    &access_token
                                )
                                .map_err(format_error)?;

                                let res = agent
                                    .update(&bucket, "update_file_chunk")
                                    .with_arg(args)
                                    .call_and_wait()
                                    .await
                                    .map_err(format_error)?;
                                let file_output =
                                    Decode!(res.as_slice(), Result<UpdateFileChunkOutput, String>)
                                        .map_err(format_error)??;
                                if file_output.crc32 != checksum {
                                    return Err(format!("checksum mismatch at chunk {}", index));
                                }
                                Ok(())
                            }
                            .await;

                            if res.is_ok() {
                                let mut r = output.write().await;
                                r.uploaded += chunk_len as usize;
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
                    Ok(_) => {
                        progress(output.read().await.uploaded);
                    }
                    Err(err) => return Err(err),
                }
            }

            Ok(())
        };

        let result = async {
            let (hash, _) = futures::future::try_join(uploading_loop, uploading_result).await?;

            // commit file
            let args = Encode!(
                &UpdateFileInput {
                    id,
                    hash: Some(ByteBuf::from(hash.to_vec())),
                    ..Default::default()
                },
                &self.access_token
            )
            .map_err(format_error)?;

            let _ = self
                .agent
                .update(&self.bucket, "update_file")
                .with_arg(args)
                .call_and_wait()
                .await
                .map_err(format_error)?;
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

async fn try_read_full<T: AsyncRead>(ar: T, size: u32) -> Result<Bytes, String> {
    let mut frames = Box::pin(FramedRead::new(ar, ChunksCodec::new(size)));

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
