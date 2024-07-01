use ic_oss_types::{
    crc32,
    file::{
        CreateFileInput, CreateFileOutput, UpdateFileChunkInput, UpdateFileChunkOutput,
        UpdateFileInput, UpdateFileOutput, MAX_CHUNK_SIZE,
    },
    to_cbor_bytes,
};
use serde_bytes::ByteBuf;

use crate::{is_controller_or_manager, store, unwrap_trap, MILLISECONDS};

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn create_file(
    input: CreateFileInput,
    _access_token: Option<ByteBuf>,
) -> Result<CreateFileOutput, String> {
    // use trap to make the update fail.

    unwrap_trap(input.validate(), "invalid CreateFileInput");
    if input.parent != 0 {
        ic_cdk::trap("parent directory not found");
    }

    let size = input.size.unwrap_or(0);
    store::state::with(|s| {
        if size > s.max_file_size {
            ic_cdk::trap(&format!("file size exceeds the limit {}", s.max_file_size));
        }
        if let Some(ref custom) = input.custom {
            let len = to_cbor_bytes(custom).len();
            if len > s.max_custom_data_size as usize {
                ic_cdk::trap(&format!(
                    "custom data size exceeds the limit {}",
                    s.max_custom_data_size
                ));
            }
        }
    });

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let id = unwrap_trap(
        store::fs::add_file(store::FileMetadata {
            name: input.name,
            content_type: input.content_type,
            size,
            hash: input.hash,
            custom: input.custom,
            er: input.er,
            created_at: now_ms,
            ..Default::default()
        }),
        "failed to add file",
    );
    let output = CreateFileOutput {
        id,
        created_at: now_ms,
    };

    if let Some(content) = input.content {
        if let Some(checksum) = input.crc32 {
            if crc32(&content) != checksum {
                ic_cdk::trap("crc32 checksum mismatch");
            }
        }
        if size > 0 && content.len() != size as usize {
            ic_cdk::trap("content size mismatch");
        }

        for (i, chunk) in content.chunks(MAX_CHUNK_SIZE as usize).enumerate() {
            let _ = unwrap_trap(
                store::fs::update_chunk(id, i as u32, now_ms, chunk.to_vec()),
                "failed to update chunk",
            );
        }

        if let Some(status) = input.status {
            unwrap_trap(
                store::fs::update_file(id, |metadata| {
                    metadata.status = status;
                }),
                "update file failed",
            );
        }
    }

    Ok(output)
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn update_file_info(
    input: UpdateFileInput,
    _access_token: Option<ByteBuf>,
) -> Result<UpdateFileOutput, String> {
    unwrap_trap(input.validate(), "invalid UpdateFileInput");

    if let Some(_parent) = input.parent {
        ic_cdk::trap("parent directory not found");
    }

    store::state::with(|s| {
        if let Some(ref custom) = input.custom {
            let len = to_cbor_bytes(custom).len();
            if len > s.max_custom_data_size as usize {
                ic_cdk::trap(&format!(
                    "custom data size exceeds the limit {}",
                    s.max_custom_data_size
                ));
            }
        }
    });

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    unwrap_trap(
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
            if input.er.is_some() {
                metadata.er = input.er;
            }
        }),
        "update file failed",
    );

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
            ic_cdk::trap("crc32 checksum mismatch");
        }
    }

    let filled = unwrap_trap(
        store::fs::update_chunk(
            input.id,
            input.chunk_index,
            now_ms,
            input.content.into_vec(),
        ),
        "failed to add update chunk",
    );

    Ok(UpdateFileChunkOutput {
        filled,
        updated_at: now_ms,
    })
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn delete_file(id: u32, _access_token: Option<ByteBuf>) -> Result<(), String> {
    store::fs::delete_file(id).map_err(|err| ic_cdk::trap(&err))
}
