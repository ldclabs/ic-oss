use ic_cdk_management_canister as mgt;

const TOKEN_KEY_DERIVATION_PATH: &[u8] = b"ic_oss_cluster";

fn token_derivation_path() -> Vec<Vec<u8>> {
    vec![TOKEN_KEY_DERIVATION_PATH.to_vec()]
}

pub async fn sign_ecdsa(key_name: String, message_hash: [u8; 32]) -> Result<Vec<u8>, String> {
    let result = mgt::sign_with_ecdsa(&mgt::SignWithEcdsaArgs {
        message_hash: message_hash.to_vec(),
        derivation_path: token_derivation_path(),
        key_id: mgt::EcdsaKeyId {
            curve: mgt::EcdsaCurve::Secp256k1,
            name: key_name,
        },
    })
    .await
    .map_err(|err| format!("sign_with_ecdsa failed: {err:?}"))?;
    Ok(result.signature)
}

pub async fn sign_ed25519(key_name: String, message: Vec<u8>) -> Result<Vec<u8>, String> {
    let result = mgt::sign_with_schnorr(&mgt::SignWithSchnorrArgs {
        message,
        derivation_path: token_derivation_path(),
        key_id: mgt::SchnorrKeyId {
            algorithm: mgt::SchnorrAlgorithm::Ed25519,
            name: key_name,
        },
        aux: None,
    })
    .await
    .map_err(|err| format!("sign_with_schnorr failed: {err:?}"))?;
    Ok(result.signature)
}

pub async fn ecdsa_public_key(key_name: String) -> Result<mgt::EcdsaPublicKeyResult, String> {
    mgt::ecdsa_public_key(&mgt::EcdsaPublicKeyArgs {
        canister_id: None,
        derivation_path: token_derivation_path(),
        key_id: mgt::EcdsaKeyId {
            curve: mgt::EcdsaCurve::Secp256k1,
            name: key_name,
        },
    })
    .await
    .map_err(|err| format!("ecdsa_public_key failed: {err:?}"))
}

pub async fn ed25519_public_key(key_name: String) -> Result<mgt::SchnorrPublicKeyResult, String> {
    mgt::schnorr_public_key(&mgt::SchnorrPublicKeyArgs {
        canister_id: None,
        derivation_path: token_derivation_path(),
        key_id: mgt::SchnorrKeyId {
            algorithm: mgt::SchnorrAlgorithm::Ed25519,
            name: key_name,
        },
    })
    .await
    .map_err(|err| format!("schnorr_public_key failed: {err:?}"))
}
