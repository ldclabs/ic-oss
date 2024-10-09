use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

const MAX_SIGN_WITH_SCHNORR_FEE: u128 = 26_153_846_153;

#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SchnorrAlgorithm {
    #[serde(rename = "bip340secp256k1")]
    Bip340Secp256k1,
    #[serde(rename = "ed25519")]
    Ed25519,
}

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PublicKeyOutput {
    pub public_key: ByteBuf,
    pub chain_code: ByteBuf,
}

#[derive(CandidType, Deserialize, Serialize, Debug)]
pub struct SignWithSchnorrArgs {
    pub message: Vec<u8>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: SchnorrKeyId,
}

#[derive(CandidType, Deserialize, Serialize, Debug)]
pub struct SignWithSchnorrResult {
    pub signature: Vec<u8>,
}

pub async fn sign_with_schnorr(
    key_name: String,
    alg: SchnorrAlgorithm,
    derivation_path: Vec<Vec<u8>>,
    message: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let args = SignWithSchnorrArgs {
        message,
        derivation_path,
        key_id: SchnorrKeyId {
            algorithm: alg,
            name: key_name,
        },
    };

    let (res,): (SignWithSchnorrResult,) = ic_cdk::api::call::call_with_payment128(
        Principal::management_canister(),
        "sign_with_schnorr",
        (args,),
        MAX_SIGN_WITH_SCHNORR_FEE,
    )
    .await
    .map_err(|err| format!("sign_with_ecdsa failed {:?}", err))?;

    Ok(res.signature)
}

#[derive(CandidType, Deserialize, Serialize, Debug)]
pub struct SchnorrPublicKeyArgs {
    pub canister_id: Option<Principal>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: SchnorrKeyId,
}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SchnorrKeyId {
    algorithm: SchnorrAlgorithm,
    name: String,
}

pub async fn schnorr_public_key(
    key_name: String,
    alg: SchnorrAlgorithm,
    derivation_path: Vec<Vec<u8>>,
) -> Result<PublicKeyOutput, String> {
    let args = SchnorrPublicKeyArgs {
        canister_id: None,
        derivation_path,
        key_id: SchnorrKeyId {
            algorithm: alg,
            name: key_name,
        },
    };

    let (res,): (PublicKeyOutput,) = ic_cdk::call(
        Principal::management_canister(),
        "schnorr_public_key",
        (args,),
    )
    .await
    .map_err(|err| format!("schnorr_public_key failed {:?}", err))?;
    Ok(res)
}
