use candid::Principal;
use ic_oss_types::{cluster::ClusterInfo, cwt::Token};
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

mod api_admin;
mod api_auth;
mod ecdsa;
mod init;
mod store;

use crate::init::ChainArgs;

static ANONYMOUS: Principal = Principal::anonymous();
const SECONDS: u64 = 1_000_000_000;

#[ic_cdk::query]
fn get_cluster_info() -> Result<ClusterInfo, String> {
    Ok(store::state::with(|r| ClusterInfo {
        name: r.name.clone(),
        ecdsa_key_name: r.ecdsa_key_name.clone(),
        ecdsa_token_public_key: r.ecdsa_token_public_key.clone(),
        token_expiration: r.token_expiration,
        managers: r.managers.clone(),
    }))
}

fn is_controller() -> Result<(), String> {
    let caller = ic_cdk::caller();
    if ic_cdk::api::is_controller(&caller) {
        Ok(())
    } else {
        Err("user is not a controller".to_string())
    }
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
