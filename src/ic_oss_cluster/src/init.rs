use candid::{CandidType, Principal};
use serde::Deserialize;
use std::time::Duration;

use crate::store;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum ChainArgs {
    Init(InitArgs),
    Upgrade(UpgradeArgs),
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct InitArgs {
    name: String,
    ecdsa_key_name: String, // Use "dfx_test_key" for local replica and "test_key_1" for a testing key for testnet and mainnet
    schnorr_key_name: String, // Use "dfx_test_key" for local replica and "test_key_1" for a testing key for testnet and mainnet
    token_expiration: u64,    // in seconds
    bucket_topup_threshold: u128,
    bucket_topup_amount: u128,
    governance_canister: Option<Principal>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct UpgradeArgs {
    name: Option<String>,
    token_expiration: Option<u64>, // in seconds
    bucket_topup_threshold: Option<u128>,
    bucket_topup_amount: Option<u128>,
    governance_canister: Option<Principal>,
}

#[ic_cdk::init]
fn init(args: Option<ChainArgs>) {
    match args.expect("init args is missing") {
        ChainArgs::Init(args) => {
            store::state::with_mut(|s| {
                s.name = args.name;
                s.ecdsa_key_name = args.ecdsa_key_name;
                s.schnorr_key_name = args.schnorr_key_name;
                s.token_expiration = if args.token_expiration == 0 {
                    3600
                } else {
                    args.token_expiration
                };
                s.bucket_topup_threshold = args.bucket_topup_threshold;
                s.bucket_topup_amount = args.bucket_topup_amount;
                s.governance_canister = args.governance_canister;
            });
        }
        ChainArgs::Upgrade(_) => {
            ic_cdk::trap(
                "cannot initialize the canister with an Upgrade args. Please provide an Init args.",
            );
        }
    }

    ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_cdk::spawn(store::state::try_init_public_key())
    });
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    store::state::save();
}

#[ic_cdk::post_upgrade]
fn post_upgrade(args: Option<ChainArgs>) {
    store::state::load();

    match args {
        Some(ChainArgs::Upgrade(args)) => {
            store::state::with_mut(|s| {
                if let Some(name) = args.name {
                    s.name = name;
                }
                if let Some(token_expiration) = args.token_expiration {
                    s.token_expiration = if token_expiration == 0 {
                        3600
                    } else {
                        token_expiration
                    };
                }
                if let Some(bucket_topup_threshold) = args.bucket_topup_threshold {
                    s.bucket_topup_threshold = bucket_topup_threshold;
                }
                if let Some(bucket_topup_amount) = args.bucket_topup_amount {
                    s.bucket_topup_amount = bucket_topup_amount;
                }
                if let Some(governance_canister) = args.governance_canister {
                    s.governance_canister = Some(governance_canister);
                }
            });
        }
        Some(ChainArgs::Init(_)) => {
            ic_cdk::trap(
                "cannot upgrade the canister with an Init args. Please provide an Upgrade args.",
            );
        }
        _ => {}
    }

    store::state::with_mut(|s| {
        if s.schnorr_key_name.is_empty() {
            s.schnorr_key_name = s.ecdsa_key_name.clone();
        }
    });

    ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_cdk::spawn(store::state::try_init_public_key())
    });
}
