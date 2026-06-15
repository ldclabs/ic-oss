use candid::{CandidType, Principal};
use cose2::{cwt::Claims, iana, tag, CoseMap, Error as CoseError, Label, Sign1Message, Value};
use ed25519_dalek::{Signature, VerifyingKey};
use k256::{ecdsa, ecdsa::signature::hazmat::PrehashVerifier};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use sha2::Digest;

pub use cose2;
pub use iana::{AlgorithmES256K as ES256K, AlgorithmEdDSA as EdDSA};

const CLOCK_SKEW: i64 = 5 * 60; // 5 minutes
const ALG_ED25519: i64 = EdDSA;
const ALG_SECP256K1: i64 = ES256K;

const SCOPE_NAME: i64 = iana::CWTClaimScope;

pub static BUCKET_TOKEN_AAD: &[u8] = b"ic_oss_bucket";

#[derive(CandidType, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Token {
    pub subject: Principal,
    pub audience: Principal,
    pub policies: String,
}

impl Token {
    pub fn from_sign1(
        sign1_token: &[u8],
        secp256k1_pub_keys: &[ByteBuf],
        ed25519_pub_keys: &[ByteArray<32>],
        aad: &[u8],
        now_sec: i64,
    ) -> Result<Self, String> {
        let cs1 = Sign1Message::from_slice(sign1_token)
            .map_err(|err| format!("invalid COSE sign1 token: {}", err))?;

        let tbs_data = Sign1Message::to_be_signed(
            cs1.protected_raw(),
            aad,
            cs1.payload.as_deref().unwrap_or_default(),
        )
        .map_err(|err| format!("invalid COSE signing input: {}", err))?;
        match cs1
            .protected
            .alg()
            .map_err(|err| format!("invalid COSE header: {}", err))?
        {
            Some(Label::Int(ALG_SECP256K1)) => {
                Self::secp256k1_verify(secp256k1_pub_keys, &tbs_data, cs1.signature())?;
            }
            Some(Label::Int(ALG_ED25519)) => {
                Self::ed25519_verify(ed25519_pub_keys, &tbs_data, cs1.signature())?;
            }
            alg => {
                Err(format!("unsupported algorithm: {:?}", alg))?;
            }
        }

        Self::from_cwt_bytes(cs1.payload.as_deref().unwrap_or_default(), now_sec)
    }

    pub fn to_cwt(self, now_sec: i64, expiration_sec: i64) -> Claims {
        let now = to_cwt_timestamp(now_sec);
        let expiration = to_cwt_timestamp(now_sec.saturating_add(expiration_sec));
        let mut extra = CoseMap::new();
        extra.insert(SCOPE_NAME, self.policies);

        Claims {
            issuer: None,
            subject: Some(self.subject.to_text()),
            audience: Some(self.audience.to_text()),
            expiration: Some(expiration),
            not_before: Some(now),
            issued_at: Some(now),
            cwt_id: None,
            extra,
        }
    }

    fn secp256k1_verify(
        pub_keys: &[ByteBuf],
        tbs_data: &[u8],
        signature: &[u8],
    ) -> Result<(), String> {
        let keys: Vec<ecdsa::VerifyingKey> = pub_keys
            .iter()
            .map(|key| {
                ecdsa::VerifyingKey::from_sec1_bytes(key)
                    .map_err(|_| "invalid verifying key".to_string())
            })
            .collect::<Result<_, _>>()?;
        let sig = ecdsa::Signature::try_from(signature).map_err(|_| "invalid signature")?;
        let digest = sha256(tbs_data);
        match keys
            .iter()
            .any(|key| key.verify_prehash(digest.as_slice(), &sig).is_ok())
        {
            true => Ok(()),
            false => Err("signature verification failed".to_string()),
        }
    }

    fn ed25519_verify(
        pub_keys: &[ByteArray<32>],
        tbs_data: &[u8],
        signature: &[u8],
    ) -> Result<(), String> {
        let keys: Vec<VerifyingKey> = pub_keys
            .iter()
            .map(|key| {
                VerifyingKey::from_bytes(key).map_err(|_| "invalid verifying key".to_string())
            })
            .collect::<Result<_, _>>()?;
        let sig = Signature::from_slice(signature).map_err(|_| "invalid signature")?;

        match keys
            .iter()
            .any(|key| key.verify_strict(tbs_data, &sig).is_ok())
        {
            true => Ok(()),
            false => Err("signature verification failed".to_string()),
        }
    }

    fn from_cwt_bytes(data: &[u8], now_sec: i64) -> Result<Self, String> {
        let claims = claims_from_slice(data).map_err(|err| format!("invalid claims: {}", err))?;
        if let Some(exp) = timestamp_claim(&claims, iana::CWTClaimExp)? {
            if exp < now_sec - CLOCK_SKEW {
                return Err("token expired".to_string());
            }
        }
        if let Some(nbf) = timestamp_claim(&claims, iana::CWTClaimNbf)? {
            if nbf > now_sec + CLOCK_SKEW {
                return Err("token not yet valid".to_string());
            }
        }
        Self::try_from(claims)
    }
}

/// algorithm: EdDSA | ES256K
pub fn cose_sign1(cs: Claims, alg: i64, key_id: Option<Vec<u8>>) -> Result<Sign1Message, String> {
    let tagged_payload = cs.to_vec().map_err(|err| err.to_string())?;
    let payload = tag::skip_tag(tag::CWT_PREFIX, &tagged_payload).to_vec();
    let mut msg = Sign1Message::new(Some(payload));
    msg.protected.set_alg(Label::Int(alg));
    if let Some(key_id) = key_id {
        msg.unprotected.set_kid(key_id);
    }
    Ok(msg)
}

pub fn cose_sign1_to_vec(sign1: &Sign1Message) -> Result<Vec<u8>, CoseError> {
    let encoded = sign1.to_vec()?;
    Ok(tag::skip_tag(tag::SIGN1_PREFIX, &encoded).to_vec())
}

impl TryFrom<CoseMap> for Token {
    type Error = String;

    fn try_from(claims: CoseMap) -> Result<Self, Self::Error> {
        let scope = claims
            .get_text(SCOPE_NAME)
            .map_err(|_| "invalid scope text")?
            .ok_or("missing scope")?;
        let subject = claims
            .get_text(iana::CWTClaimSub)
            .map_err(|_| "invalid subject text")?
            .ok_or("missing subject")?;
        let audience = claims
            .get_text(iana::CWTClaimAud)
            .map_err(|_| "invalid audience text")?
            .ok_or("missing audience")?;

        Ok(Token {
            subject: Principal::from_text(subject)
                .map_err(|err| format!("invalid subject: {}", err))?,
            audience: Principal::from_text(audience)
                .map_err(|err| format!("invalid audience: {}", err))?,
            policies: scope.to_string(),
        })
    }
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn to_cwt_timestamp(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

fn claims_from_slice(data: &[u8]) -> Result<CoseMap, CoseError> {
    let data = tag::skip_tag(tag::CBOR_SELF_PREFIX, data);
    let data = tag::skip_tag(tag::CWT_PREFIX, data);
    CoseMap::from_slice(data)
}

fn timestamp_claim(claims: &CoseMap, key: i64) -> Result<Option<i64>, String> {
    match claims.get(key) {
        None => Ok(None),
        Some(Value::Integer(value)) => i64::try_from(*value)
            .map(Some)
            .map_err(|_| "invalid timestamp integer".to_string()),
        Some(Value::Float(value)) => Ok(Some(value.to_i64().unwrap_or_default())),
        Some(_) => Err("invalid timestamp".to_string()),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::permission::{Operation, Permission, Policies, Policy, Resource, Resources};
    use ed25519_dalek::Signer;

    #[test]
    fn test_ed25519_token() {
        let secret_key = [8u8; 32];
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_key);
        let pub_key: &VerifyingKey = signing_key.as_ref();
        let pub_key = pub_key.to_bytes();
        let ps = Policies::from([
            Policy {
                permission: Permission {
                    resource: Resource::Bucket,
                    operation: Operation::Read,
                    constraint: Some(Resource::All),
                },
                resources: Resources::from([]),
            },
            Policy {
                permission: Permission {
                    resource: Resource::Folder,
                    operation: Operation::All,
                    constraint: None,
                },
                resources: Resources::from(["1".to_string()]),
            },
        ]);
        let token = Token {
            subject: Principal::from_text(
                "z7wjp-v6fe3-kksu5-26f64-dedtw-j7ndj-57onx-qga6c-et5e3-njx53-tae",
            )
            .unwrap(),
            audience: Principal::from_text("mmrxu-fqaaa-aaaap-ahhna-cai").unwrap(),
            policies: ps.to_string(),
        };
        println!("token: {:?}", &token);

        let now_sec = 1720676064;
        let claims = token.clone().to_cwt(now_sec, 3600);
        let mut sign1 = cose_sign1(claims, EdDSA, None).unwrap();
        let tbs_data = sign1
            .prepare_signature(None, None, Some(BUCKET_TOKEN_AAD))
            .unwrap();
        let sig = signing_key.sign(&tbs_data).to_bytes();
        sign1.set_signature(sig.to_vec()).unwrap();
        let sign1_token = cose_sign1_to_vec(&sign1).unwrap();
        println!("principal: {:?}", &Principal::anonymous().to_text());
        println!("pub_key: {:?}", &pub_key);
        println!("sign1_token: {:?}", &sign1_token);

        let token2 = Token::from_sign1(
            &sign1_token,
            &[],
            &[pub_key.into()],
            BUCKET_TOKEN_AAD,
            now_sec,
        )
        .unwrap();
        assert_eq!(token, token2);
    }
}
