use ic_cdk::management_canister as mgt;

pub async fn sign_with_ecdsa(
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
    message_hash: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if message_hash.len() != 32 {
        return Err("message must be 32 bytes".to_string());
    }
    let args = mgt::SignWithEcdsaArgs {
        message_hash,
        derivation_path,
        key_id: mgt::EcdsaKeyId {
            curve: mgt::EcdsaCurve::Secp256k1,
            name: key_name,
        },
    };

    let rt = mgt::sign_with_ecdsa(&args)
        .await
        .map_err(|err| format!("sign_with_ecdsa failed {:?}", err))?;

    Ok(rt.signature)
}

pub async fn ecdsa_public_key(
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
) -> Result<mgt::EcdsaPublicKeyResult, String> {
    let args = mgt::EcdsaPublicKeyArgs {
        canister_id: None,
        derivation_path,
        key_id: mgt::EcdsaKeyId {
            curve: mgt::EcdsaCurve::Secp256k1,
            name: key_name,
        },
    };

    let rt = mgt::ecdsa_public_key(&args)
        .await
        .map_err(|err| format!("ecdsa_public_key failed {:?}", err))?;

    Ok(rt)
}
