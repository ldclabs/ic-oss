use candid::CandidType;
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
    name: String,
    file_id: u32,
    max_file_size: u64,
    max_folder_depth: u8,
    max_children: u16,
    visibility: u8, // 0: private; 1: public
    max_custom_data_size: u16,
    enable_hash_index: bool,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct UpgradeArgs {
    name: Option<String>,
    max_file_size: Option<u64>,
    max_folder_depth: Option<u8>,
    max_children: Option<u16>,
    visibility: Option<u8>, // 0: private; 1: public
    max_custom_data_size: Option<u16>,
    enable_hash_index: Option<bool>,
}

impl UpgradeArgs {
    fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.name {
            if name.is_empty() {
                return Err("name cannot be empty".to_string());
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
        if let Some(visibility) = self.visibility {
            if visibility != 0 && visibility != 1 {
                return Err("visibility should be 0 or 1".to_string());
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
    match args.expect("Init args is missing") {
        CanisterArgs::Init(args) => {
            store::state::with_mut(|b| {
                b.name = if args.name.is_empty() {
                    "default".to_string()
                } else {
                    args.name
                };
                b.file_id = args.file_id;
                b.max_file_size = if args.max_file_size == 0 {
                    MAX_FILE_SIZE
                } else {
                    args.max_file_size
                };
                b.max_folder_depth = if args.max_folder_depth == 0 {
                    10
                } else {
                    args.max_folder_depth
                };
                b.max_children = if args.max_children == 0 {
                    1000
                } else {
                    args.max_children
                };
                b.visibility = if args.visibility == 0 { 0 } else { 1 };
                b.max_custom_data_size = if args.max_custom_data_size == 0 {
                    1024 * 4
                } else {
                    args.max_custom_data_size
                };
                b.enable_hash_index = args.enable_hash_index;
            });
        }
        CanisterArgs::Upgrade(_) => {
            ic_cdk::trap(
                "Cannot initialize the canister with an Upgrade args. Please provide an Init args.",
            );
        }
    }

    store::state::save();
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
                if let Some(name) = args.name {
                    s.name = name;
                }
                if let Some(max_file_size) = args.max_file_size {
                    s.max_file_size = max_file_size;
                }
                if let Some(max_folder_depth) = args.max_folder_depth {
                    s.max_folder_depth = max_folder_depth;
                }
                if let Some(max_children) = args.max_children {
                    s.max_children = max_children;
                }
                if let Some(visibility) = args.visibility {
                    s.visibility = visibility;
                }
                if let Some(max_custom_data_size) = args.max_custom_data_size {
                    s.max_custom_data_size = max_custom_data_size;
                }
                if let Some(enable_hash_index) = args.enable_hash_index {
                    s.enable_hash_index = enable_hash_index;
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
