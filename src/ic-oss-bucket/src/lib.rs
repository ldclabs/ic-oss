use candid::Principal;
use ic_http_certification::HttpRequest;
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

mod api_admin;
mod api_http;
mod api_init;
mod api_query;
mod api_update;
mod store;

use api_http::*;
use api_init::CanisterArgs;
use ic_oss_types::{file::*, folder::*};

const MILLISECONDS: u64 = 1_000_000;

static ANONYMOUS: Principal = Principal::anonymous();

// pub fn unwrap_trap<T, E: std::fmt::Debug>(res: Result<T, E>, msg: &str) -> T {
//     match res {
//         Ok(v) => v,
//         Err(err) => ic_cdk::trap(&format!("{}, {:?}", msg, err)),
//     }
// }

fn is_controller() -> Result<(), String> {
    let caller = ic_cdk::caller();
    if ic_cdk::api::is_controller(&caller) {
        Ok(())
    } else {
        Err("user is not a controller".to_string())
    }
}

fn is_controller_or_manager() -> Result<(), String> {
    let caller = ic_cdk::caller();
    if ic_cdk::api::is_controller(&caller) || store::state::is_manager(&caller) {
        Ok(())
    } else {
        Err("user is not a controller or manager".to_string())
    }
}

fn is_authenticated() -> Result<(), String> {
    if ic_cdk::caller() == ANONYMOUS {
        Err("anonymous user is not allowed".to_string())
    } else {
        Ok(())
    }
}

ic_cdk::export_candid!();
