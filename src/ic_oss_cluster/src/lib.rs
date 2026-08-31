use candid::{CandidType, Nat, Principal};
use ic_cdk_management_canister as mgt;
use ic_oss_types::{
    cluster::{AddWasmInput, BucketDeploymentInfo, ClusterInfo, DeployWasmInput, WasmInfo},
    cose::Token,
};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use std::collections::{BTreeMap, BTreeSet};

mod api_admin;
mod api_auth;
mod api_query;
mod chain_key;
mod init;
mod store;

use crate::init::ChainArgs;

static ANONYMOUS: Principal = Principal::anonymous();
// NNS Cycles Minting Canister: "rkp4c-7iaaa-aaaaa-aaaca-cai"
static CMC_PRINCIPAL: Principal = Principal::from_slice(&[0, 0, 0, 0, 0, 0, 0, 4, 1, 1]);
const SECONDS: u64 = 1_000_000_000;
const MILLISECONDS: u64 = 1_000_000;

fn is_controller() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if ic_cdk::api::is_controller(&caller) || store::state::is_controller(&caller) {
        Ok(())
    } else {
        Err("user is not a controller".to_string())
    }
}

fn is_controller_or_manager() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if ic_cdk::api::is_controller(&caller)
        || store::state::is_controller(&caller)
        || store::state::is_manager(&caller)
    {
        Ok(())
    } else {
        Err("user is not a controller or manager".to_string())
    }
}

fn is_controller_or_manager_or_committer() -> Result<(), String> {
    let caller = ic_cdk::api::msg_caller();
    if ic_cdk::api::is_controller(&caller)
        || store::state::is_controller(&caller)
        || store::state::is_manager(&caller)
        || store::state::is_committer(&caller)
    {
        Ok(())
    } else {
        Err("user is not a controller or manager or committer".to_string())
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

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub struct SubnetId {
    pub principal_id: String,
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
pub enum SubnetSelection {
    /// Choose a specific subnet
    Subnet { subnet: SubnetId },
    // Skip the SubnetFilter on the CMC SubnetSelection for simplification.
    // https://github.com/dfinity/ic/blob/master/rs/nns/cmc/cmc.did#L35
}

#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize)]
struct CreateCanisterInput {
    pub settings: Option<mgt::CanisterSettings>,
    pub subnet_selection: Option<SubnetSelection>,
    pub subnet_type: Option<String>,
}

/// Error for create_canister.
#[derive(Clone, Eq, PartialEq, Debug, CandidType, Deserialize, Serialize)]
pub enum CreateCanisterOutput {
    Refunded {
        refund_amount: u128,
        create_error: String,
    },
}

async fn create_canister_on(
    subnet: Principal,
    settings: Option<mgt::CanisterSettings>,
    cycles: u128,
) -> Result<Principal, String> {
    let arg = CreateCanisterInput {
        settings,
        subnet_type: None,
        subnet_selection: Some(SubnetSelection::Subnet {
            subnet: SubnetId {
                principal_id: subnet.to_text(),
            },
        }),
    };
    // Canister creation is non-idempotent. Wait unboundedly so a timeout cannot
    // hide a successful creation and leak the newly created canister/cycles.
    let res: Result<Principal, CreateCanisterOutput> = ic_cdk::call::Call::unbounded_wait(
        CMC_PRINCIPAL,
        "create_canister",
    )
    .with_arg(&arg)
    .with_cycles(cycles)
    .await
    .map_err(|err| format!("failed to call create_canister on {CMC_PRINCIPAL}, error: {err:?}"))?
    .candid()
    .map_err(|err| {
        format!("failed to decode create_canister response from {CMC_PRINCIPAL}, error: {err:?}")
    })?;
    res.map_err(|err| format!("failed to create canister, error: {:?}", err))
}

ic_cdk::export_candid!();
