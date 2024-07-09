use ic_oss_types::{crc32, file::*, folder::*, to_cbor_bytes};
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

use crate::{is_controller_or_manager, store, MILLISECONDS};

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn create_file(
    input: CreateFileInput,
    _access_token: Option<ByteBuf>,
) -> Result<CreateFileOutput, String> {
    input.validate()?;

    let size = input.size.unwrap_or(0);
    store::state::with(|s| {
        if size > s.max_file_size {
            return Err(format!("file size exceeds the limit {}", s.max_file_size));
        }
        if let Some(ref custom) = input.custom {
            let len = to_cbor_bytes(custom).len();
            if len > s.max_custom_data_size as usize {
                return Err(format!(
                    "custom data size exceeds the limit {}",
                    s.max_custom_data_size
                ));
            }
        }
        Ok(())
    })?;

    let res: Result<CreateFileOutput, String> = {
        let now_ms = ic_cdk::api::time() / MILLISECONDS;
        let id = store::fs::add_file(store::FileMetadata {
            parent: input.parent,
            name: input.name,
            content_type: input.content_type,
            size,
            hash: input.hash,
            custom: input.custom,
            created_at: now_ms,
            updated_at: now_ms,
            ..Default::default()
        })?;

        if let Some(content) = input.content {
            if let Some(checksum) = input.crc32 {
                if crc32(&content) != checksum {
                    Err("crc32 checksum mismatch".to_string())?;
                }
            }
            if size > 0 && content.len() != size as usize {
                Err("content size mismatch".to_string())?;
            }

            for (i, chunk) in content.chunks(MAX_CHUNK_SIZE as usize).enumerate() {
                store::fs::update_chunk(id, i as u32, now_ms, chunk.to_vec())?;
            }

            if let Some(status) = input.status {
                store::fs::update_file(id, |metadata| {
                    metadata.status = status;
                })?;
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

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn update_file_info(
    input: UpdateFileInput,
    _access_token: Option<ByteBuf>,
) -> Result<UpdateFileOutput, String> {
    input.validate()?;

    store::state::with(|s| {
        if let Some(ref custom) = input.custom {
            let len = to_cbor_bytes(custom).len();
            if len > s.max_custom_data_size as usize {
                return Err(format!(
                    "custom data size exceeds the limit {}",
                    s.max_custom_data_size
                ));
            }
        }
        Ok(())
    })?;

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    store::fs::update_file(input.id, |metadata| {
        if let Some(name) = input.name {
            metadata.name = name;
        }
        if let Some(content_type) = input.content_type {
            metadata.content_type = content_type;
        }
        if let Some(status) = input.status {
            metadata.status = status;
        }
        if input.hash.is_some() {
            metadata.hash = input.hash;
        }
        if input.custom.is_some() {
            metadata.custom = input.custom;
        }
        metadata.updated_at = now_ms;
    })?;

    Ok(UpdateFileOutput { updated_at: now_ms })
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn update_file_chunk(
    input: UpdateFileChunkInput,
    _access_token: Option<ByteBuf>,
) -> Result<UpdateFileChunkOutput, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    if let Some(checksum) = input.crc32 {
        if crc32(&input.content) != checksum {
            Err("crc32 checksum mismatch".to_string())?;
        }
    }

    let filled = store::fs::update_chunk(
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

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn move_file(input: MoveInput, _access_token: Option<ByteBuf>) -> Result<UpdateFileOutput, String> {
    let updated_at = ic_cdk::api::time() / MILLISECONDS;
    store::fs::move_file(input.id, input.from, input.to, updated_at)?;
    Ok(UpdateFileOutput { updated_at })
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn delete_file(id: u32, _access_token: Option<ByteBuf>) -> Result<bool, String> {
    store::fs::delete_file(id, ic_cdk::api::time() / MILLISECONDS)
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn batch_delete_subfiles(
    parent: u32,
    ids: BTreeSet<u32>,
    _access_token: Option<ByteBuf>,
) -> Result<Vec<u32>, String> {
    store::fs::batch_delete_subfiles(parent, ids, ic_cdk::api::time() / MILLISECONDS)
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn create_folder(
    input: CreateFolderInput,
    _access_token: Option<ByteBuf>,
) -> Result<CreateFolderOutput, String> {
    input.validate()?;

    let res: Result<CreateFolderOutput, String> = {
        let now_ms = ic_cdk::api::time() / MILLISECONDS;
        let id = store::fs::add_folder(store::FolderMetadata {
            parent: input.parent,
            name: input.name,
            created_at: now_ms,
            updated_at: now_ms,
            ..Default::default()
        })?;

        Ok(CreateFolderOutput {
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

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn update_folder_info(
    input: UpdateFolderInput,
    _access_token: Option<ByteBuf>,
) -> Result<UpdateFolderOutput, String> {
    input.validate()?;

    let updated_at = ic_cdk::api::time() / MILLISECONDS;
    store::fs::update_folder(input.id, |metadata| {
        if let Some(name) = input.name {
            metadata.name = name;
        }
        if let Some(status) = input.status {
            metadata.status = status;
        }
        metadata.updated_at = updated_at;
    })?;

    Ok(UpdateFolderOutput { updated_at })
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn move_folder(
    input: MoveInput,
    _access_token: Option<ByteBuf>,
) -> Result<UpdateFolderOutput, String> {
    let updated_at = ic_cdk::api::time() / MILLISECONDS;
    store::fs::move_folder(input.id, input.from, input.to, updated_at)?;
    Ok(UpdateFolderOutput { updated_at })
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn delete_folder(id: u32, _access_token: Option<ByteBuf>) -> Result<bool, String> {
    store::fs::delete_folder(id, ic_cdk::api::time() / MILLISECONDS)
}
