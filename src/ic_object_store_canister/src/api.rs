use candid::Principal;
use ic_oss_types::object_store::*;
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
    validate_distinct_paths(&from, &to)?;
    store::object::copy(from, to)
}

#[ic_cdk::update]
fn copy_if_not_exists(from: String, to: String) -> Result<()> {
    is_writer()?;
    validate_distinct_paths(&from, &to)?;
    store::object::copy_if_not_exists(from, to)
}

#[ic_cdk::update]
fn rename(from: String, to: String) -> Result<()> {
    is_writer()?;
    validate_distinct_paths(&from, &to)?;
    store::object::rename(from, to)
}

#[ic_cdk::update]
fn rename_if_not_exists(from: String, to: String) -> Result<()> {
    is_writer()?;
    validate_distinct_paths(&from, &to)?;
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

    let part_idx = validate_part_idx(&path, part_idx)?;

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
    store::object::put_part(path, id, part_idx, payload)
}

#[ic_cdk::update]
fn complete_multipart(
    path: String,
    id: MultipartId,
    opts: PutMultipartOptions,
) -> Result<PutResult> {
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
    let part_idx = validate_part_idx(&path, part_idx)?;

    store::object::get_part(path, part_idx)
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
    store::object::list(parse_prefix(prefix)?)
}

#[ic_cdk::query]
fn list_with_offset(prefix: Option<String>, offset: String) -> Result<Vec<ObjectMeta>> {
    is_reader()?;
    let prefix = parse_prefix(prefix)?;
    let offset = parse_path(&offset)?.to_string();
    store::object::list_with_offset(prefix, offset)
}

#[ic_cdk::query]
fn list_with_delimiter(prefix: Option<String>) -> Result<ListResult> {
    is_reader()?;
    store::object::list_with_delimiter(parse_prefix(prefix)?)
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

fn parse_path(path: &str) -> Result<&str> {
    let stripped = path.strip_prefix('/').unwrap_or(path);
    if stripped.is_empty() {
        return Ok(stripped);
    }

    let stripped = stripped.strip_suffix('/').unwrap_or(stripped);
    for segment in stripped.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(Error::InvalidPath {
                path: path.to_string(),
            });
        }
    }
    Ok(stripped)
}

fn parse_prefix(prefix: Option<String>) -> Result<String> {
    match prefix {
        Some(prefix) => Ok(parse_path(&prefix)?.to_string()),
        None => Ok(String::new()),
    }
}

fn validate_distinct_paths(from: &str, to: &str) -> Result<()> {
    if from == to {
        return Err(Error::Precondition {
            path: from.to_string(),
            error: "location 'to' is equal to 'from'".to_string(),
        });
    }
    parse_path(from)?;
    parse_path(to)?;
    Ok(())
}

fn validate_part_idx(path: &str, part_idx: u64) -> Result<u32> {
    if part_idx >= MAX_PARTS {
        return Err(Error::Precondition {
            path: path.to_string(),
            error: format!("part index {part_idx} exceeds max index {}", MAX_PARTS - 1),
        });
    }
    Ok(part_idx as u32)
}

#[cfg(test)]
mod tests {
    use super::parse_path;
    use object_store::path::Path;

    #[test]
    fn path_validation_matches_object_store() {
        let cases = [
            "",
            "/",
            "a",
            "/a",
            "a/",
            "/a/",
            "a/b",
            "a//b",
            "//",
            ".",
            "..",
            "a/./b",
            "a/../b",
            "a%2Fb/c",
            "你好/世界",
            "a\0b",
            "a\nb",
        ];

        for case in cases {
            let expected = Path::parse(case);
            let actual = parse_path(case);
            assert_eq!(actual.is_ok(), expected.is_ok(), "{case:?}");
            if let (Ok(actual), Ok(expected)) = (actual, expected) {
                assert_eq!(actual, expected.as_ref(), "{case:?}");
            }
        }
    }
}
