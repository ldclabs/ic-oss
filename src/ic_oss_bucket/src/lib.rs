use candid::Principal;
use ic_cdk::api::management_canister::main::CanisterStatusResponse;
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
    let caller = ic_cdk::caller();
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

#[cfg(all(
    target_arch = "wasm32",
    target_vendor = "unknown",
    target_os = "unknown"
))]
/// A getrandom implementation that always fails
pub fn always_fail(_buf: &mut [u8]) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}

#[cfg(all(
    target_arch = "wasm32",
    target_vendor = "unknown",
    target_os = "unknown"
))]
getrandom::register_custom_getrandom!(always_fail);

ic_cdk::export_candid!();
