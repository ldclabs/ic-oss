use candid::Principal;
use ic_oss_types::cose::Token;
use serde_bytes::ByteBuf;

use crate::{api_admin, store};

#[ic_cdk::update]
async fn access_token(audience: Principal) -> Result<ByteBuf, String> {
    let token = get_token(ic_cdk::caller(), audience)?;

    api_admin::admin_sign_access_token(token).await
}

#[ic_cdk::update]
async fn ed25519_access_token(audience: Principal) -> Result<ByteBuf, String> {
    let token = get_token(ic_cdk::caller(), audience)?;

    api_admin::admin_ed25519_access_token(token).await
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
