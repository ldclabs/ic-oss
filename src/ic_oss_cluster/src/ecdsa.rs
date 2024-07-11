use ic_cdk::api::management_canister::ecdsa;

pub async fn sign_with(
    key_name: &str,
    derivation_path: Vec<Vec<u8>>,
    message_hash: [u8; 32],
) -> Result<Vec<u8>, String> {
    let args = ecdsa::SignWithEcdsaArgument {
        message_hash: message_hash.to_vec(),
        derivation_path,
        key_id: ecdsa::EcdsaKeyId {
            curve: ecdsa::EcdsaCurve::Secp256k1,
            name: key_name.to_string(),
        },
    };

    let (response,): (ecdsa::SignWithEcdsaResponse,) = ecdsa::sign_with_ecdsa(args)
        .await
        .map_err(|err| format!("sign_with_ecdsa failed {:?}", err))?;

    Ok(response.signature)
}

pub async fn public_key_with(
    key_name: &str,
    derivation_path: Vec<Vec<u8>>,
) -> Result<ecdsa::EcdsaPublicKeyResponse, String> {
    let args = ecdsa::EcdsaPublicKeyArgument {
        canister_id: None,
        derivation_path,
        key_id: ecdsa::EcdsaKeyId {
            curve: ecdsa::EcdsaCurve::Secp256k1,
            name: key_name.to_string(),
        },
    };

    let (response,): (ecdsa::EcdsaPublicKeyResponse,) = ecdsa::ecdsa_public_key(args)
        .await
        .map_err(|err| format!("ecdsa_public_key failed {:?}", err))?;

    Ok(response)
}
