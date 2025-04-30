use ic_cdk::management_canister as mgt;

pub use mgt::SchnorrAlgorithm;

pub async fn sign_with_schnorr(
    key_name: String,
    alg: mgt::SchnorrAlgorithm,
    derivation_path: Vec<Vec<u8>>,
    message: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let args = mgt::SignWithSchnorrArgs {
        message,
        derivation_path,
        key_id: mgt::SchnorrKeyId {
            algorithm: alg,
            name: key_name,
        },
        aux: None,
    };

    let rt = mgt::sign_with_schnorr(&args)
        .await
        .map_err(|err| format!("sign_with_ecdsa failed: {:?}", err))?;

    Ok(rt.signature)
}

pub async fn schnorr_public_key(
    key_name: String,
    alg: mgt::SchnorrAlgorithm,
    derivation_path: Vec<Vec<u8>>,
) -> Result<mgt::SchnorrPublicKeyResult, String> {
    let args = mgt::SchnorrPublicKeyArgs {
        canister_id: None,
        derivation_path,
        key_id: mgt::SchnorrKeyId {
            algorithm: alg,
            name: key_name,
        },
    };

    let rt = mgt::schnorr_public_key(&args)
        .await
        .map_err(|err| format!("schnorr_public_key failed {:?}", err))?;
    Ok(rt)
}
