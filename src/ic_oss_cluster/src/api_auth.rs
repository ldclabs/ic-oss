use candid::Principal;
use coset::CborSerializable;
use ic_oss_types::cwt::{cose_sign1, sha256, Token, BUCKET_TOKEN_AAD, CLUSTER_TOKEN_AAD, ES256K};
use serde_bytes::ByteBuf;

use crate::{ecdsa, store, SECONDS};

#[ic_cdk::update]
async fn access_token(audience: Principal) -> Result<ByteBuf, String> {
    if !store::state::is_manager(&ic_cdk::caller()) {
        Err("user is not a manager".to_string())?;
    }
    let subject = ic_cdk::caller();

    match store::auth::get_all_policies(&subject) {
        None => Err("no policies found".to_string()),
        Some(pt) => {
            let policies = pt.0.get(&audience).ok_or("no policies found")?;
            let token = Token {
                subject,
                audience,
                policies: policies.to_owned(),
            };

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
    }
}
