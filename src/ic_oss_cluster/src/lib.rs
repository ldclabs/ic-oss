use candid::Principal;
use ic_oss_types::cwt::Token;
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

mod api_admin;
mod ecdsa;
mod init;
mod store;

use crate::init::ChainArgs;

static ANONYMOUS: Principal = Principal::anonymous();
const SECONDS: u64 = 1_000_000_000;

#[ic_cdk::query]
fn get_state(_access_token: Option<ByteBuf>) -> Result<store::State, ()> {
    let s = store::state::with(|s| s.clone());
    Ok(s)
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
