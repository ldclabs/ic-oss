use candid::Principal;
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

mod api_admin;
mod api_http;
mod api_init;
mod api_query;
mod api_update;
mod permission;
mod store;

use api_init::CanisterArgs;
use ic_oss_types::{bucket::*, file::*, folder::*, ByteN};

const MILLISECONDS: u64 = 1_000_000;
const SECONDS: u64 = 1_000_000_000;

static ANONYMOUS: Principal = Principal::anonymous();

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
