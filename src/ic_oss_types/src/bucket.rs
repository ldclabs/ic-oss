use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use std::collections::BTreeSet;

use crate::file::MAX_FILE_SIZE;

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BucketInfo {
    pub name: String,
    pub file_id: u32,
    pub folder_id: u32,
    pub max_file_size: u64,
    pub max_folder_depth: u8,
    pub max_children: u16,
    pub max_custom_data_size: u16,
    pub enable_hash_index: bool,
    pub status: i8,     // -1: archived; 0: readable and writable; 1: readonly
    pub visibility: u8, // 0: private; 1: public
    pub total_files: u64,
    pub total_chunks: u64,
    pub total_folders: u64,
    pub managers: BTreeSet<Principal>, // managers can read and write
    // auditors can read and list even if the bucket is private
    pub auditors: BTreeSet<Principal>,
    // used to verify the request token signed with SECP256K1
    pub trusted_ecdsa_pub_keys: Vec<ByteBuf>,
    // used to verify the request token signed with ED25519
    pub trusted_eddsa_pub_keys: Vec<ByteArray<32>>,
    pub governance_canister: Option<Principal>,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateBucketInput {
    pub name: Option<String>,
    pub max_file_size: Option<u64>,
    pub max_folder_depth: Option<u8>,
    pub max_children: Option<u16>,
    pub max_custom_data_size: Option<u16>,
    pub enable_hash_index: Option<bool>,
    pub status: Option<i8>, // -1: archived; 0: readable and writable; 1: readonly
    pub visibility: Option<u8>, // 0: private; 1: public
    pub trusted_ecdsa_pub_keys: Option<Vec<ByteBuf>>,
    pub trusted_eddsa_pub_keys: Option<Vec<ByteArray<32>>>,
}

impl UpdateBucketInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.name {
            if name.trim().is_empty() {
                return Err("invalid bucket name".to_string());
            }
        }
        if let Some(max_file_size) = self.max_file_size {
            if max_file_size == 0 {
                return Err("max_file_size should be greater than 0".to_string());
            }
            if max_file_size < MAX_FILE_SIZE {
                return Err(format!(
                    "max_file_size should be greater than or equal to {}",
                    MAX_FILE_SIZE
                ));
            }
        }

        if let Some(max_folder_depth) = self.max_folder_depth {
            if max_folder_depth == 0 {
                return Err("max_folder_depth should be greater than 0".to_string());
            }
        }

        if let Some(max_children) = self.max_children {
            if max_children == 0 {
                return Err("max_children should be greater than 0".to_string());
            }
        }

        if let Some(max_custom_data_size) = self.max_custom_data_size {
            if max_custom_data_size == 0 {
                return Err("max_custom_data_size should be greater than 0".to_string());
            }
        }

        if let Some(status) = self.status {
            if !(-1i8..=1i8).contains(&status) {
                return Err("status should be -1, 0 or 1".to_string());
            }
        }

        if let Some(visibility) = self.visibility {
            if visibility != 0 && visibility != 1 {
                return Err("visibility should be 0 or 1".to_string());
            }
        }
        Ok(())
    }
}
