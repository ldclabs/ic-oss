use candid::{Nat, Principal};
use ic_oss_types::{
    cluster::{AddWasmInput, BucketDeploymentInfo, ClusterInfo, DeployWasmInput, WasmInfo},
    cose::Token,
};
use serde_bytes::{ByteArray, ByteBuf};
use std::collections::{BTreeMap, BTreeSet};

mod api_admin;
mod api_auth;
mod api_query;
mod ecdsa;
mod init;
mod schnorr;
mod store;

use crate::init::ChainArgs;

static ANONYMOUS: Principal = Principal::anonymous();
static TOKEN_KEY_DERIVATION_PATH: &[u8] = b"ic_oss_cluster";
const SECONDS: u64 = 1_000_000_000;
const MILLISECONDS: u64 = 1_000_000;

fn is_controller() -> Result<(), String> {
    let caller = ic_cdk::caller();
    if ic_cdk::api::is_controller(&caller) || store::state::is_controller(&caller) {
        Ok(())
    } else {
        Err("user is not a controller".to_string())
    }
}

fn is_controller_or_manager() -> Result<(), String> {
    let caller = ic_cdk::caller();
    if ic_cdk::api::is_controller(&caller)
        || store::state::is_controller(&caller)
        || store::state::is_manager(&caller)
    {
        Ok(())
    } else {
        Err("user is not a controller or manager".to_string())
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
