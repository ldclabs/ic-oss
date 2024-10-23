use ic_oss_types::{file::*, folder::*, to_cbor_bytes};
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

use crate::{permission, store, MILLISECONDS, SECONDS};

#[ic_cdk::update]
fn create_file(
    input: CreateFileInput,
    access_token: Option<ByteBuf>,
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

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    if !permission::check_file_create(&ctx.ps, &canister, input.parent) {
        Err("permission denied".to_string())?;
    }

    let res: Result<CreateFileOutput, String> = {
        let id = store::fs::add_file(store::FileMetadata {
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
        })?;

        if let Some(content) = input.content {
            if size > 0 && content.len() != size as usize {
                Err("content size mismatch".to_string())?;
            }

            for (i, chunk) in content.chunks(CHUNK_SIZE as usize).enumerate() {
                store::fs::update_chunk(id, i as u32, now_ms, chunk.to_vec(), |_| Ok(()))?;
            }

            if input.status.is_some() {
                store::fs::update_file(
                    UpdateFileInput {
                        id,
                        status: input.status,
                        ..Default::default()
                    },
                    now_ms,
                    |_| Ok(()),
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
    access_token: Option<ByteBuf>,
) -> Result<UpdateFileOutput, String> {
    input.validate()?;

    store::state::with(|s| {
        if input.size.unwrap_or_default() > s.max_file_size {
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

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    let id = input.id;
    let res = store::fs::update_file(input, now_ms, |file| {
        match permission::check_file_update(&ctx.ps, &canister, id, file.parent) {
            true => Ok(()),
            false => Err("permission denied".to_string()),
        }
    });

    match res {
        Ok(_) => Ok(UpdateFileOutput { updated_at: now_ms }),
        Err(err) => {
            // trap and rollback state
            ic_cdk::trap(&format!("update file info failed: {}", err));
        }
    }
}

#[ic_cdk::update]
fn update_file_chunk(
    input: UpdateFileChunkInput,
    access_token: Option<ByteBuf>,
) -> Result<UpdateFileChunkOutput, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(
            ic_cdk::caller(),
            &canister,
            access_token,
            ic_cdk::api::time() / SECONDS,
        )
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    let id = input.id;
    let res = store::fs::update_chunk(
        input.id,
        input.chunk_index,
        now_ms,
        input.content.into_vec(),
        |file| match permission::check_file_update(&ctx.ps, &canister, id, file.parent) {
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
            ic_cdk::trap(&format!("update file chunk failed: {}", err));
        }
    }
}

#[ic_cdk::update]
fn move_file(input: MoveInput, access_token: Option<ByteBuf>) -> Result<UpdateFileOutput, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    if !permission::check_file_delete(&ctx.ps, &canister, input.from) {
        Err("permission denied".to_string())?;
    }

    if !permission::check_file_create(&ctx.ps, &canister, input.to) {
        Err("permission denied".to_string())?;
    }

    store::fs::move_file(input.id, input.from, input.to, now_ms)?;
    Ok(UpdateFileOutput { updated_at: now_ms })
}

#[ic_cdk::update]
fn delete_file(id: u32, access_token: Option<ByteBuf>) -> Result<bool, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    store::fs::delete_file(id, now_ms, |file| {
        match permission::check_file_delete(&ctx.ps, &canister, file.parent) {
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
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    if !permission::check_file_delete(&ctx.ps, &canister, parent) {
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
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    if !permission::check_folder_create(&ctx.ps, &canister, input.parent) {
        Err("permission denied".to_string())?;
    }

    let res: Result<CreateFolderOutput, String> = {
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

#[ic_cdk::update]
fn update_folder_info(
    input: UpdateFolderInput,
    access_token: Option<ByteBuf>,
) -> Result<UpdateFolderOutput, String> {
    input.validate()?;

    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    let id = input.id;
    store::fs::update_folder(
        input,
        now_ms,
        |folder| match permission::check_folder_update(&ctx.ps, &canister, id, folder.parent) {
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
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    if !permission::check_folder_delete(&ctx.ps, &canister, input.from) {
        Err("permission denied".to_string())?;
    }

    if !permission::check_folder_create(&ctx.ps, &canister, input.to) {
        Err("permission denied".to_string())?;
    }

    store::fs::move_folder(input.id, input.from, input.to, now_ms)?;
    Ok(UpdateFolderOutput { updated_at: now_ms })
}

#[ic_cdk::update]
fn delete_folder(id: u32, access_token: Option<ByteBuf>) -> Result<bool, String> {
    let now_ms = ic_cdk::api::time() / MILLISECONDS;
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.write_permission(ic_cdk::caller(), &canister, access_token, now_ms / 1000)
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    store::fs::delete_folder(id, now_ms, |folder| {
        match permission::check_folder_delete(&ctx.ps, &canister, folder.parent) {
            true => Ok(()),
            false => Err("permission denied".to_string()),
        }
    })
}
