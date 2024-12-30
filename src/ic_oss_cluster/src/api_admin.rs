use candid::Principal;
use ed25519_dalek::{Signer, SigningKey};
use ic_cdk::api::management_canister::main::*;
use ic_oss_types::{
    cluster::{AddWasmInput, DeployWasmInput},
    cose::{cose_sign1, coset::CborSerializable, sha256, EdDSA, Token, BUCKET_TOKEN_AAD, ES256K},
    format_error,
    permission::Policies,
};
use serde_bytes::{ByteArray, ByteBuf};
use std::collections::BTreeSet;
use std::time::Duration;

use crate::{
    create_canister_on, ecdsa, is_controller, is_controller_or_manager,
    is_controller_or_manager_or_committer, schnorr, store, validate_principals, MILLISECONDS,
    SECONDS, TOKEN_KEY_DERIVATION_PATH,
};

// encoded candid arguments: ()
// println!("{:?}", candid::utils::encode_args(()).unwrap());
static EMPTY_CANDID_ARGS: &[u8] = &[68, 73, 68, 76, 0, 0];

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers = args;
    });
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_managers(mut args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers.append(&mut args);
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_remove_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers.retain(|p| !args.contains(p));
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_committers(mut args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.committers.append(&mut args);
        Ok(())
    })
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_remove_committers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.committers.retain(|p| !args.contains(p));
        Ok(())
    })
}

#[ic_cdk::update]
fn validate2_admin_set_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    Ok(())
}

#[ic_cdk::update]
fn validate_admin_add_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_remove_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_add_committers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_remove_committers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    Ok("ok".to_string())
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
pub async fn admin_sign_access_token(token: Token) -> Result<ByteBuf, String> {
    let now_sec = ic_cdk::api::time() / SECONDS;
    let (ecdsa_key_name, token_expiration) =
        store::state::with(|r| (r.ecdsa_key_name.clone(), r.token_expiration));
    let mut claims = token.to_cwt(now_sec as i64, token_expiration as i64);
    claims.issuer = Some(ic_cdk::id().to_text());
    let mut sign1 = cose_sign1(claims, ES256K, None)?;
    let tbs_data = sign1.tbs_data(BUCKET_TOKEN_AAD);
    let message_hash = sha256(&tbs_data);

    let sig = ecdsa::sign_with(
        &ecdsa_key_name,
        vec![TOKEN_KEY_DERIVATION_PATH.to_vec()],
        message_hash,
    )
    .await?;
    sign1.signature = sig;
    let token = sign1.to_vec().map_err(|err| err.to_string())?;
    Ok(ByteBuf::from(token))
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
pub async fn admin_ed25519_access_token(token: Token) -> Result<ByteBuf, String> {
    let now_sec = ic_cdk::api::time() / SECONDS;
    let (schnorr_key_name, token_expiration) =
        store::state::with(|r| (r.schnorr_key_name.clone(), r.token_expiration));

    let mut claims = token.to_cwt(now_sec as i64, token_expiration as i64);
    claims.issuer = Some(ic_cdk::id().to_text());
    let mut sign1 = cose_sign1(claims, EdDSA, None)?;
    let tbs_data = sign1.tbs_data(BUCKET_TOKEN_AAD);

    let sig = schnorr::sign_with_schnorr(
        schnorr_key_name,
        schnorr::SchnorrAlgorithm::Ed25519,
        vec![TOKEN_KEY_DERIVATION_PATH.to_vec()],
        tbs_data,
    )
    .await?;
    sign1.signature = sig;
    let token = sign1.to_vec().map_err(|err| err.to_string())?;
    Ok(ByteBuf::from(token))
}

#[ic_cdk::query(guard = "is_controller_or_manager")]
pub fn admin_weak_access_token(
    token: Token,
    now_sec: u64,
    expiration_sec: u64,
) -> Result<ByteBuf, String> {
    let secret_key = store::state::with(|r| r.weak_ed25519_secret_key);
    let mut claims = token.to_cwt(now_sec as i64, expiration_sec as i64);
    claims.issuer = Some(ic_cdk::id().to_text());
    let mut sign1 = cose_sign1(claims, EdDSA, None)?;
    let tbs_data = sign1.tbs_data(BUCKET_TOKEN_AAD);

    let signing_key = SigningKey::from_bytes(&secret_key);
    let sig = signing_key.sign(&tbs_data).to_bytes();
    sign1.signature = sig.to_vec();
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

#[ic_cdk::update(guard = "is_controller_or_manager_or_committer")]
async fn admin_add_wasm(
    args: AddWasmInput,
    force_prev_hash: Option<ByteArray<32>>,
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
async fn validate2_admin_add_wasm(
    args: AddWasmInput,
    force_prev_hash: Option<ByteArray<32>>,
) -> Result<String, String> {
    validate_admin_add_wasm(args, force_prev_hash).await?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
async fn validate_admin_add_wasm(
    args: AddWasmInput,
    force_prev_hash: Option<ByteArray<32>>,
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
async fn admin_create_bucket(
    settings: Option<CanisterSettings>,
    args: Option<ByteBuf>,
) -> Result<Principal, String> {
    let self_id = ic_cdk::id();
    let mut settings = settings.unwrap_or_default();
    let controllers = settings.controllers.get_or_insert_with(Default::default);
    if !controllers.contains(&self_id) {
        controllers.push(self_id);
    }

    let res = create_canister(
        CreateCanisterArgument {
            settings: Some(settings),
        },
        2_000_000_000_000,
    )
    .await
    .map_err(format_error)?;
    let canister_id = res.0.canister_id;
    let (hash, wasm) = store::wasm::get_latest()?;
    let arg = args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS));
    let res = install_code(InstallCodeArgument {
        mode: CanisterInstallMode::Install,
        canister_id,
        wasm_module: wasm.wasm.into_vec(),
        arg: arg.clone().into_vec(),
    })
    .await
    .map_err(format_error);

    let id = store::wasm::add_log(store::DeployLog {
        deploy_at: ic_cdk::api::time() / MILLISECONDS,
        canister: canister_id,
        prev_hash: Default::default(),
        wasm_hash: hash,
        args: arg,
        error: res.clone().err(),
    })?;

    if res.is_ok() {
        store::state::with_mut(|s| {
            s.bucket_deployed_list.insert(canister_id, (id, hash));
        })
    }
    Ok(canister_id)
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_create_bucket_on(
    subnet: Principal,
    settings: Option<CanisterSettings>,
    args: Option<ByteBuf>,
) -> Result<Principal, String> {
    let self_id = ic_cdk::id();
    let mut settings = settings.unwrap_or_default();
    let controllers = settings.controllers.get_or_insert_with(Default::default);
    if !controllers.contains(&self_id) {
        controllers.push(self_id);
    }

    let canister_id = create_canister_on(subnet, Some(settings), 2_000_000_000_000)
        .await
        .map_err(format_error)?;
    let (hash, wasm) = store::wasm::get_latest()?;
    let arg = args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS));
    let res = install_code(InstallCodeArgument {
        mode: CanisterInstallMode::Install,
        canister_id,
        wasm_module: wasm.wasm.into_vec(),
        arg: arg.clone().into_vec(),
    })
    .await
    .map_err(format_error);

    let id = store::wasm::add_log(store::DeployLog {
        deploy_at: ic_cdk::api::time() / MILLISECONDS,
        canister: canister_id,
        prev_hash: Default::default(),
        wasm_hash: hash,
        args: arg,
        error: res.clone().err(),
    })?;

    if res.is_ok() {
        store::state::with_mut(|s| {
            s.bucket_deployed_list.insert(canister_id, (id, hash));
        })
    }
    Ok(canister_id)
}

#[ic_cdk::update]
fn validate_admin_create_bucket(
    _settings: Option<CanisterSettings>,
    _args: Option<ByteBuf>,
) -> Result<String, String> {
    let _ = store::wasm::get_latest()?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
fn validate_admin_create_bucket_on(
    _subnet: Principal,
    _settings: Option<CanisterSettings>,
    _args: Option<ByteBuf>,
) -> Result<String, String> {
    let _ = store::wasm::get_latest()?;
    Ok("ok".to_string())
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_deploy_bucket(
    args: DeployWasmInput,
    ignore_prev_hash: Option<ByteArray<32>>,
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

    let mode = if info.module_hash.is_none() {
        CanisterInstallMode::Install
    } else {
        CanisterInstallMode::Upgrade(None)
    };

    let prev_hash: [u8; 32] = if let Some(hash) = info.module_hash {
        hash.try_into().map_err(format_error)?
    } else {
        Default::default()
    };
    let prev_hash = ByteArray::from(prev_hash);
    let (hash, wasm) = if let Some(ignore_prev_hash) = ignore_prev_hash {
        if ignore_prev_hash != prev_hash {
            Err(format!(
                "prev_hash mismatch: {} != {}",
                hex::encode(prev_hash.as_ref()),
                hex::encode(ignore_prev_hash.as_ref())
            ))?;
        }
        store::wasm::get_latest()?
    } else {
        store::wasm::next_version(prev_hash)?
    };

    let arg = args
        .args
        .unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS));
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
async fn validate2_admin_deploy_bucket(
    args: DeployWasmInput,
    ignore_prev_hash: Option<ByteArray<32>>,
) -> Result<String, String> {
    validate_admin_deploy_bucket(args, ignore_prev_hash).await?;
    Ok("ok".to_string())
}

#[ic_cdk::update]
async fn validate_admin_deploy_bucket(
    args: DeployWasmInput,
    ignore_prev_hash: Option<ByteArray<32>>,
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
    let prev_hash = ByteArray::from(prev_hash);
    if let Some(ignore_prev_hash) = ignore_prev_hash {
        if ignore_prev_hash != prev_hash {
            Err(format!(
                "prev_hash mismatch: {} != {}",
                hex::encode(prev_hash.as_ref()),
                hex::encode(ignore_prev_hash.as_ref())
            ))?;
        }
        let hash = store::state::with(|s| s.bucket_latest_version);
        let _ = store::wasm::get_wasm(&hash)
            .ok_or_else(|| format!("wasm not found: {}", hex::encode(hash.as_ref())))?;
    } else {
        store::wasm::next_version(prev_hash)?;
    }
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_upgrade_all_buckets(args: Option<ByteBuf>) -> Result<(), String> {
    store::state::with_mut(|s| {
        if s.bucket_upgrade_process.is_some() {
            return Err("upgrade process is running".to_string());
        }
        s.bucket_upgrade_process = Some(args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS)));
        Ok(())
    })?;

    upgrade_buckets().await
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
async fn admin_batch_call_buckets(
    buckets: BTreeSet<Principal>,
    method: String,
    args: Option<ByteBuf>,
) -> Result<Vec<ByteBuf>, String> {
    let ids = store::state::with(|s| {
        for id in &buckets {
            if !s.bucket_deployed_list.contains_key(id) {
                return Err(format!("canister {} is not deployed", id));
            }
        }
        if buckets.is_empty() {
            Ok(s.bucket_deployed_list.keys().cloned().collect())
        } else {
            Ok(buckets)
        }
    })?;

    let args = args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS));
    let mut res = Vec::with_capacity(ids.len());
    for id in ids {
        let data = ic_cdk::api::call::call_raw(id, &method, &args, 0)
            .await
            .map_err(format_error)?;
        res.push(ByteBuf::from(data));
    }

    Ok(res)
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
async fn admin_topup_all_buckets() -> Result<u128, String> {
    let (threshold, amount, buckets) = store::state::with(|s| {
        (
            s.bucket_topup_threshold,
            s.bucket_topup_amount,
            s.bucket_deployed_list.keys().cloned().collect::<Vec<_>>(),
        )
    });
    if threshold == 0 || amount == 0 {
        Err("bucket topup is disabled".to_string())?;
    }
    if buckets.is_empty() {
        Err("no bucket deployed".to_string())?;
    }

    let mut total = 0u128;
    for ids in buckets.chunks(7) {
        let res = futures::future::try_join_all(ids.iter().map(|id| async {
            let balance = ic_cdk::api::canister_balance128();
            if balance < threshold + amount {
                Err(format!(
                    "balance {} is less than threshold {} + amount {}",
                    balance, threshold, amount
                ))?;
            }

            let arg = CanisterIdRecord { canister_id: *id };
            let (status,) = canister_status(arg).await.map_err(format_error)?;
            if status.cycles <= threshold {
                deposit_cycles(arg, amount).await.map_err(format_error)?;
                return Ok::<u128, String>(amount);
            }
            Ok::<u128, String>(0)
        }))
        .await?;
        total += res.iter().sum::<u128>();
    }

    Ok(total)
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_update_bucket_canister_settings(args: UpdateSettingsArgument) -> Result<(), String> {
    store::state::with(|s| {
        if !s.bucket_deployed_list.contains_key(&args.canister_id) {
            return Err("bucket not found".to_string());
        }
        Ok(())
    })?;
    update_settings(args).await.map_err(format_error)?;
    Ok(())
}

#[ic_cdk::update]
async fn validate2_admin_upgrade_all_buckets(_args: Option<ByteBuf>) -> Result<String, String> {
    Ok("ok".to_string())
}

#[ic_cdk::update]
async fn validate_admin_upgrade_all_buckets(_args: Option<ByteBuf>) -> Result<(), String> {
    Ok(())
}

#[ic_cdk::update]
async fn validate2_admin_batch_call_buckets(
    _buckets: BTreeSet<Principal>,
    _method: String,
    _args: Option<ByteBuf>,
) -> Result<String, String> {
    Ok("ok".to_string())
}

#[ic_cdk::update]
async fn validate_admin_batch_call_buckets(
    _buckets: BTreeSet<Principal>,
    _method: String,
    _args: Option<ByteBuf>,
) -> Result<Vec<ByteBuf>, String> {
    Ok(Vec::new())
}

#[ic_cdk::update]
async fn validate_admin_update_bucket_canister_settings(
    args: UpdateSettingsArgument,
) -> Result<String, String> {
    store::state::with(|s| {
        if !s.bucket_deployed_list.contains_key(&args.canister_id) {
            return Err("bucket not found".to_string());
        }
        Ok(())
    })?;
    Ok("ok".to_string())
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
