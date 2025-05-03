use candid::Principal;
use ic_oss_types::object_store::*;
use object_store::path::Path;
use serde_bytes::ByteBuf;

use crate::store;

#[ic_cdk::query]
fn get_state() -> Result<StateInfo, String> {
    store::state::with(|s| {
        Ok(StateInfo {
            name: s.name.clone(),
            managers: s.managers.clone(),
            auditors: s.auditors.clone(),
            governance_canister: s.governance_canister,
            objects: s.locations.len() as u64,
            next_etag: s.next_etag,
        })
    })
}

#[ic_cdk::query]
fn is_member(member_kind: String, user: Principal) -> Result<bool, String> {
    store::state::with(|s| match member_kind.as_str() {
        "manager" => Ok(s.managers.contains(&user)),
        "auditor" => Ok(s.auditors.contains(&user)),
        _ => Err(format!("invalid member kind: {}", member_kind)),
    })
}

#[ic_cdk::update]
fn put_opts(path: String, payload: ByteBuf, opts: PutOptions) -> Result<PutResult> {
    is_writer()?;
    parse_path(&path)?;
    if payload.len() as u64 > MAX_PAYLOAD_SIZE {
        return Err(Error::Precondition {
            path,
            error: format!(
                "payload size {} exceeds max size {}",
                payload.len(),
                MAX_PAYLOAD_SIZE
            ),
        });
    }
    let now_ms = ic_cdk::api::time() / 1000000;
    store::object::put_opts(path, payload, opts, now_ms)
}

#[ic_cdk::update]
fn delete(path: String) -> Result<()> {
    is_writer()?;
    parse_path(&path)?;
    store::object::delete(path)
}

#[ic_cdk::update]
fn copy(from: String, to: String) -> Result<()> {
    is_writer()?;
    if from == to {
        return Err(Error::Precondition {
            path: from,
            error: "location 'to' is equal to 'from'".to_string(),
        });
    }
    parse_path(&from)?;
    parse_path(&to)?;
    store::object::copy(from, to)
}

#[ic_cdk::update]
fn copy_if_not_exists(from: String, to: String) -> Result<()> {
    is_writer()?;
    if from == to {
        return Err(Error::Precondition {
            path: from,
            error: "location 'to' is equal to 'from'".to_string(),
        });
    }
    parse_path(&from)?;
    parse_path(&to)?;
    store::object::copy_if_not_exists(from, to)
}

#[ic_cdk::update]
fn rename(from: String, to: String) -> Result<()> {
    is_writer()?;
    if from == to {
        return Err(Error::Precondition {
            path: from,
            error: "location 'to' is equal to 'from'".to_string(),
        });
    }
    parse_path(&from)?;
    parse_path(&to)?;
    store::object::rename(from, to)
}

#[ic_cdk::update]
fn rename_if_not_exists(from: String, to: String) -> Result<()> {
    is_writer()?;
    if from == to {
        return Err(Error::Precondition {
            path: from,
            error: "location 'to' is equal to 'from'".to_string(),
        });
    }
    parse_path(&from)?;
    parse_path(&to)?;
    store::object::rename_if_not_exists(from, to)
}

#[ic_cdk::update]
fn create_multipart(path: String) -> Result<MultipartId> {
    is_writer()?;
    parse_path(&path)?;

    store::object::create_multipart(path)
}

#[ic_cdk::update]
fn put_part(path: String, id: MultipartId, part_idx: u64, payload: ByteBuf) -> Result<PartId> {
    is_writer()?;
    parse_path(&path)?;

    if part_idx >= MAX_PARTS {
        return Err(Error::Precondition {
            path,
            error: format!(
                "part index {} exceeds max index {}",
                part_idx,
                MAX_PARTS - 1
            ),
        });
    }

    if payload.len() as u64 > CHUNK_SIZE {
        return Err(Error::Precondition {
            path,
            error: format!(
                "part size {} exceeds max size {}",
                payload.len(),
                CHUNK_SIZE
            ),
        });
    }
    store::object::put_part(path, id, part_idx as u32, payload)
}

#[ic_cdk::update]
fn complete_multipart(path: String, id: MultipartId, opts: PutMultipartOpts) -> Result<PutResult> {
    is_writer()?;
    parse_path(&path)?;
    let now_ms = ic_cdk::api::time() / 1000000;
    store::object::complete_multipart(path, id, opts, now_ms)
}

#[ic_cdk::update]
fn abort_multipart(path: String, id: MultipartId) -> Result<()> {
    is_writer()?;
    parse_path(&path)?;
    store::object::abort_multipart(path, id)
}

#[ic_cdk::query]
fn get_part(path: String, part_idx: u64) -> Result<ByteBuf> {
    is_reader()?;
    parse_path(&path)?;
    if part_idx > MAX_PARTS {
        return Err(Error::Precondition {
            path,
            error: format!(
                "part index {} exceeds max index {}",
                part_idx,
                MAX_PARTS - 1
            ),
        });
    }

    store::object::get_part(path, part_idx as u32)
}

#[ic_cdk::query]
fn get_opts(path: String, opts: GetOptions) -> Result<GetResult> {
    is_reader()?;
    parse_path(&path)?;
    store::object::get_opts(path, opts)
}

#[ic_cdk::query]
fn get_ranges(path: String, ranges: Vec<(u64, u64)>) -> Result<Vec<ByteBuf>> {
    is_reader()?;
    parse_path(&path)?;
    store::object::get_ranges(path, ranges)
}

#[ic_cdk::query]
fn head(path: String) -> Result<ObjectMeta> {
    is_reader()?;
    parse_path(&path)?;
    store::object::head(path)
}

#[ic_cdk::query]
fn list(prefix: Option<String>) -> Result<Vec<ObjectMeta>> {
    is_reader()?;
    let prefix = match prefix {
        Some(prefix) => parse_path(&prefix)?,
        None => Path::default(),
    };
    store::object::list(prefix)
}

#[ic_cdk::query]
fn list_with_offset(prefix: Option<String>, offset: String) -> Result<Vec<ObjectMeta>> {
    is_reader()?;
    let prefix = match prefix {
        Some(prefix) => parse_path(&prefix)?,
        None => Path::default(),
    };
    let offset = parse_path(&offset)?;
    store::object::list_with_offset(prefix, offset)
}

#[ic_cdk::query]
fn list_with_delimiter(prefix: Option<String>) -> Result<ListResult> {
    is_reader()?;
    let prefix = match prefix {
        Some(prefix) => parse_path(&prefix)?,
        None => Path::default(),
    };
    store::object::list_with_delimiter(prefix)
}

fn is_writer() -> Result<()> {
    let caller = ic_cdk::api::msg_caller();
    if store::state::is_writer(&caller) {
        Ok(())
    } else {
        Err(Error::PermissionDenied {
            path: "".to_string(),
            error: "no write permission".to_string(),
        })
    }
}

fn is_reader() -> Result<()> {
    let caller = ic_cdk::api::msg_caller();
    if store::state::is_reader(&caller) {
        Ok(())
    } else {
        Err(Error::PermissionDenied {
            path: "".to_string(),
            error: "no read permission".to_string(),
        })
    }
}

fn parse_path(path: &str) -> Result<Path> {
    Path::parse(path).map_err(|_| Error::InvalidPath {
        path: path.to_string(),
    })
}
