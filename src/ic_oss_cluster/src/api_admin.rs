use candid::{pretty::candid::value::pp_value, CandidType, IDLArgs, IDLValue, Principal};
use ed25519_dalek::{Signer, SigningKey};
use ic_cdk_management_canister as mgt;
use ic_oss_types::{
    cluster::{AddWasmInput, DeployWasmInput},
    cose::{cose_sign1, cose_sign1_to_vec, sha256, EdDSA, Token, BUCKET_TOKEN_AAD, ES256K},
    format_error,
    permission::Policies,
};
use serde_bytes::{ByteArray, ByteBuf};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::time::Duration;

use crate::{
    chain_key, create_canister_on, is_controller, is_controller_or_manager,
    is_controller_or_manager_or_committer, store, validate_principals, MILLISECONDS, SECONDS,
};

// encoded candid arguments: ()
// println!("{:?}", candid::utils::encode_args(()).unwrap());
static EMPTY_CANDID_ARGS: &[u8] = &[68, 73, 68, 76, 0, 0];
const BUCKET_INITIAL_CYCLES: u128 = 2_000_000_000_000;
const UPGRADE_BATCH_SIZE: usize = 10;

/// Borrowing the large blobs here avoids the full Wasm clone performed by the
/// convenience management-canister wrapper before Candid encoding.
#[derive(CandidType)]
struct InstallCodeArgs<'a> {
    mode: mgt::CanisterInstallMode,
    canister_id: Principal,
    wasm_module: &'a [u8],
    arg: &'a [u8],
    sender_canister_version: Option<u64>,
}

async fn install_code(
    mode: mgt::CanisterInstallMode,
    canister_id: Principal,
    wasm_module: &[u8],
    arg: &[u8],
) -> Result<(), String> {
    ic_cdk::call::Call::unbounded_wait(Principal::management_canister(), "install_code")
        .with_arg(InstallCodeArgs {
            mode,
            canister_id,
            wasm_module,
            arg,
            sender_canister_version: Some(ic_cdk::api::canister_version()),
        })
        .await
        .map_err(format_error)?
        .candid()
        .map_err(format_error)
}

async fn install_bucket(
    mode: mgt::CanisterInstallMode,
    canister: Principal,
    prev_hash: ByteArray<32>,
    wasm_hash: ByteArray<32>,
    wasm: &store::Wasm,
    args: ByteBuf,
) -> Result<(), String> {
    let result = install_code(mode, canister, &wasm.wasm, &args).await;
    let log_id = store::wasm::add_log(store::DeployLog {
        deploy_at: ic_cdk::api::time() / MILLISECONDS,
        canister,
        prev_hash,
        wasm_hash,
        args,
        error: result.as_ref().err().cloned(),
    })?;

    if result.is_ok() {
        store::deployment::record(canister, log_id, wasm_hash);
    }
    result
}

fn with_cluster_controller(
    mut settings: mgt::CanisterSettings,
) -> Result<mgt::CanisterSettings, String> {
    let self_id = ic_cdk::api::canister_self();
    let controllers = settings.controllers.get_or_insert_with(Default::default);
    if !controllers.contains(&self_id) {
        if controllers.len() >= 10 {
            return Err("controllers already contains the maximum of 10 principals".to_string());
        }
        controllers.push(self_id);
    }
    Ok(settings)
}

async fn install_new_bucket(
    canister_id: Principal,
    hash: ByteArray<32>,
    args: Option<ByteBuf>,
) -> Result<Principal, String> {
    let args = args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS));
    let result = match store::wasm::get_wasm(&hash) {
        Some(wasm) => {
            install_bucket(
                mgt::CanisterInstallMode::Install,
                canister_id,
                Default::default(),
                hash,
                &wasm,
                args,
            )
            .await
        }
        None => Err(format!(
            "NotFound: wasm not found: {}",
            hex::encode(hash.as_ref())
        )),
    };
    result.map_err(|err| {
        format!(
            "bucket {} was created but installation failed: {}",
            canister_id.to_text(),
            err
        )
    })?;
    Ok(canister_id)
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers = args;
    });
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_add_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.managers.extend(args);
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
fn admin_add_committers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    store::state::with_mut(|r| {
        r.committers.extend(args);
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
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_principals(&args)?;
    Ok(())
}

#[ic_cdk::update]
fn validate_admin_add_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_remove_managers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_add_committers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update]
fn validate_admin_remove_committers(args: BTreeSet<Principal>) -> Result<String, String> {
    validate_principals(&args)?;
    pretty_format(&args)
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
pub async fn admin_sign_access_token(token: Token) -> Result<ByteBuf, String> {
    let now_sec = ic_cdk::api::time() / SECONDS;
    let (ecdsa_key_name, token_expiration) =
        store::state::with(|r| (r.ecdsa_key_name.clone(), r.token_expiration));
    let mut claims = token.to_cwt(now_sec as i64, token_expiration as i64);
    claims.issuer = Some(ic_cdk::api::canister_self().to_text());
    let mut sign1 = cose_sign1(claims, ES256K, None)?;
    let tbs_data = sign1
        .prepare_signature(None, None, Some(BUCKET_TOKEN_AAD))
        .map_err(|err| err.to_string())?;
    let message_hash = sha256(&tbs_data);

    let sig = chain_key::sign_ecdsa(ecdsa_key_name, message_hash).await?;
    sign1.set_signature(sig).map_err(|err| err.to_string())?;
    let token = cose_sign1_to_vec(&sign1).map_err(|err| err.to_string())?;
    Ok(ByteBuf::from(token))
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
pub async fn admin_ed25519_access_token(token: Token) -> Result<ByteBuf, String> {
    let now_sec = ic_cdk::api::time() / SECONDS;
    let (schnorr_key_name, token_expiration) =
        store::state::with(|r| (r.schnorr_key_name.clone(), r.token_expiration));

    let mut claims = token.to_cwt(now_sec as i64, token_expiration as i64);
    claims.issuer = Some(ic_cdk::api::canister_self().to_text());
    let mut sign1 = cose_sign1(claims, EdDSA, None)?;
    let tbs_data = sign1
        .prepare_signature(None, None, Some(BUCKET_TOKEN_AAD))
        .map_err(|err| err.to_string())?;

    let sig = chain_key::sign_ed25519(schnorr_key_name, tbs_data).await?;
    sign1.set_signature(sig).map_err(|err| err.to_string())?;
    let token = cose_sign1_to_vec(&sign1).map_err(|err| err.to_string())?;
    Ok(ByteBuf::from(token))
}

#[ic_cdk::query(guard = "is_controller_or_manager")]
pub fn admin_weak_access_token(
    token: Token,
    now_sec: u64,
    expiration_sec: u64,
) -> Result<ByteBuf, String> {
    sign_weak_access_token(token, now_sec, expiration_sec)
}

pub(crate) fn sign_weak_access_token(
    token: Token,
    now_sec: u64,
    expiration_sec: u64,
) -> Result<ByteBuf, String> {
    let secret_key = store::state::with(|r| r.weak_ed25519_secret_key);
    if secret_key.as_ref() == &[0u8; 32] {
        return Err("weak ed25519 key is not initialized".to_string());
    }

    let mut claims = token.to_cwt(now_sec as i64, expiration_sec as i64);
    claims.issuer = Some(ic_cdk::api::canister_self().to_text());
    let mut sign1 = cose_sign1(claims, EdDSA, None)?;
    let tbs_data = sign1
        .prepare_signature(None, None, Some(BUCKET_TOKEN_AAD))
        .map_err(|err| err.to_string())?;

    let signing_key = SigningKey::from_bytes(&secret_key);
    let sig = signing_key.sign(&tbs_data).to_bytes();
    sign1
        .set_signature(sig.to_vec())
        .map_err(|err| err.to_string())?;
    let token = cose_sign1_to_vec(&sign1).map_err(|err| err.to_string())?;
    Ok(ByteBuf::from(token))
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn admin_attach_policies(args: Token) -> Result<(), String> {
    let policies = Policies::try_from(args.policies.as_str())?;
    store::auth::attach_policies(args.subject, args.audience, policies);
    Ok(())
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn admin_detach_policies(args: Token) -> Result<(), String> {
    let policies = Policies::try_from(args.policies.as_str())?;
    store::auth::detach_policies(args.subject, args.audience, policies);
    Ok(())
}

#[ic_cdk::update(guard = "is_controller_or_manager_or_committer")]
fn admin_add_wasm(
    args: AddWasmInput,
    force_prev_hash: Option<ByteArray<32>>,
) -> Result<(), String> {
    store::wasm::add_wasm(
        ic_cdk::api::msg_caller(),
        ic_cdk::api::time() / MILLISECONDS,
        args,
        force_prev_hash,
        false,
    )?;
    Ok(())
}

#[ic_cdk::update]
fn validate2_admin_add_wasm(
    args: AddWasmInput,
    force_prev_hash: Option<ByteArray<32>>,
) -> Result<String, String> {
    let description = args.description.clone();
    let hash = store::wasm::add_wasm(
        ic_cdk::api::msg_caller(),
        ic_cdk::api::time() / MILLISECONDS,
        args,
        force_prev_hash,
        true,
    )?;
    pretty_format(&(
        ("wasm", hash),
        ("description", description),
        ("force_prev_hash", force_prev_hash),
    ))
}

#[ic_cdk::update]
fn validate_admin_add_wasm(
    args: AddWasmInput,
    force_prev_hash: Option<ByteArray<32>>,
) -> Result<(), String> {
    store::wasm::add_wasm(
        ic_cdk::api::msg_caller(),
        ic_cdk::api::time() / MILLISECONDS,
        args,
        force_prev_hash,
        true,
    )?;
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_create_bucket(
    settings: Option<mgt::CanisterSettings>,
    args: Option<ByteBuf>,
) -> Result<Principal, String> {
    // Validate the local prerequisite before paying to create a canister.
    let hash = store::wasm::get_latest_hash()?;
    let settings = with_cluster_controller(settings.unwrap_or_default())?;
    let res = mgt::create_canister_with_extra_cycles(
        &mgt::CreateCanisterArgs {
            settings: Some(settings),
        },
        BUCKET_INITIAL_CYCLES,
    )
    .await
    .map_err(format_error)?;
    install_new_bucket(res.canister_id, hash, args).await
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_create_bucket_on(
    subnet: Principal,
    settings: Option<mgt::CanisterSettings>,
    args: Option<ByteBuf>,
) -> Result<Principal, String> {
    // Validate the local prerequisite before paying to create a canister.
    let hash = store::wasm::get_latest_hash()?;
    let settings = with_cluster_controller(settings.unwrap_or_default())?;
    let canister_id = create_canister_on(subnet, Some(settings), BUCKET_INITIAL_CYCLES)
        .await
        .map_err(format_error)?;
    install_new_bucket(canister_id, hash, args).await
}

#[ic_cdk::update]
fn validate_admin_create_bucket(
    settings: Option<mgt::CanisterSettings>,
    args: Option<ByteBuf>,
) -> Result<String, String> {
    let args = IDLArgs::from_bytes(&args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS)))
        .map_err(|err| format!("Invalid args: {err}"))?;
    let settings = with_cluster_controller(settings.unwrap_or_default())?;
    let hash = store::wasm::get_latest_hash()?;
    pretty_format(&(
        ("settings", Some(settings)),
        ("wasm", hash),
        ("args", args.to_string()),
    ))
}

#[ic_cdk::update]
fn validate_admin_create_bucket_on(
    subnet: Principal,
    settings: Option<mgt::CanisterSettings>,
    args: Option<ByteBuf>,
) -> Result<String, String> {
    let args = IDLArgs::from_bytes(&args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS)))
        .map_err(|err| format!("Invalid args: {err}"))?;
    let settings = with_cluster_controller(settings.unwrap_or_default())?;
    let hash = store::wasm::get_latest_hash()?;
    pretty_format(&(
        ("subnet", subnet),
        ("settings", Some(settings)),
        ("wasm", hash),
        ("args", args.to_string()),
    ))
}

#[ic_cdk::update(guard = "is_controller")]
async fn admin_deploy_bucket(
    args: DeployWasmInput,
    ignore_prev_hash: Option<ByteArray<32>>,
) -> Result<(), String> {
    let info = mgt::canister_info(&mgt::CanisterInfoArgs {
        canister_id: args.canister,
        num_requested_changes: None,
    })
    .await
    .map_err(format_error)?;
    let id = ic_cdk::api::canister_self();
    if !info.controllers.contains(&id) {
        Err(format!(
            "{} is not a controller of the canister {}",
            id.to_text(),
            args.canister.to_text()
        ))?;
    }

    let mode = if info.module_hash.is_none() {
        mgt::CanisterInstallMode::Install
    } else {
        mgt::CanisterInstallMode::Upgrade(None)
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
    install_bucket(mode, args.canister, prev_hash, hash, &wasm, arg).await
}

#[ic_cdk::update]
async fn validate2_admin_deploy_bucket(
    args: DeployWasmInput,
    ignore_prev_hash: Option<ByteArray<32>>,
) -> Result<String, String> {
    let args_ = IDLArgs::from_bytes(
        &args
            .args
            .as_ref()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(ByteBuf::from(EMPTY_CANDID_ARGS))),
    )
    .map_err(|err| format!("Invalid args: {err}"))?;
    let rt = pretty_format(&(
        ("canister", args.canister),
        ("args", args_.to_string()),
        ("ignore_prev_hash", ignore_prev_hash),
    ))?;

    validate_admin_deploy_bucket(args, ignore_prev_hash).await?;
    Ok(rt)
}

#[ic_cdk::update]
async fn validate_admin_deploy_bucket(
    args: DeployWasmInput,
    ignore_prev_hash: Option<ByteArray<32>>,
) -> Result<(), String> {
    let info = mgt::canister_info(&mgt::CanisterInfoArgs {
        canister_id: args.canister,
        num_requested_changes: None,
    })
    .await
    .map_err(format_error)?;
    let id = ic_cdk::api::canister_self();
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
        store::wasm::get_latest_hash()?;
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
        s.bucket_upgrade_cursor = None;
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
    for id in &buckets {
        if !store::deployment::contains(id) {
            return Err(format!("canister {} is not deployed", id));
        }
    }
    let ids = if buckets.is_empty() {
        store::deployment::ids().into_iter().collect()
    } else {
        buckets
    };

    let args = args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS));
    let mut res = Vec::with_capacity(ids.len());
    for id in ids {
        let data = ic_cdk::call::Call::bounded_wait(id, &method)
            .with_raw_args(&args)
            .await
            .map_err(format_error)?;
        res.push(ByteBuf::from(data.into_bytes()));
    }

    Ok(res)
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
async fn admin_topup_all_buckets() -> Result<u128, String> {
    let (threshold, amount) =
        store::state::with(|s| (s.bucket_topup_threshold, s.bucket_topup_amount));
    if threshold == 0 || amount == 0 {
        Err("bucket topup is disabled".to_string())?;
    }

    let buckets = store::deployment::ids();
    if buckets.is_empty() {
        Err("no bucket deployed".to_string())?;
    }
    let required_balance = threshold
        .checked_add(amount)
        .ok_or_else(|| "bucket topup threshold + amount overflows nat128".to_string())?;

    let mut total = 0u128;
    for ids in buckets.chunks(7) {
        let res = futures::future::try_join_all(ids.iter().map(|id| async {
            let arg = mgt::DepositCyclesArgs { canister_id: *id };
            let status = mgt::canister_status(&arg).await.map_err(format_error)?;
            if status.cycles <= threshold {
                // read the balance right before spending: the other buckets in
                // this batch are topped up concurrently and have already spent
                // from it by now
                let balance = ic_cdk::api::canister_cycle_balance();
                if balance < required_balance {
                    Err(format!(
                        "balance {} is less than threshold {} + amount {}",
                        balance, threshold, amount
                    ))?;
                }

                mgt::deposit_cycles(&arg, amount)
                    .await
                    .map_err(format_error)?;
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
async fn admin_update_bucket_canister_settings(
    args: mgt::UpdateSettingsArgs,
) -> Result<(), String> {
    if !store::deployment::contains(&args.canister_id) {
        return Err("NotFound: bucket not found".to_string());
    }
    mgt::update_settings(&args).await.map_err(format_error)?;
    Ok(())
}

#[ic_cdk::update]
async fn validate2_admin_upgrade_all_buckets(args: Option<ByteBuf>) -> Result<String, String> {
    let args = IDLArgs::from_bytes(&args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS)))
        .map_err(|err| format!("Invalid args: {err}"))?;
    pretty_format(&args.to_string())
}

#[ic_cdk::update]
async fn validate_admin_upgrade_all_buckets(_args: Option<ByteBuf>) -> Result<(), String> {
    Ok(())
}

#[ic_cdk::update]
async fn validate2_admin_batch_call_buckets(
    buckets: BTreeSet<Principal>,
    method: String,
    args: Option<ByteBuf>,
) -> Result<String, String> {
    let args = IDLArgs::from_bytes(&args.unwrap_or_else(|| ByteBuf::from(EMPTY_CANDID_ARGS)))
        .map_err(|err| format!("Invalid args: {err}"))?;
    pretty_format(&(
        ("buckets", buckets),
        ("method", method),
        ("args", args.to_string()),
    ))
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
    args: mgt::UpdateSettingsArgs,
) -> Result<String, String> {
    if !store::deployment::contains(&args.canister_id) {
        return Err("NotFound: bucket not found".to_string());
    }

    pretty_format(&args)
}

async fn upgrade_buckets() -> Result<(), String> {
    match upgrade_batch().await {
        Ok(true) => {
            resume_bucket_upgrade();
            Ok(())
        }
        Ok(false) => {
            store::state::with_mut(|s| {
                s.bucket_upgrade_process = None;
                s.bucket_upgrade_cursor = None;
            });
            Ok(())
        }
        Err(err) => {
            store::state::with_mut(|s| {
                s.bucket_upgrade_process = None;
                s.bucket_upgrade_cursor = None;
            });
            Err(err)
        }
    }
}

pub(crate) fn resume_bucket_upgrade() {
    ic_cdk_timers::set_timer(Duration::from_secs(0), async {
        if let Err(err) = upgrade_buckets().await {
            ic_cdk::println!("bucket upgrade stopped: {err}");
        }
    });
}

async fn upgrade_batch() -> Result<bool, String> {
    let Some((args, mut cursor)) = store::state::with(|s| {
        s.bucket_upgrade_process
            .clone()
            .map(|args| (args, s.bucket_upgrade_cursor))
    }) else {
        return Ok(false);
    };
    let mut cached_wasm: Option<(ByteArray<32>, store::Wasm)> = None;

    for _ in 0..UPGRADE_BATCH_SIZE {
        let Some((canister, prev_hash, wasm_hash)) = store::deployment::next_upgrade(cursor) else {
            return Ok(false);
        };

        if cached_wasm
            .as_ref()
            .map(|(hash, _)| hash != &wasm_hash)
            .unwrap_or(true)
        {
            let wasm = store::wasm::get_wasm(&wasm_hash).ok_or_else(|| {
                format!(
                    "NotFound: wasm not found: {}",
                    hex::encode(wasm_hash.as_ref())
                )
            })?;
            cached_wasm = Some((wasm_hash, wasm));
        }

        let wasm = &cached_wasm.as_ref().expect("Wasm cache is initialized").1;
        install_bucket(
            mgt::CanisterInstallMode::Upgrade(None),
            canister,
            prev_hash,
            wasm_hash,
            wasm,
            args.clone(),
        )
        .await?;
        cursor = Some(canister);
    }

    store::state::with_mut(|s| s.bucket_upgrade_cursor = cursor);
    Ok(store::deployment::next_upgrade(cursor).is_some())
}

fn pretty_format<T>(data: &T) -> Result<String, String>
where
    T: CandidType,
{
    let val = IDLValue::try_from_candid_type(data).map_err(|err| format!("{err:?}"))?;
    let doc = pp_value(7, &val);

    Ok(format!("{}", doc.pretty(120)))
}
