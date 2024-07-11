use candid::CandidType;
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
    ecdsa_key_name: String, // Use "dfx_test_key" for local replica and "test_key_1" for a testing key for testnet and mainnet
    token_expiration: u64,  // in seconds
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct UpgradeArgs {}

#[ic_cdk::init]
fn init(args: Option<ChainArgs>) {
    match args.expect("init args is missing") {
        ChainArgs::Init(args) => {
            store::state::with_mut(|s| {
                s.ecdsa_key_name = args.ecdsa_key_name;
                s.token_expiration = if args.token_expiration == 0 {
                    3600
                } else {
                    args.token_expiration
                };
            });
        }
        ChainArgs::Upgrade(_) => {
            ic_cdk::trap(
                "cannot initialize the canister with an Upgrade args. Please provide an Init args.",
            );
        }
    }

    ic_cdk_timers::set_timer(Duration::from_secs(0), || {
        ic_cdk::spawn(async {
            store::state::init_ecdsa_public_key().await;
        })
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
        Some(ChainArgs::Upgrade(_)) => {}
        Some(ChainArgs::Init(_)) => {
            ic_cdk::trap(
                "cannot upgrade the canister with an Init args. Please provide an Upgrade args.",
            );
        }
        _ => {}
    }
}
