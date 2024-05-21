use candid::Nat;
use ic_oss_types::file::{
    CreateFileInput, CreateFileOutput, UpdateFileChunkInput, UpdateFileChunkOutput,
    UpdateFileInput, UpdateFileOutput, MAX_CHUNK_SIZE,
};
use serde_bytes::ByteBuf;

use crate::{is_controller_or_manager, store, unwrap_hash, unwrap_trap, MILLISECONDS};

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

    if let Some(size) = input.size {
        let max_size = store::state::max_file_size();
        if size > max_size {
            ic_cdk::trap(&format!("file size exceeds the limit {}", max_size));
        }
    }

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let id = unwrap_trap(
        store::fs::add_file(store::FileMetadata {
            name: input.name,
            content_type: input.content_type,
            hash: unwrap_hash(input.hash),
            created_at: now_ms,
            ..Default::default()
        }),
        "failed to add file",
    );
    let mut output = CreateFileOutput {
        id,
        chunks_crc32: Vec::new(),
        created_at: Nat::from(now_ms),
    };

    if let Some(content) = input.content {
        for (i, chunk) in content.chunks(MAX_CHUNK_SIZE as usize).enumerate() {
            let (_, crc32) = unwrap_trap(
                store::fs::update_chunk(id, i as u32, now_ms, chunk.to_vec()),
                "failed to update chunk",
            );
            output.chunks_crc32.push(crc32);
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

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    unwrap_trap(
        store::fs::update_file(input.id, |metadata| {
            if let Some(name) = input.name {
                metadata.name = name;
            }
            if let Some(content_type) = input.content_type {
                metadata.content_type = content_type;
            }
            if input.hash.is_some() {
                metadata.hash = unwrap_hash(input.hash);
            }
        }),
        "update file failed",
    );

    Ok(UpdateFileOutput {
        updated_at: Nat::from(now_ms),
    })
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn update_file_chunk(
    input: UpdateFileChunkInput,
    _access_token: Option<ByteBuf>,
) -> Result<UpdateFileChunkOutput, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let (_, crc32) = unwrap_trap(
        store::fs::update_chunk(
            input.id,
            input.chunk_index,
            now_ms,
            input.content.into_vec(),
        ),
        "failed to add update chunk",
    );

    Ok(UpdateFileChunkOutput {
        crc32,
        updated_at: Nat::from(now_ms),
    })
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn delete_file(id: u32, _access_token: Option<ByteBuf>) -> Result<(), String> {
    store::fs::delete_file(id).map_err(|err| ic_cdk::trap(&err))
}
