use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(CandidType, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ClusterInfo {
    pub name: String,
    pub ecdsa_key_name: String,
    pub ecdsa_token_public_key: String,
    pub token_expiration: u64, // in seconds
    pub managers: BTreeSet<Principal>,
}
