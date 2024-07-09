use candid::CandidType;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::file::valid_file_name;

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FolderInfo {
    pub id: u32,
    pub parent: u32, // 0: root
    pub name: String,
    pub created_at: u64,        // unix timestamp in milliseconds
    pub updated_at: u64,        // unix timestamp in milliseconds
    pub status: i8,             // -1: archived; 0: readable and writable; 1: readonly
    pub files: BTreeSet<u32>,   // length <= max_children
    pub folders: BTreeSet<u32>, // length <= max_children
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FolderName {
    pub id: u32,
    pub name: String,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct CreateFolderInput {
    pub parent: u32,
    pub name: String,
}

impl CreateFolderInput {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_file_name(&self.name) {
            return Err("invalid folder name".to_string());
        }

        Ok(())
    }
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct CreateFolderOutput {
    pub id: u32,
    pub created_at: u64,
}

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateFolderInput {
    pub id: u32,
    pub name: Option<String>,
    pub status: Option<i8>, // when set to 1, the file must be fully filled, and hash must be provided
}

impl UpdateFolderInput {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.name {
            if !valid_file_name(name) {
                return Err("invalid folder name".to_string());
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
pub struct UpdateFolderOutput {
    pub updated_at: u64,
}
