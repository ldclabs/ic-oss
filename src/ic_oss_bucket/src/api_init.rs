use candid::{CandidType, Principal};
use ic_oss_types::file::MAX_FILE_SIZE;
use serde::Deserialize;

use crate::store;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum CanisterArgs {
    Init(InitArgs),
    Upgrade(UpgradeArgs),
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct InitArgs {
    name: String,              // bucket name
    file_id: u32,              // the first file id, default is 0
    max_file_size: u64,        // in bytes, default is 384GB
    max_folder_depth: u8,      // default is 10
    max_children: u16, //  maximum number of subfolders and subfiles in a folder., default is 1000
    max_custom_data_size: u16, // in bytes, default is 4KB
    enable_hash_index: bool, // if enabled, indexing will be built using file hash, allowing files to be read by their hash and preventing duplicate hash for files. default is false
    visibility: u8,          // 0: private; 1: public, can be accessed by anyone, default is 0
    governance_canister: Option<Principal>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct UpgradeArgs {
    max_file_size: Option<u64>,
    max_folder_depth: Option<u8>,
    max_children: Option<u16>,
    max_custom_data_size: Option<u16>,
    enable_hash_index: Option<bool>,
    governance_canister: Option<Principal>,
}

impl UpgradeArgs {
    fn validate(&self) -> Result<(), String> {
        if let Some(max_file_size) = self.max_file_size {
            if max_file_size == 0 {
                return Err("max_file_size should be greater than 0".to_string());
            }
            if max_file_size >= MAX_FILE_SIZE {
                return Err(format!(
                    "max_file_size should be smaller than or equal to {}",
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
        Ok(())
    }
}

#[ic_cdk::init]
fn init(args: Option<CanisterArgs>) {
    match args {
        Some(CanisterArgs::Init(args)) => {
            store::state::with_mut(|b| {
                if !args.name.is_empty() {
                    b.name = args.name
                };
                b.file_id = args.file_id;
                if args.max_file_size > 0 {
                    b.max_file_size = args.max_file_size
                };
                if args.max_folder_depth > 0 {
                    b.max_folder_depth = args.max_folder_depth
                };
                if args.max_children > 0 {
                    b.max_children = args.max_children
                };
                if args.visibility > 0 {
                    b.visibility = 1
                };
                if args.max_custom_data_size > 0 {
                    b.max_custom_data_size = args.max_custom_data_size
                };
                b.enable_hash_index = args.enable_hash_index;
                b.governance_canister = args.governance_canister;
            });
        }
        Some(CanisterArgs::Upgrade(_)) => {
            ic_cdk::trap(
                "Cannot initialize the canister with an Upgrade args. Please provide an Init args.",
            );
        }
        None => {}
    }

    store::state::init_http_certified_data();
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    store::state::save();
}

#[ic_cdk::post_upgrade]
fn post_upgrade(args: Option<CanisterArgs>) {
    store::state::load();
    match args {
        Some(CanisterArgs::Upgrade(args)) => {
            if let Err(err) = args.validate() {
                ic_cdk::trap(&err);
            }

            store::state::with_mut(|s| {
                if let Some(max_file_size) = args.max_file_size {
                    s.max_file_size = max_file_size;
                }
                if let Some(max_folder_depth) = args.max_folder_depth {
                    s.max_folder_depth = max_folder_depth;
                }
                if let Some(max_children) = args.max_children {
                    s.max_children = max_children;
                }

                if let Some(max_custom_data_size) = args.max_custom_data_size {
                    s.max_custom_data_size = max_custom_data_size;
                }
                if let Some(enable_hash_index) = args.enable_hash_index {
                    s.enable_hash_index = enable_hash_index;
                }
                if let Some(governance_canister) = args.governance_canister {
                    s.governance_canister = Some(governance_canister);
                }
            });
        }
        Some(CanisterArgs::Init(_)) => {
            ic_cdk::trap(
                "Cannot upgrade the canister with an Init args. Please provide an Upgrade args.",
            );
        }
        _ => {}
    }

    store::state::init_http_certified_data();
}
