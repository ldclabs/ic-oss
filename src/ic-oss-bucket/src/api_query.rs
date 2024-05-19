use candid::Nat;
use ic_oss_types::file::FileInfo;
use serde_bytes::ByteBuf;

use crate::store;

#[ic_cdk::query]
fn api_version() -> u16 {
    1
}

#[ic_cdk::query]
fn bucket_info(_access_token: Option<ByteBuf>) -> Result<store::Bucket, ()> {
    Ok(store::state::with(|r| r.clone()))
}

#[ic_cdk::query]
fn get_file_info(id: u32, _access_token: Option<ByteBuf>) -> Result<FileInfo, String> {
    match store::fs::get_file(id) {
        Some(meta) => Ok(FileInfo {
            id,
            parent: meta.parent,
            name: meta.name,
            content_type: meta.content_type,
            size: Nat::from(meta.size),
            filled: Nat::from(meta.filled),
            created_at: Nat::from(meta.created_at),
            updated_at: Nat::from(meta.updated_at),
            chunks: meta.chunks,
            hash: meta.hash.map(ByteBuf::from),
            status: meta.status,
        }),
        None => Err("file not found".to_string()),
    }
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
