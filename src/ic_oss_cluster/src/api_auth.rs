use candid::Principal;
use ic_oss_types::cose::Token;
use serde_bytes::ByteBuf;

use crate::{api_admin, store, SECONDS};

#[ic_cdk::update]
async fn access_token(audience: Principal) -> Result<ByteBuf, String> {
    let token = get_token(ic_cdk::api::msg_caller(), audience)?;

    api_admin::admin_sign_access_token(token).await
}

#[ic_cdk::update]
async fn ed25519_access_token(audience: Principal) -> Result<ByteBuf, String> {
    let token = get_token(ic_cdk::api::msg_caller(), audience)?;

    api_admin::admin_ed25519_access_token(token).await
}

/// Low-cost token path signed by the canister's replicated local key. This is
/// intentionally a query and therefore has weaker trust guarantees than the
/// threshold-signature update methods.
#[ic_cdk::query]
fn weak_access_token(audience: Principal) -> Result<ByteBuf, String> {
    let token = get_token(ic_cdk::api::msg_caller(), audience)?;
    let now_sec = ic_cdk::api::time() / SECONDS;
    let expiration_sec = store::state::with(|s| s.token_expiration);
    api_admin::sign_weak_access_token(token, now_sec, expiration_sec)
}

fn get_token(subject: Principal, audience: Principal) -> Result<Token, String> {
    match store::auth::get_all_policies(&subject) {
        None => Err("no policies found".to_string()),
        Some(pt) => {
            let policies = pt.0.get(&audience).ok_or("no policies found")?;
            Ok(Token {
                subject,
                audience,
                policies: policies.to_owned(),
            })
        }
    }
}
