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
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct WasmInfo {
    pub kind: u8,        // 0: bucket wasm, 1: cluster wasm
    pub created_at: u64, // in milliseconds
    pub created_by: Principal,
    pub description: String,
    pub wasm: ByteBuf,
    pub hash: ByteN<32>, // sha256 hash of the wasm data
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct AddWasmInput {
    pub kind: u8, // 0: bucket wasm, 1: cluster wasm
    pub description: String,
    pub wasm: ByteBuf,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize)]
pub struct DeployWasmInput {
    pub kind: u8, // 0: bucket wasm, 1: cluster wasm
    pub canister: Principal,
    pub args: Option<ByteBuf>,
}
