use cbor2::serialized_size;
use ic_oss_types::{file::*, folder::*, MapValue};
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

use crate::{permission, store, MILLISECONDS};

fn validate_file_limits(size: u64, custom: Option<&MapValue>) -> Result<(), String> {
    store::state::with(|state| {
        if size > state.max_file_size {
            return Err(format!(
                "file size exceeds the limit {}",
                state.max_file_size
            ));
        }

        if let Some(custom) = custom {
            let len = serialized_size(custom)
                .map_err(|err| format!("failed to measure custom data: {err}"))?;
            if len > state.max_custom_data_size as u64 {
                return Err(format!(
                    "custom data size exceeds the limit {}",
                    state.max_custom_data_size
                ));
            }
        }
        Ok(())
    })
}

fn write_context(
    access_token: Option<ByteBuf>,
    now_ms: u64,
) -> Result<permission::Context, String> {
    permission::authorize_write(access_token, now_ms / 1000).map_err(|(_, err)| err)
}

#[ic_cdk::update]
fn create_file(
    input: CreateFileInput,
    access_token: Option<ByteBuf>,
) -> Result<CreateFileOutput, String> {
    input.validate()?;

    let size = input.size.unwrap_or(0);
    validate_file_limits(size, input.custom.as_ref())?;

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    if !permission::check_file_create(&ctx, input.parent) {
        Err("permission denied".to_string())?;
    }

    // `create_file` validates everything before it touches any store, so a
    // failure leaves no partial state to roll back and can be returned as-is.
    let id = store::fs::create_file(
        store::FileMetadata {
            parent: input.parent,
            name: input.name,
            content_type: input.content_type,
            size,
            hash: input.hash,
            dek: input.dek,
            custom: input.custom,
            created_at: now_ms,
            updated_at: now_ms,
            ..Default::default()
        },
        input.content.map(ByteBuf::into_vec),
        input.status,
    )?;

    Ok(CreateFileOutput {
        id,
        created_at: now_ms,
    })
}

#[ic_cdk::update]
fn update_file_info(
    input: UpdateFileInput,
    access_token: Option<ByteBuf>,
) -> Result<UpdateFileOutput, String> {
    input.validate()?;

    validate_file_limits(input.size.unwrap_or_default(), input.custom.as_ref())?;

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    let id = input.id;
    let res = store::fs::update_file(input, now_ms, |file| {
        match permission::check_file_update(&ctx, id, file.parent) {
            true => Ok(()),
            false => Err("permission denied".to_string()),
        }
    });

    match res {
        Ok(_) => Ok(UpdateFileOutput { updated_at: now_ms }),
        Err(err) => {
            // trap and rollback state
            ic_cdk::trap(format!("update file info failed: {}", err));
        }
    }
}

#[ic_cdk::update]
fn update_file_chunk(
    input: UpdateFileChunkInput,
    access_token: Option<ByteBuf>,
) -> Result<UpdateFileChunkOutput, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    let id = input.id;
    let res = store::fs::update_chunk(
        input.id,
        input.chunk_index,
        now_ms,
        input.content.into_vec(),
        |file| match permission::check_file_update(&ctx, id, file.parent) {
            true => Ok(()),
            false => Err("permission denied".to_string()),
        },
    );

    match res {
        Ok(filled) => Ok(UpdateFileChunkOutput {
            filled,
            updated_at: now_ms,
        }),
        Err(err) => {
            // trap and rollback state
            ic_cdk::trap(format!("update file chunk failed: {}", err));
        }
    }
}

#[ic_cdk::update]
fn move_file(input: MoveInput, access_token: Option<ByteBuf>) -> Result<UpdateFileOutput, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    if !permission::check_file_delete(&ctx, input.from) {
        Err("permission denied".to_string())?;
    }

    if !permission::check_file_create(&ctx, input.to) {
        Err("permission denied".to_string())?;
    }

    store::fs::move_file(input.id, input.from, input.to, now_ms)?;
    Ok(UpdateFileOutput { updated_at: now_ms })
}

#[ic_cdk::update]
fn delete_file(id: u32, access_token: Option<ByteBuf>) -> Result<bool, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    store::fs::delete_file(id, now_ms, |file| {
        match permission::check_file_delete(&ctx, file.parent) {
            true => Ok(()),
            false => Err("permission denied".to_string()),
        }
    })
}

#[ic_cdk::update]
fn batch_delete_subfiles(
    parent: u32,
    ids: BTreeSet<u32>,
    access_token: Option<ByteBuf>,
) -> Result<Vec<u32>, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    if !permission::check_file_delete(&ctx, parent) {
        Err("permission denied".to_string())?;
    }

    store::fs::batch_delete_subfiles(parent, ids, now_ms)
}

#[ic_cdk::update]
fn create_folder(
    input: CreateFolderInput,
    access_token: Option<ByteBuf>,
) -> Result<CreateFolderOutput, String> {
    input.validate()?;
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    if !permission::check_folder_create(&ctx, input.parent) {
        Err("permission denied".to_string())?;
    }

    // `add_folder` validates everything before it touches any store, so a
    // failure leaves no partial state to roll back and can be returned as-is.
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
}

#[ic_cdk::update]
fn update_folder_info(
    input: UpdateFolderInput,
    access_token: Option<ByteBuf>,
) -> Result<UpdateFolderOutput, String> {
    input.validate()?;

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    let id = input.id;
    store::fs::update_folder(
        input,
        now_ms,
        |folder| match permission::check_folder_update(&ctx, id, folder.parent) {
            true => Ok(()),
            false => Err("permission denied".to_string()),
        },
    )?;

    Ok(UpdateFolderOutput { updated_at: now_ms })
}

#[ic_cdk::update]
fn move_folder(
    input: MoveInput,
    access_token: Option<ByteBuf>,
) -> Result<UpdateFolderOutput, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    if !permission::check_folder_delete(&ctx, input.from) {
        Err("permission denied".to_string())?;
    }

    if !permission::check_folder_create(&ctx, input.to) {
        Err("permission denied".to_string())?;
    }

    store::fs::move_folder(input.id, input.from, input.to, now_ms)?;
    Ok(UpdateFolderOutput { updated_at: now_ms })
}

#[ic_cdk::update]
fn delete_folder(id: u32, access_token: Option<ByteBuf>) -> Result<bool, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let ctx = write_context(access_token, now_ms)?;

    store::fs::delete_folder(id, now_ms, |folder| {
        match permission::check_folder_delete(&ctx, folder.parent) {
            true => Ok(()),
            false => Err("permission denied".to_string()),
        }
    })
}
