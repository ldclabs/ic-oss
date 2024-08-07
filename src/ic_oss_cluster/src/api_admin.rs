use candid::Principal;
use ic_cdk::api::management_canister::main::*;
use ic_oss_cose::{
    cose_sign1, coset::CborSerializable, sha256, Token as CoseToken, BUCKET_TOKEN_AAD,
    CLUSTER_TOKEN_AAD, ES256K,
};
use ic_oss_types::{
    bucket::Token,
    cluster::{AddWasmInput, DeployWasmInput},
    format_error,
    permission::Policies,
    ByteN,
};
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;
use std::time::Duration;

use crate::{
    ecdsa, is_controller, is_controller_or_manager, store, ANONYMOUS, MILLISECONDS, SECONDS,
};

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_admin_set_managers(args.clone())?;
    store::state::with_mut(|r| {
        r.managers = args;
    });
    Ok(())
}

#[ic_cdk::update]
fn validate_admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    if args.is_empty() {
        return Err("managers cannot be empty".to_string());
    }
    if args.contains(&ANONYMOUS) {
        return Err("anonymous user is not allowed".to_string());
    }
    Ok(())
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
async fn admin_sign_access_token(token: Token) -> Result<ByteBuf, String> {
    let now_sec = ic_cdk::api::time() / SECONDS;
    let (ecdsa_key_name, token_expiration) =
        store::state::with(|r| (r.ecdsa_key_name.clone(), r.token_expiration));
    let mut claims = CoseToken::from(token).to_cwt(now_sec as i64, token_expiration as i64);
    claims.issuer = Some(ic_cdk::id().to_text());
    let mut sign1 = cose_sign1(claims, ES256K, None)?;
    let tbs_data = sign1.tbs_data(BUCKET_TOKEN_AAD);
    let message_hash = sha256(&tbs_data);

    let sig = ecdsa::sign_with(
        &ecdsa_key_name,
        vec![CLUSTER_TOKEN_AAD.to_vec()],
        message_hash,
    )
    .await?;
    sign1.signature = sig;
    let token = sign1.to_vec().map_err(|err| err.to_string())?;
    Ok(ByteBuf::from(token))
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
async fn admin_attach_policies(args: Token) -> Result<(), String> {
    let policies = Policies::try_from(args.policies.as_str())?;
    store::auth::attach_policies(args.subject, args.audience, policies);
    Ok(())
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
async fn admin_detach_policies(args: Token) -> Result<(), String> {
    let policies = Policies::try_from(args.policies.as_str())?;
    store::auth::detach_policies(args.subject, args.audience, policies);
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_add_wasm(
    args: AddWasmInput,
    force_prev_hash: Option<ByteN<32>>,
) -> Result<(), String> {
    store::wasm::add_wasm(
        ic_cdk::caller(),
        ic_cdk::api::time() / MILLISECONDS,
        args,
        force_prev_hash,
        false,
    )
}

#[ic_cdk::update]
async fn validate_admin_add_wasm(
    args: AddWasmInput,
    force_prev_hash: Option<ByteN<32>>,
) -> Result<(), String> {
    store::wasm::add_wasm(
        ic_cdk::caller(),
        ic_cdk::api::time() / MILLISECONDS,
        args,
        force_prev_hash,
        true,
    )
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_deploy_bucket(args: DeployWasmInput, reinstall: Option<bool>) -> Result<(), String> {
    let (info,) = canister_info(CanisterInfoRequest {
        canister_id: args.canister,
        num_requested_changes: None,
    })
    .await
    .map_err(format_error)?;
    let id = ic_cdk::id();
    if !info.controllers.contains(&id) {
        Err(format!(
            "{} is not a controller of the canister {}",
            id.to_text(),
            args.canister.to_text()
        ))?;
    }

    let mode = if info.module_hash.is_none() {
        CanisterInstallMode::Install
    } else if reinstall.unwrap_or(false) {
        CanisterInstallMode::Reinstall
    } else {
        CanisterInstallMode::Upgrade(None)
    };

    let prev_hash: [u8; 32] = if let Some(hash) = info.module_hash {
        hash.try_into().map_err(format_error)?
    } else {
        Default::default()
    };
    let prev_hash = ByteN::from(prev_hash);
    let (hash, wasm) = store::wasm::next_version(prev_hash)?;
    let arg = args.args.unwrap_or_default();
    let res = install_code(InstallCodeArgument {
        mode,
        canister_id: args.canister,
        wasm_module: wasm.wasm.into_vec(),
        arg: arg.clone().into_vec(),
    })
    .await
    .map_err(format_error);

    let id = store::wasm::add_log(store::DeployLog {
        deploy_at: ic_cdk::api::time() / MILLISECONDS,
        canister: args.canister,
        prev_hash,
        wasm_hash: hash,
        args: arg,
        error: res.clone().err(),
    })?;

    if res.is_ok() {
        store::state::with_mut(|s| {
            s.bucket_deployed_list.insert(args.canister, (id, hash));
        })
    }
    res
}

#[ic_cdk::update]
async fn validate_admin_deploy_bucket(
    args: DeployWasmInput,
    _reinstall: Option<bool>,
) -> Result<(), String> {
    let (info,) = canister_info(CanisterInfoRequest {
        canister_id: args.canister,
        num_requested_changes: None,
    })
    .await
    .map_err(format_error)?;
    let id = ic_cdk::id();
    if !info.controllers.contains(&id) {
        Err(format!(
            "{} is not a controller of the canister {}",
            id.to_text(),
            args.canister.to_text()
        ))?;
    }

    let prev_hash: [u8; 32] = if let Some(hash) = info.module_hash {
        hash.try_into().map_err(format_error)?
    } else {
        Default::default()
    };
    let prev_hash = ByteN::from(prev_hash);
    let _ = store::wasm::next_version(prev_hash)?;
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_upgrade_all_buckets(args: Option<ByteBuf>) -> Result<(), String> {
    store::state::with_mut(|s| {
        if s.bucket_upgrade_process.is_some() {
            return Err("upgrade process is running".to_string());
        }
        s.bucket_upgrade_process = Some(args.unwrap_or_default());
        Ok(())
    })?;

    upgrade_buckets().await
}

#[ic_cdk::update]
async fn validate_admin_upgrade_all_buckets(_args: Option<ByteBuf>) -> Result<(), String> {
    Ok(())
}

async fn upgrade_buckets() -> Result<(), String> {
    match upgrade_bucket().await {
        Ok(Some(_)) => {
            ic_cdk_timers::set_timer(Duration::from_secs(0), || {
                ic_cdk::spawn(async {
                    let _ = upgrade_buckets().await;
                })
            });
            Ok(())
        }
        Ok(None) => {
            store::state::with_mut(|s| {
                s.bucket_upgrade_process = None;
            });
            Ok(())
        }
        Err(err) => {
            store::state::with_mut(|s| {
                s.bucket_upgrade_process = None;
            });
            Err(err)
        }
    }
}

async fn upgrade_bucket() -> Result<Option<Principal>, String> {
    let next = store::state::with(|s| {
        for (canister, (_, hash)) in s.bucket_deployed_list.iter() {
            if let Some(next) = s.bucket_upgrade_path.get(hash).cloned() {
                return Some((*canister, *hash, next, s.bucket_upgrade_process.clone()));
            }
        }
        None
    });

    match next {
        None => Ok(None),
        Some((canister, prev, hash, args)) => match store::wasm::get_wasm(&hash) {
            None => Err(format!("wasm not found: {}", hex::encode(hash.as_ref()))),
            Some(wasm) => {
                let res = install_code(InstallCodeArgument {
                    mode: CanisterInstallMode::Upgrade(None),
                    canister_id: canister,
                    wasm_module: wasm.wasm.into_vec(),
                    arg: args.unwrap_or_default().into_vec(),
                })
                .await
                .map_err(format_error);

                let id = store::wasm::add_log(store::DeployLog {
                    deploy_at: ic_cdk::api::time() / MILLISECONDS,
                    canister,
                    prev_hash: prev,
                    wasm_hash: hash,
                    args: ByteBuf::default(),
                    error: res.clone().err(),
                })?;

                match res {
                    Ok(_) => {
                        store::state::with_mut(|s| {
                            s.bucket_deployed_list.insert(canister, (id, hash));
                        });
                        Ok(Some(canister))
                    }
                    Err(err) => Err(err),
                }
            }
        },
    }
}
