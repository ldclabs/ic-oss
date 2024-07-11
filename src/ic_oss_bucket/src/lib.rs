use candid::Principal;
use ic_http_certification::HttpRequest;
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

mod api_admin;
mod api_http;
mod api_init;
mod api_query;
mod api_update;
mod permission;
mod store;

use api_http::*;
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

ic_cdk::export_candid!();
