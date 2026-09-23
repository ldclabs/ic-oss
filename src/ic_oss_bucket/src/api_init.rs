use candid::{CandidType, Principal};
use ic_oss_types::bucket::UpdateBucketInput;
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

#[ic_cdk::init]
fn init(args: Option<CanisterArgs>) {
    match args {
        Some(CanisterArgs::Init(args)) => {
            // zero values keep the defaults
            let input = UpdateBucketInput {
                name: (!args.name.is_empty()).then_some(args.name),
                max_file_size: (args.max_file_size > 0).then_some(args.max_file_size),
                max_folder_depth: (args.max_folder_depth > 0).then_some(args.max_folder_depth),
                max_children: (args.max_children > 0).then_some(args.max_children),
                max_custom_data_size: (args.max_custom_data_size > 0)
                    .then_some(args.max_custom_data_size),
                enable_hash_index: Some(args.enable_hash_index),
                visibility: Some(args.visibility.min(1)),
                ..Default::default()
            };
            update_bucket(input);
            store::state::with_mut(|b| {
                b.file_id = args.file_id;
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
            update_bucket(UpdateBucketInput {
                max_file_size: args.max_file_size,
                max_folder_depth: args.max_folder_depth,
                max_children: args.max_children,
                max_custom_data_size: args.max_custom_data_size,
                enable_hash_index: args.enable_hash_index,
                ..Default::default()
            });
            if let Some(governance_canister) = args.governance_canister {
                store::state::with_mut(|s| s.governance_canister = Some(governance_canister));
            }
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

fn update_bucket(input: UpdateBucketInput) {
    if let Err(err) = store::state::validate_update(&input) {
        ic_cdk::trap(&err);
    }
    store::state::apply_update(input);
}
