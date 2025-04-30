use candid::Principal;
use ic_cdk::management_canister as mgt;
use serde_bytes::{ByteArray, ByteBuf};
use std::collections::BTreeSet;

mod api_admin;
mod api_http;
mod api_init;
mod api_query;
mod api_update;
mod permission;
mod store;

use api_init::CanisterArgs;
use ic_oss_types::{bucket::*, file::*, folder::*};

const MILLISECONDS: u64 = 1_000_000;
const SECONDS: u64 = 1_000_000_000;

static ANONYMOUS: Principal = Principal::anonymous();

fn is_controller() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if ic_cdk::api::is_controller(&caller) || store::state::is_controller(&caller) {
        Ok(())
    } else {
        Err("user is not a controller".to_string())
    }
}

pub fn validate_principals(principals: &BTreeSet<Principal>) -> Result<(), String> {
    if principals.is_empty() {
        return Err("principals cannot be empty".to_string());
    }
    if principals.contains(&ANONYMOUS) {
        return Err("anonymous user is not allowed".to_string());
    }
    Ok(())
}

ic_cdk::export_candid!();
