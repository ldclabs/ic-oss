use candid::{Nat, Principal};
use ic_cdk::management_canister as mgt;
use ic_oss_types::{
    cluster::{BucketDeploymentInfo, ClusterInfo, WasmInfo},
    format_error, nat_to_u64,
};
use serde_bytes::ByteArray;
use std::collections::BTreeMap;

use crate::{is_controller_or_manager, store};

#[ic_cdk::query]
fn get_cluster_info() -> Result<ClusterInfo, String> {
    Ok(store::state::get_cluster_info())
}

#[ic_cdk::query]
fn get_bucket_wasm(hash: ByteArray<32>) -> Result<WasmInfo, String> {
    store::wasm::get_wasm(&hash)
        .map(|w| WasmInfo {
            created_at: w.created_at,
            created_by: w.created_by,
            description: w.description,
            wasm: w.wasm,
            hash,
        })
        .ok_or_else(|| "NotFound: wasm not found".to_string())
}

#[ic_cdk::query]
fn get_deployed_buckets() -> Result<Vec<BucketDeploymentInfo>, String> {
    Ok(store::wasm::get_deployed_buckets())
}

#[ic_cdk::query]
fn get_buckets() -> Result<Vec<Principal>, String> {
    store::state::with(|s| Ok(s.bucket_deployed_list.keys().cloned().collect()))
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
async fn get_canister_status(
    canister: Option<Principal>,
) -> Result<mgt::CanisterStatusResult, String> {
    let self_id = ic_cdk::api::canister_self();
    let canister = canister.unwrap_or(self_id);
    if canister != self_id {
        store::state::with(|s| {
            if !s.bucket_deployed_list.contains_key(&canister) {
                return Err("NotFound: bucket not found".to_string());
            }
            Ok(())
        })?;
    }

    let res = mgt::canister_status(&mgt::CanisterStatusArgs {
        canister_id: canister,
    })
    .await
    .map_err(format_error)?;
    Ok(res)
}

#[ic_cdk::query(guard = "is_controller_or_manager")]
fn bucket_deployment_logs(
    prev: Option<Nat>,
    take: Option<Nat>,
) -> Result<Vec<BucketDeploymentInfo>, String> {
    let prev = prev.as_ref().map(nat_to_u64);
    let take = take.as_ref().map(nat_to_u64).unwrap_or(10).min(1000) as usize;
    Ok(store::wasm::bucket_deployment_logs(prev, take))
}

#[ic_cdk::query(guard = "is_controller_or_manager")]
fn get_subject_policies(subject: Principal) -> Result<BTreeMap<Principal, String>, String> {
    store::auth::get_all_policies(&subject)
        .map(|ps| ps.0)
        .ok_or_else(|| "NotFound: subject not found".to_string())
}

#[ic_cdk::query(guard = "is_controller_or_manager")]
fn get_subject_policies_for(subject: Principal, audience: Principal) -> Result<String, String> {
    match store::auth::get_all_policies(&subject) {
        None => Err("NotFound: subject not found".to_string()),
        Some(ps) => {
            ps.0.get(&audience)
                .cloned()
                .ok_or_else(|| "NotFound: policies not found".to_string())
        }
    }
}
