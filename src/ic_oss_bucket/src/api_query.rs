use ic_cdk::api::management_canister::main::{
    canister_status, CanisterIdRecord, CanisterStatusResponse,
};
use ic_oss_types::{
    bucket::BucketInfo,
    file::{FileChunk, FileInfo},
    folder::{FolderInfo, FolderName},
    format_error,
};
use serde_bytes::{ByteArray, ByteBuf};

use crate::{permission, store, SECONDS};

#[ic_cdk::query]
fn api_version() -> u16 {
    1
}

#[ic_cdk::query]
fn get_bucket_info(_access_token: Option<ByteBuf>) -> Result<BucketInfo, String> {
    // let canister = ic_cdk::id();
    // let ctx = match store::state::with(|s| {
    //     s.read_permission(
    //         ic_cdk::caller(),
    //         &canister,
    //         access_token,
    //         ic_cdk::api::time() / SECONDS,
    //     )
    // }) {
    //     Ok(ctx) => ctx,
    //     Err((_, err)) => {
    //         return Err(err);
    //     }
    // };

    // if !permission::check_bucket_read(&ctx.ps, &canister) {
    //     return Err("permission denied".to_string());
    // }

    Ok(store::state::with(|r| BucketInfo {
        name: r.name.clone(),
        file_id: r.file_id,
        folder_id: r.folder_id,
        max_file_size: r.max_file_size,
        max_folder_depth: r.max_folder_depth,
        max_children: r.max_children,
        max_custom_data_size: r.max_custom_data_size,
        enable_hash_index: r.enable_hash_index,
        status: r.status,
        visibility: r.visibility,
        total_files: store::fs::total_files(),
        total_chunks: store::fs::total_chunks(),
        total_folders: store::fs::total_folders(),
        managers: r.managers.clone(),
        auditors: r.auditors.clone(),
        trusted_ecdsa_pub_keys: r.trusted_ecdsa_pub_keys.clone(),
        trusted_eddsa_pub_keys: r.trusted_eddsa_pub_keys.clone(),
        governance_canister: r.governance_canister,
    }))
}

#[ic_cdk::update]
async fn get_canister_status() -> Result<CanisterStatusResponse, String> {
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.read_permission(
            ic_cdk::caller(),
            &canister,
            None,
            ic_cdk::api::time() / SECONDS,
        )
    }) {
        Ok(ctx) => ctx,
        Err((_, err)) => {
            return Err(err);
        }
    };

    if !permission::check_bucket_read(&ctx.ps, &canister) {
        return Err("permission denied".to_string());
    }

    let (res,) = canister_status(CanisterIdRecord {
        canister_id: ic_cdk::id(),
    })
    .await
    .map_err(format_error)?;
    Ok(res)
}

#[ic_cdk::query]
fn get_file_info(id: u32, access_token: Option<ByteBuf>) -> Result<FileInfo, String> {
    match store::fs::get_file(id) {
        None => Err("file not found".to_string()),
        Some(file) => {
            if !file.read_by_hash(&access_token) {
                let canister = ic_cdk::id();
                let ctx = match store::state::with(|s| {
                    s.read_permission(
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

                if !permission::check_file_read(&ctx.ps, &canister, id, file.parent) {
                    Err("permission denied".to_string())?;
                }
            }

            Ok(file.into_info(id))
        }
    }
}

#[ic_cdk::query]
fn get_file_info_by_hash(
    hash: ByteArray<32>,
    access_token: Option<ByteBuf>,
) -> Result<FileInfo, String> {
    let id = store::fs::get_file_id(&hash).ok_or("file not found")?;

    get_file_info(id, access_token)
}

#[ic_cdk::query]
fn get_file_ancestors(id: u32, access_token: Option<ByteBuf>) -> Result<Vec<FolderName>, String> {
    let ancestors = store::fs::get_file_ancestors(id);
    if let Some(parent) = ancestors.first() {
        let canister = ic_cdk::id();
        let ctx = match store::state::with(|s| {
            s.read_permission(
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

        if !permission::check_file_read(&ctx.ps, &canister, id, parent.id) {
            Err("permission denied".to_string())?;
        }
    }
    Ok(ancestors)
}

#[ic_cdk::query]
fn get_file_chunks(
    id: u32,
    index: u32,
    take: Option<u32>,
    access_token: Option<ByteBuf>,
) -> Result<Vec<FileChunk>, String> {
    match store::fs::get_file(id) {
        None => Err("file not found".to_string()),
        Some(file) => {
            if !file.read_by_hash(&access_token) {
                let canister = ic_cdk::id();
                let ctx = match store::state::with(|s| {
                    s.read_permission(
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

                if file.status < 0 && ctx.role < store::Role::Auditor {
                    Err("file archived".to_string())?;
                }

                if !permission::check_file_read(&ctx.ps, &canister, id, file.parent) {
                    Err("permission denied".to_string())?;
                }
            }

            Ok(store::fs::get_chunks(id, index, take.unwrap_or(8).min(8)))
        }
    }
}

#[ic_cdk::query]
fn list_files(
    parent: u32,
    prev: Option<u32>,
    take: Option<u32>,
    access_token: Option<ByteBuf>,
) -> Result<Vec<FileInfo>, String> {
    let prev = prev.unwrap_or(u32::MAX);
    let take = take.unwrap_or(10).min(100);
    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.read_permission(
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

    if !permission::check_file_list(&ctx.ps, &canister, parent) {
        Err("permission denied".to_string())?;
    }
    Ok(store::fs::list_files(&ctx, parent, prev, take))
}

#[ic_cdk::query]
fn get_folder_info(id: u32, access_token: Option<ByteBuf>) -> Result<FolderInfo, String> {
    match store::fs::get_folder(id) {
        None => Err("folder not found".to_string()),
        Some(meta) => {
            let canister = ic_cdk::id();
            let ctx = match store::state::with(|s| {
                s.read_permission(
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

            if !permission::check_folder_read(&ctx.ps, &canister, id) {
                Err("permission denied".to_string())?;
            }

            Ok(meta.into_info(id))
        }
    }
}

#[ic_cdk::query]
fn get_folder_ancestors(id: u32, access_token: Option<ByteBuf>) -> Result<Vec<FolderName>, String> {
    let ancestors = store::fs::get_folder_ancestors(id);
    if !ancestors.is_empty() {
        let canister = ic_cdk::id();
        let ctx = match store::state::with(|s| {
            s.read_permission(
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

        if !permission::check_folder_read(&ctx.ps, &canister, id) {
            Err("permission denied".to_string())?;
        }
    }
    Ok(ancestors)
}

#[ic_cdk::query]
fn list_folders(
    parent: u32,
    prev: Option<u32>,
    take: Option<u32>,
    access_token: Option<ByteBuf>,
) -> Result<Vec<FolderInfo>, String> {
    let prev = prev.unwrap_or(u32::MAX);
    let take = take.unwrap_or(10).min(100);

    let canister = ic_cdk::id();
    let ctx = match store::state::with(|s| {
        s.read_permission(
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

    if !permission::check_folder_list(&ctx.ps, &canister, parent) {
        Err("permission denied".to_string())?;
    }
    Ok(store::fs::list_folders(&ctx, parent, prev, take))
}
