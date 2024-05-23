use ic_oss_types::file::{FileChunk, FileInfo};
use serde_bytes::ByteBuf;

use crate::store;

#[ic_cdk::query]
fn api_version() -> u16 {
    1
}

#[ic_cdk::query]
fn get_bucket_info(_access_token: Option<ByteBuf>) -> Result<store::Bucket, ()> {
    Ok(store::state::with(|r| r.clone()))
}

#[ic_cdk::query]
fn get_file_info(id: u32, _access_token: Option<ByteBuf>) -> Result<FileInfo, String> {
    match store::fs::get_file(id) {
        Some(meta) => Ok(meta.into_info(id)),
        None => Err("file not found".to_string()),
    }
}

#[ic_cdk::query]
fn get_file_info_by_hash(
    hash: ByteBuf,
    _access_token: Option<ByteBuf>,
) -> Result<FileInfo, String> {
    if hash.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", hash.len()));
    }
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash);
    let id = store::fs::get_file_id(&result).ok_or("file not found")?;

    match store::fs::get_file(id) {
        Some(meta) => Ok(meta.into_info(id)),
        None => Err("file not found".to_string()),
    }
}

#[ic_cdk::query]
fn get_file_chunks(
    id: u32,
    index: u32,
    take: Option<u32>,
    _access_token: Option<ByteBuf>,
) -> Result<Vec<FileChunk>, String> {
    Ok(store::fs::get_chunks(id, index, take.unwrap_or(10).min(8)))
}

#[ic_cdk::query]
fn list_files(
    parent: u32,
    prev: Option<u32>,
    take: Option<u32>,
    _access_token: Option<ByteBuf>,
) -> Vec<FileInfo> {
    let max_prev = store::state::with(|s| s.file_id).saturating_add(1);
    let prev = prev.unwrap_or(max_prev).min(max_prev);
    let take = take.unwrap_or(10).min(100);
    store::fs::list_files(parent, prev, take)
}
