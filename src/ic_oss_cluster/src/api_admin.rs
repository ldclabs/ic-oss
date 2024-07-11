use candid::Principal;
use coset::CborSerializable;
use ic_oss_types::cwt::{cose_sign1, sha256, Token, BUCKET_TOKEN_AAD, CLUSTER_TOKEN_AAD, ES256K};
use serde_bytes::ByteBuf;
use std::collections::BTreeSet;

use crate::{ecdsa, is_controller, store, ANONYMOUS, SECONDS};

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    validate_admin_set_managers(args.clone())?;
    store::state::with_mut(|r| {
        r.managers = args;
    });
    Ok(())
}

#[ic_cdk::query]
fn validate_admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    if args.is_empty() {
        return Err("managers cannot be empty".to_string());
    }
    if args.contains(&ANONYMOUS) {
        return Err("anonymous user is not allowed".to_string());
    }
    Ok(())
}

#[ic_cdk::update]
async fn admin_sign_access_token(token: Token) -> Result<ByteBuf, String> {
    if !store::state::is_manager(&ic_cdk::caller()) {
        Err("user is not a manager".to_string())?;
    }

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
        vec![CLUSTER_TOKEN_AAD.to_vec()],
        message_hash,
    )
    .await?;
    sign1.signature = sig;
    let token = sign1.to_vec().map_err(|err| err.to_string())?;
    Ok(ByteBuf::from(token))
}
