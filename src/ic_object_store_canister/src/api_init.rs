use candid::{CandidType, Principal};
use serde::Deserialize;

use crate::store;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum InstallArgs {
    Init(InitArgs),
    Upgrade(UpgradeArgs),
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct InitArgs {
    name: String,
    governance_canister: Option<Principal>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct UpgradeArgs {
    name: Option<String>, // seconds
    governance_canister: Option<Principal>,
}

#[ic_cdk::init]
fn init(args: Option<InstallArgs>) {
    store::state::with_mut(|s| {
        s.name = "ICObjectStore".to_string();
    });

    match args {
        Some(InstallArgs::Init(args)) => {
            store::state::with_mut(|s| {
                s.name = args.name;
                s.governance_canister = args.governance_canister;
            });
        }
        Some(InstallArgs::Upgrade(_)) => {
            ic_cdk::trap(
                "cannot initialize the canister with an Upgrade args. Please provide an Init args.",
            );
        }
        _ => {}
    }
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    store::state::save();
}

#[ic_cdk::post_upgrade]
fn post_upgrade(args: Option<InstallArgs>) {
    store::state::load();

    match args {
        Some(InstallArgs::Upgrade(args)) => {
            store::state::with_mut(|s| {
                if let Some(name) = args.name {
                    s.name = name;
                }
                if let Some(governance_canister) = args.governance_canister {
                    s.governance_canister = Some(governance_canister);
                }
            });
        }
        Some(InstallArgs::Init(_)) => {
            ic_cdk::trap(
                "cannot upgrade the canister with an Init args. Please provide an Upgrade args.",
            );
        }
        _ => {}
    }
}
