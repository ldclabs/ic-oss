use base64::{engine::general_purpose, Engine};
use candid::CandidType;
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use std::path::Path;
use url::Url;

use crate::{format_error, MapValue};

pub const CHUNK_SIZE: u32 = 256 * 1024;
pub const MAX_FILE_SIZE: u64 = 384 * 1024 * 1024 * 1024; // 384GB
pub const MAX_FILE_SIZE_PER_CALL: u64 = 1024 * 2048; // should less than 2MB

pub static CUSTOM_KEY_BY_HASH: &str = "by_hash";

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FileInfo {
    pub id: u32,
    pub parent: u32, // 0: root
    pub name: String,
    pub content_type: String,
    pub size: u64,
    pub filled: u64,
    pub created_at: u64, // unix timestamp in milliseconds
    pub updated_at: u64, // unix timestamp in milliseconds
    pub chunks: u32,
    pub status: i8, // -1: archived; 0: readable and writable; 1: readonly
    pub hash: Option<ByteArray<32>>,
    pub dek: Option<ByteBuf>, // // Data Encryption Key that encrypted by BYOK or vetKey in COSE_Encrypt0
    pub custom: Option<MapValue>, // custom metadata
    pub ex: Option<MapValue>, // External Resource info
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct CreateFileInput {
    pub parent: u32,
    pub name: String,
    pub content_type: String,
    pub size: Option<u64>, // if provided, can be used to detect the file is fully filled
    pub content: Option<ByteBuf>, // should <= 1024 * 1024 * 2 - 1024
    pub status: Option<i8>, // when set to 1, the file must be fully filled, and hash must be provided
    pub hash: Option<ByteArray<32>>, // recommend sha3 256
    pub dek: Option<ByteBuf>,
    pub custom: Option<MapValue>,
}

pub fn valid_file_name(name: &str) -> bool {
    if name.is_empty() || name.trim() != name || name.len() > 96 {
        return false;
    }

    let p = Path::new(name);
    p.file_name() == Some(p.as_os_str())
}

pub fn valid_file_parent(parent: &str) -> bool {
    if parent.is_empty() || parent == "/" {
        return true;
    }

    if !parent.starts_with('/') {
        return false;
    }

    for name in parent[1..].split('/') {
        if !valid_file_name(name) {
            return false;
        }
    }
    true
}

impl CreateFileInput {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_file_name(&self.name) {
            return Err("invalid file name".to_string());
        }

        if self.content_type.is_empty() {
            return Err("content_type cannot be empty".to_string());
        }

        if let Some(content) = &self.content {
            if content.is_empty() {
                return Err("content cannot be empty".to_string());
            }
        }

        if let Some(status) = self.status {
            if !(0i8..=1i8).contains(&status) {
                return Err("status should be 0 or 1".to_string());
            }
        }
        Ok(())
    }
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct CreateFileOutput {
    pub id: u32,
    pub created_at: u64,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateFileInput {
    pub id: u32,
    pub name: Option<String>,
    pub content_type: Option<String>,
    pub status: Option<i8>, // when set to 1, the file must be fully filled, and hash must be provided
    pub size: Option<u64>, // if provided and smaller than file.filled, the file content will be deleted and should be refilled
    pub hash: Option<ByteArray<32>>,
    pub custom: Option<MapValue>,
}

impl UpdateFileInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.name {
            if !valid_file_name(name) {
                return Err("invalid file name".to_string());
            }
        }
        if let Some(content_type) = &self.content_type {
            if content_type.is_empty() {
                return Err("content_type cannot be empty".to_string());
            }
        }
        if let Some(status) = self.status {
            if !(-1i8..=1i8).contains(&status) {
                return Err("status should be -1, 0 or 1".to_string());
            }
        }
        Ok(())
    }
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateFileOutput {
    pub updated_at: u64,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateFileChunkInput {
    pub id: u32,
    pub chunk_index: u32,
    pub content: ByteBuf, // should be in (0, 1024 * 256]
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateFileChunkOutput {
    pub filled: u64,
    pub updated_at: u64,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct FileChunk(pub u32, pub ByteBuf);

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct MoveInput {
    pub id: u32,
    pub from: u32,
    pub to: u32,
}

#[derive(Debug)]
pub struct UrlFileParam {
    pub file: u32,
    pub hash: Option<ByteArray<32>>,
    pub token: Option<ByteBuf>,
    pub name: Option<String>,
    pub inline: bool,
}

impl UrlFileParam {
    pub fn from_url(req_url: &str) -> Result<Self, String> {
        let url = if req_url.starts_with('/') {
            Url::parse(format!("http://localhost{}", req_url).as_str())
        } else {
            Url::parse(req_url)
        };
        let url = url.map_err(|_| format!("invalid url: {}", req_url))?;
        let mut path_segments = url
            .path_segments()
            .ok_or_else(|| format!("invalid url path: {}", req_url))?;

        let mut param = match path_segments.next() {
            Some("f") => Self {
                file: path_segments
                    .next()
                    .unwrap_or_default()
                    .parse()
                    .map_err(|_| "invalid file id")?,
                hash: None,
                token: None,
                name: None,
                inline: false,
            },
            Some("h") => {
                let val = path_segments.next().unwrap_or_default();
                let data = hex::decode(val).map_err(format_error)?;
                let hash: [u8; 32] = data.try_into().map_err(format_error)?;
                let hash = ByteArray::from(hash);
                Self {
                    file: 0,
                    hash: Some(hash),
                    token: None,
                    name: None,
                    inline: false,
                }
            }
            _ => return Err(format!("invalid url path: {}", req_url)),
        };

        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "token" => {
                    let data = general_purpose::URL_SAFE_NO_PAD
                        .decode(value.as_bytes())
                        .map_err(|_| format!("failed to decode base64 token from {}", value))?;
                    param.token = Some(ByteBuf::from(data));
                    break;
                }
                "filename" => {
                    param.name = Some(value.to_string());
                }
                "inline" => {
                    param.inline = true;
                }
                _ => {}
            }
        }

        // use the last path segment as filename if provided
        if let Some(filename) = path_segments.next() {
            param.name = Some(filename.to_string());
        }

        Ok(param)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_file_name_works() {
        assert!(valid_file_name("file"));
        assert!(valid_file_name("file.txt"));
        assert!(valid_file_name(".file.txt"));
        assert!(valid_file_name("file.txt."));
        assert!(valid_file_name("..."));

        assert!(!valid_file_name(""));
        assert!(!valid_file_name("."));
        assert!(!valid_file_name(".."));
        assert!(!valid_file_name(" file.txt"));
        assert!(!valid_file_name("/file.txt"));
        assert!(!valid_file_name("./file.txt"));
        assert!(!valid_file_name("test/file.txt"));
        assert!(!valid_file_name("file.txt/"));
    }

    #[test]
    fn valid_file_parent_works() {
        assert!(valid_file_parent(""));
        assert!(valid_file_parent("/"));
        assert!(valid_file_parent("/file"));
        assert!(valid_file_parent("/file.txt"));
        assert!(valid_file_parent("/file/.txt"));

        assert!(!valid_file_parent("file.txt"));
        assert!(!valid_file_parent("//file.txt"));
        assert!(!valid_file_parent("/./file.txt"));
        assert!(!valid_file_parent("/../file.txt"));
        assert!(!valid_file_parent("test/file.txt"));
        assert!(!valid_file_parent("/file/"));
    }
}
