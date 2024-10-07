// Usage example:
// ic_oss_can::ic_oss_fs!();
//
#[macro_export]
macro_rules! ic_oss_fs {
    () => {
        #[allow(dead_code)]
        pub mod fs {
            use candid::Principal;
            use ciborium::{from_reader, into_writer};
            use ic_oss_types::file::{FileChunk, FileInfo, UpdateFileInput, CHUNK_SIZE};
            use serde_bytes::ByteBuf;
            use std::{cell::RefCell, collections::BTreeSet};

            use super::FS_CHUNKS_STORE;
            use $crate::types::*;

            thread_local! {
                static FS_METADATA: RefCell<Files> = RefCell::new(Files::default());
            }

            fn with_mut<R>(f: impl FnOnce(&mut Files) -> R) -> R {
                FS_METADATA.with(|r| f(&mut r.borrow_mut()))
            }

            pub fn set_max_file_size(size: u64) {
                with_mut(|r| r.max_file_size = size);
            }

            pub fn set_visibility(visibility: u8) {
                with_mut(|r| r.visibility = if visibility == 0 { 0 } else { 1 });
            }

            pub fn set_managers(managers: BTreeSet<Principal>) {
                with_mut(|r| r.managers = managers);
            }

            pub fn is_manager(caller: &Principal) -> bool {
                with(|r| r.managers.contains(caller))
            }

            pub fn with<R>(f: impl FnOnce(&Files) -> R) -> R {
                FS_METADATA.with(|r| f(&r.borrow()))
            }

            pub fn load() {
                FS_CHUNKS_STORE.with(|r| {
                    FS_METADATA.with(|h| {
                        if let Some(data) = r.borrow().get(&FileId(0, 0)) {
                            let v: Files = from_reader(&data.0[..])
                                .expect("failed to decode FS_METADATA data");
                            *h.borrow_mut() = v;
                        }
                    });
                });
            }

            pub fn save() {
                FS_METADATA.with(|h| {
                    FS_CHUNKS_STORE.with(|r| {
                        let mut buf = vec![];
                        into_writer(&(*h.borrow()), &mut buf)
                            .expect("failed to encode FS_METADATA data");
                        r.borrow_mut().insert(FileId(0, 0), Chunk(buf));
                    });
                });
            }

            pub fn total_chunks() -> u64 {
                FS_CHUNKS_STORE.with(|r| r.borrow().len())
            }

            pub fn get_file(id: u32) -> Option<FileMetadata> {
                if id == 0 {
                    return None;
                }
                FS_METADATA.with(|r| r.borrow().files.get(&id).cloned())
            }

            pub fn list_files(prev: u32, take: u32) -> Vec<FileInfo> {
                FS_METADATA.with(|r| r.borrow().list_files(prev, take))
            }

            pub fn add_file(file: FileMetadata) -> Result<u32, String> {
                with_mut(|r| {
                    if file.size > r.max_file_size {
                        Err(format!("file size exceeds limit: {}", r.max_file_size))?;
                    }

                    let id = r.file_id;
                    if id == u32::MAX {
                        Err("file id overflow".to_string())?;
                    }

                    r.file_id = id.saturating_add(1);
                    r.files.insert(id, file);
                    Ok(id)
                })
            }

            pub fn update_file(change: UpdateFileInput, now_ms: u64) -> Result<(), String> {
                if change.id == 0 {
                    Err("invalid file id".to_string())?;
                }
                with_mut(|r| match r.files.get_mut(&change.id) {
                    None => Err(format!("file not found: {}", change.id)),
                    Some(file) => {
                        if file.size != file.filled {
                            Err("file not fully uploaded".to_string())?;
                        }

                        if let Some(name) = change.name {
                            file.name = name;
                        }
                        if let Some(content_type) = change.content_type {
                            file.content_type = content_type;
                        }
                        if change.hash.is_some() {
                            file.hash = change.hash;
                        }
                        file.updated_at = now_ms;
                        Ok(())
                    }
                })
            }

            pub fn get_chunk(id: u32, chunk_index: u32) -> Option<FileChunk> {
                if id == 0 {
                    return None;
                }
                FS_CHUNKS_STORE.with(|r| {
                    r.borrow()
                        .get(&FileId(id, chunk_index))
                        .map(|v| FileChunk(chunk_index, ByteBuf::from(v.0)))
                })
            }

            pub fn get_full_chunks(id: u32) -> Result<Vec<u8>, String> {
                if id == 0 {
                    Err("invalid file id".to_string())?;
                }
                let (size, chunks) = with(|r| match r.files.get(&id) {
                    None => Err(format!("file not found: {}", id)),
                    Some(file) => {
                        if file.size != file.filled {
                            return Err("file not fully uploaded".to_string());
                        }
                        Ok((file.size, file.chunks))
                    }
                })?;

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
                if file_id == 0 {
                    Err("invalid file id".to_string())?;
                }

                if chunk.is_empty() {
                    Err("empty chunk".to_string())?;
                }

                if chunk.len() > CHUNK_SIZE as usize {
                    Err(format!(
                        "chunk size too large, max size is {} bytes",
                        CHUNK_SIZE
                    ))?;
                }

                with_mut(|r| match r.files.get_mut(&file_id) {
                    None => Err(format!("file not found: {}", file_id)),
                    Some(file) => {
                        file.updated_at = now_ms;
                        file.filled += chunk.len() as u64;
                        if file.filled > r.max_file_size {
                            Err(format!("file size exceeds limit: {}", r.max_file_size))?;
                        }

                        match FS_CHUNKS_STORE.with(|r| {
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

                        Ok(filled)
                    }
                })
            }

            pub fn delete_file(id: u32) -> Result<bool, String> {
                if id == 0 {
                    Err("invalid file id".to_string())?;
                }

                with_mut(|r| match r.files.remove(&id) {
                    Some(file) => {
                        FS_CHUNKS_STORE.with(|r| {
                            let mut fs_data = r.borrow_mut();
                            for i in 0..file.chunks {
                                fs_data.remove(&FileId(id, i));
                            }
                        });
                        Ok(true)
                    }
                    None => Ok(false),
                })
            }
        }

        pub mod api {
            use ic_oss_types::file::*;
            use serde_bytes::ByteBuf;

            use super::fs;
            use $crate::types::*;

            #[ic_cdk::query]
            fn list_files(
                _parent: u32,
                prev: Option<u32>,
                take: Option<u32>,
                _access_token: Option<ByteBuf>,
            ) -> Result<Vec<FileInfo>, String> {
                let caller = ic_cdk::api::caller();
                let max_prev = fs::with(|r| {
                    if r.visibility == 0 && !r.managers.contains(&caller) {
                        Err("permission denied".to_string())?;
                    }
                    Ok::<u32, String>(r.file_id)
                })?;
                let prev = prev.unwrap_or(max_prev).min(max_prev);
                let take = take.unwrap_or(10).min(100);
                Ok(fs::list_files(prev, take))
            }

            #[ic_cdk::update]
            fn create_file(
                input: CreateFileInput,
                _access_token: Option<ByteBuf>,
            ) -> Result<CreateFileOutput, String> {
                input.validate()?;
                let caller = ic_cdk::api::caller();
                if !fs::is_manager(&caller) {
                    Err("permission denied".to_string())?;
                }

                let size = input.size.unwrap_or(0);
                let now_ms = ic_cdk::api::time() / MILLISECONDS;
                let res: Result<CreateFileOutput, String> = {
                    let id = fs::add_file(FileMetadata {
                        name: input.name,
                        content_type: input.content_type,
                        size,
                        hash: input.hash,
                        created_at: now_ms,
                        updated_at: now_ms,
                        ..Default::default()
                    })?;

                    if let Some(content) = input.content {
                        if size > 0 && content.len() != size as usize {
                            Err("content size mismatch".to_string())?;
                        }

                        for (i, chunk) in content.chunks(CHUNK_SIZE as usize).enumerate() {
                            fs::update_chunk(id, i as u32, now_ms, chunk.to_vec())?;
                        }

                        if input.status.is_some() {
                            fs::update_file(
                                UpdateFileInput {
                                    id,
                                    status: input.status,
                                    ..Default::default()
                                },
                                now_ms,
                            )?;
                        }
                    }

                    Ok(CreateFileOutput {
                        id,
                        created_at: now_ms,
                    })
                };

                match res {
                    Ok(output) => Ok(output),
                    Err(err) => {
                        // trap and rollback state
                        ic_cdk::trap(&format!("create file failed: {}", err));
                    }
                }
            }

            #[ic_cdk::update]
            fn update_file_info(
                input: UpdateFileInput,
                _access_token: Option<ByteBuf>,
            ) -> Result<UpdateFileOutput, String> {
                input.validate()?;
                let caller = ic_cdk::api::caller();
                if !fs::is_manager(&caller) {
                    Err("permission denied".to_string())?;
                }

                let now_ms = ic_cdk::api::time() / MILLISECONDS;
                fs::update_file(input, now_ms)?;
                Ok(UpdateFileOutput { updated_at: now_ms })
            }

            #[ic_cdk::update]
            fn update_file_chunk(
                input: UpdateFileChunkInput,
                _access_token: Option<ByteBuf>,
            ) -> Result<UpdateFileChunkOutput, String> {
                let caller = ic_cdk::api::caller();
                if !fs::is_manager(&caller) {
                    Err("permission denied".to_string())?;
                }

                let now_ms = ic_cdk::api::time() / MILLISECONDS;
                let filled = fs::update_chunk(
                    input.id,
                    input.chunk_index,
                    now_ms,
                    input.content.into_vec(),
                )?;

                Ok(UpdateFileChunkOutput {
                    filled,
                    updated_at: now_ms,
                })
            }

            #[ic_cdk::update]
            fn delete_file(id: u32, _access_token: Option<ByteBuf>) -> Result<bool, String> {
                let caller = ic_cdk::api::caller();
                if !fs::is_manager(&caller) {
                    Err("permission denied".to_string())?;
                }

                fs::delete_file(id)
            }
        }
    };
}
