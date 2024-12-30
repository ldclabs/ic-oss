use candid::{utils::ArgumentEncoder, CandidType, Nat, Principal};
use ic_cdk::api::management_canister::main::{
    CanisterSettings, CanisterStatusResponse, UpdateSettingsArgument,
};
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
mod ecdsa;
mod init;
mod schnorr;
mod store;

use crate::init::ChainArgs;

static ANONYMOUS: Principal = Principal::anonymous();
// NNS Cycles Minting Canister: "rkp4c-7iaaa-aaaaa-aaaca-cai"
static CMC_PRINCIPAL: Principal = Principal::from_slice(&[0, 0, 0, 0, 0, 0, 0, 4, 1, 1]);
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

fn is_controller_or_manager_or_committer() -> Result<(), String> {
    let caller = ic_cdk::caller();
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

async fn call<In, Out>(id: Principal, method: &str, args: In, cycles: u128) -> Result<Out, String>
where
    In: ArgumentEncoder + Send,
    Out: candid::CandidType + for<'a> candid::Deserialize<'a>,
{
    let (res,): (Out,) = ic_cdk::api::call::call_with_payment128(id, method, args, cycles)
        .await
        .map_err(|(code, msg)| {
            format!(
                "failed to call {} on {:?}, code: {}, message: {}",
                method, &id, code as u32, msg
            )
        })?;
    Ok(res)
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
    pub settings: Option<CanisterSettings>,
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
    settings: Option<CanisterSettings>,
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
    let res: Result<Principal, CreateCanisterOutput> =
        call(CMC_PRINCIPAL, "create_canister", (arg,), cycles).await?;
    res.map_err(|err| format!("failed to create canister, error: {:?}", err))
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
