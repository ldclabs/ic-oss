use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

use crate::ByteN;

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize)]
pub struct ClusterInfo {
    pub name: String,
    pub ecdsa_key_name: String,
    pub ecdsa_token_public_key: String,
    pub token_expiration: u64, // in seconds
    pub managers: BTreeSet<Principal>,
    pub subject_authz_total: u64,
    pub bucket_latest_version: ByteN<32>,
    pub bucket_wasm_total: u64,
    pub bucket_deployed_total: u64,
    pub bucket_deployment_logs: u64,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct WasmInfo {
    pub created_at: u64, // in milliseconds
    pub created_by: Principal,
    pub description: String,
    pub wasm: ByteBuf,
    pub hash: ByteN<32>, // sha256 hash of the wasm data
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct AddWasmInput {
    pub description: String,
    pub wasm: ByteBuf,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct DeployWasmInput {
    pub canister: Principal,
    pub args: Option<ByteBuf>,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct BucketDeploymentInfo {
    pub deploy_at: u64, // in milliseconds
    pub canister: Principal,
    pub prev_hash: ByteN<32>,
    pub wasm_hash: ByteN<32>,
    pub args: Option<ByteBuf>,
    pub error: Option<String>,
}
