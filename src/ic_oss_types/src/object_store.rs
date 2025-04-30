use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
};

pub const CHUNK_SIZE: u64 = 256 * 1024;
pub const MAX_PARTS: u64 = 1024;
// https://internetcomputer.org/docs/current/developer-docs/smart-contracts/maintain/resource-limits
pub const MAX_PAYLOAD_SIZE: u64 = 2000 * 1024;

// https://github.com/apache/arrow-rs/blob/main/object_store/src/lib.rs

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StateInfo {
    pub name: String,
    pub managers: BTreeSet<Principal>,
    pub auditors: BTreeSet<Principal>,
    pub governance_canister: Option<Principal>,
    pub objects: u64,
    pub next_etag: u64,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum PutMode {
    /// Perform an atomic write operation, overwriting any object present at the provided path
    #[default]
    Overwrite,
    /// Perform an atomic write operation, returning [`Error::AlreadyExists`] if an
    /// object already exists at the provided path
    Create,
    /// Perform an atomic write operation if the current version of the object matches the
    /// provided [`UpdateVersion`], returning [`Error::Precondition`] otherwise
    Update(UpdateVersion),
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UpdateVersion {
    /// The unique identifier for the newly created object
    ///
    /// <https://datatracker.ietf.org/doc/html/rfc9110#name-etag>
    pub e_tag: Option<String>,
    /// A version indicator for the newly created object
    pub version: Option<String>,
}

pub type PutResult = UpdateVersion;

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Attribute {
    ContentDisposition,
    ContentEncoding,
    ContentLanguage,
    ContentType,
    CacheControl,
    Metadata(String),
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct PutOptions {
    /// Configure the [`PutMode`] for this operation
    pub mode: PutMode,
    /// Provide a [`TagSet`] for this object
    ///
    /// Implementations that don't support object tagging should ignore this
    pub tags: String,
    /// Provide a set of [`Attributes`]
    ///
    /// Implementations that don't support an attribute should return an error
    pub attributes: BTreeMap<Attribute, String>,
    /// A nonce with AES256-GCM encryption
    pub aes_nonce: Option<ByteArray<12>>,
    /// A set of tags with AES256-GCM encryption
    /// Each part of the object has its own tag
    pub aes_tags: Option<Vec<ByteArray<16>>>,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct PutMultipartOpts {
    /// Provide a [`TagSet`] for this object
    ///
    /// Implementations that don't support object tagging should ignore this
    pub tags: String,
    /// Provide a set of [`Attributes`]
    ///
    /// Implementations that don't support an attribute should return an error
    pub attributes: BTreeMap<Attribute, String>,
    /// A nonce with AES256-GCM encryption
    pub aes_nonce: Option<ByteArray<12>>,
    /// A set of tags with AES256-GCM encryption
    /// Each part of the object has its own tag
    pub aes_tags: Option<Vec<ByteArray<16>>>,
}

#[derive(CandidType, Default, Clone, Debug, Deserialize, Serialize)]
pub struct GetOptions {
    /// Request will succeed if the `ObjectMeta::e_tag` matches
    /// otherwise returning [`Error::Precondition`]
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc9110#name-if-match>
    ///
    /// Examples:
    ///
    /// ```text
    /// If-Match: "xyzzy"
    /// If-Match: "xyzzy", "r2d2xxxx", "c3piozzzz"
    /// If-Match: *
    /// ```
    pub if_match: Option<String>,
    /// Request will succeed if the `ObjectMeta::e_tag` does not match
    /// otherwise returning [`Error::NotModified`]
    ///
    /// See <https://datatracker.ietf.org/doc/html/rfc9110#section-13.1.2>
    ///
    /// Examples:
    ///
    /// ```text
    /// If-None-Match: "xyzzy"
    /// If-None-Match: "xyzzy", "r2d2xxxx", "c3piozzzz"
    /// If-None-Match: *
    /// ```
    pub if_none_match: Option<String>,
    /// Request will succeed if the object has been modified since
    ///
    /// <https://datatracker.ietf.org/doc/html/rfc9110#section-13.1.3>
    pub if_modified_since: Option<u64>,
    /// Request will succeed if the object has not been modified since
    /// otherwise returning [`Error::Precondition`]
    ///
    /// Some stores, such as S3, will only return `NotModified` for exact
    /// timestamp matches, instead of for any timestamp greater than or equal.
    ///
    /// <https://datatracker.ietf.org/doc/html/rfc9110#section-13.1.4>
    pub if_unmodified_since: Option<u64>,
    /// Request transfer of only the specified range of bytes
    /// otherwise returning [`Error::NotModified`]
    ///
    /// <https://datatracker.ietf.org/doc/html/rfc9110#name-range>
    pub range: Option<GetRange>,
    /// Request a particular object version
    pub version: Option<String>,
    /// Request transfer of no content
    ///
    /// <https://datatracker.ietf.org/doc/html/rfc9110#name-head>
    pub head: bool,
}

impl GetOptions {
    /// Returns an error if the modification conditions on this request are not satisfied
    ///
    /// <https://datatracker.ietf.org/doc/html/rfc7232#section-6>
    pub fn check_preconditions(&self, meta: &ObjectMeta) -> Result<()> {
        // The use of the invalid etag "*" means no ETag is equivalent to never matching
        let etag = meta.e_tag.as_deref().unwrap_or("*");
        let last_modified = meta.last_modified;

        if let Some(m) = &self.if_match {
            if m != "*" && m.split(',').map(str::trim).all(|x| x != etag) {
                return Err(Error::Precondition {
                    path: meta.location.to_string(),
                    error: format!("{etag} does not match {m}"),
                });
            }
        } else if let Some(date) = self.if_unmodified_since {
            if last_modified > date {
                return Err(Error::Precondition {
                    path: meta.location.to_string(),
                    error: format!("{date} < {last_modified}"),
                });
            }
        }

        if let Some(m) = &self.if_none_match {
            if m == "*" || m.split(',').map(str::trim).any(|x| x == etag) {
                return Err(Error::NotModified {
                    path: meta.location.to_string(),
                    error: format!("{etag} matches {m}"),
                });
            }
        } else if let Some(date) = self.if_modified_since {
            if last_modified <= date {
                return Err(Error::NotModified {
                    path: meta.location.to_string(),
                    error: format!("{date} >= {last_modified}"),
                });
            }
        }
        Ok(())
    }
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub enum GetRange {
    /// Request a specific range of bytes
    ///
    /// If the given range is zero-length or starts after the end of the object,
    /// an error will be returned. Additionally, if the range ends after the end
    /// of the object, the entire remainder of the object will be returned.
    /// Otherwise, the exact requested range will be returned.
    Bounded(u64, u64),
    /// Request all bytes starting from a given byte offset
    Offset(u64),
    /// Request up to the last n bytes
    Suffix(u64),
}

impl GetRange {
    /// Convert to a [`Range`] if valid.
    pub fn into_range(self, len: u64) -> Result<Range<u64>, String> {
        match self {
            Self::Bounded(start, end) => {
                if start >= end {
                    return Err(format!(
                        "wanted range starting at {start} and ending at {end}, but start >= end"
                    ));
                }
                if start >= len {
                    Err(format!(
                        "wanted range starting at {start}, but object was only {len} bytes long"
                    ))
                } else if end > len {
                    Ok(start..len)
                } else {
                    Ok(start..end)
                }
            }
            Self::Offset(start) => {
                if start >= len {
                    Err(format!(
                        "wanted range starting at {start}, but object was only {len} bytes long"
                    ))
                } else {
                    Ok(start..len)
                }
            }
            Self::Suffix(n) => Ok(len.saturating_sub(n)..len),
        }
    }
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct GetResult {
    pub payload: ByteBuf,
    pub meta: ObjectMeta,
    pub range: (u64, u64),
    pub attributes: BTreeMap<Attribute, String>,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct ObjectMeta {
    /// The full path to the object
    pub location: String,
    /// The last modified time
    pub last_modified: u64,
    /// The size in bytes of the object
    pub size: u64,
    /// The unique identifier for the object
    ///
    /// <https://datatracker.ietf.org/doc/html/rfc9110#name-etag>
    pub e_tag: Option<String>,
    /// A version indicator for this object
    pub version: Option<String>,
    /// A nonce with AES256-GCM encryption
    pub aes_nonce: Option<ByteArray<12>>,
    /// A set of tags with AES256-GCM encryption
    /// Each part of the object has its own tag
    pub aes_tags: Option<Vec<ByteArray<16>>>,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct ListResult {
    /// Prefixes that are common (like directories)
    pub common_prefixes: Vec<String>,
    /// Object metadata for the listing
    pub objects: Vec<ObjectMeta>,
}

pub type MultipartId = String;

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct PartId {
    /// Id of this part
    pub content_id: String,
}

#[derive(CandidType, Debug, Deserialize, Serialize, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A fallback error type when no variant matches
    #[error("Generic error: {}", error)]
    Generic {
        /// The wrapped error
        error: String,
    },

    /// Error when the object is not found at given location
    #[error("Object at location {} not found", path)]
    NotFound {
        /// The path to file
        path: String,
    },

    /// Error for invalid path
    #[error("Encountered object with invalid path: {}", path)]
    InvalidPath {
        /// The wrapped error
        path: String,
    },

    /// Error when the attempted operation is not supported
    #[error("Operation not supported: {}", error)]
    NotSupported {
        /// The wrapped error
        error: String,
    },

    /// Error when the object already exists
    #[error("Object at location {} already exists", path)]
    AlreadyExists {
        /// The path to the
        path: String,
    },

    /// Error when the required conditions failed for the operation
    #[error("Request precondition failure for path {}: {}", path, error)]
    Precondition {
        /// The path to the file
        path: String,
        /// The wrapped error
        error: String,
    },

    /// Error when the object at the location isn't modified
    #[error("Object at location {} not modified: {}", path, error)]
    NotModified {
        /// The path to the file
        path: String,
        /// The wrapped error
        error: String,
    },

    /// Error when an operation is not implemented
    #[error("Operation not yet implemented.")]
    NotImplemented,

    /// Error when the used credentials don't have enough permission
    /// to perform the requested operation
    #[error(
        "The operation lacked the necessary privileges to complete for path {}: {}",
        path,
        error
    )]
    PermissionDenied {
        /// The path to the file
        path: String,
        /// The wrapped error
        error: String,
    },

    /// Error when the used credentials lack valid authentication
    #[error(
        "The operation lacked valid authentication credentials for path {}: {}",
        path,
        error
    )]
    Unauthenticated {
        /// The path to the file
        path: String,
        /// The wrapped error
        error: String,
    },

    /// Error when a configuration key is invalid for the store used
    #[error("Configuration key: '{}' is not valid", key)]
    UnknownConfigurationKey {
        /// The configuration key used
        key: String,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
