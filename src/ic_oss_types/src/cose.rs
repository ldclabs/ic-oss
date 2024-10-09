use candid::{CandidType, Principal};
use coset::{
    cwt::{ClaimName, ClaimsSet, Timestamp},
    iana, Algorithm, CborSerializable, CoseSign1, CoseSign1Builder, HeaderBuilder,
};
use ed25519_dalek::{Signature, VerifyingKey};
use k256::{ecdsa, ecdsa::signature::hazmat::PrehashVerifier};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use sha2::Digest;

pub use coset;
pub use iana::Algorithm::{EdDSA, ES256K};

const CLOCK_SKEW: i64 = 5 * 60; // 5 minutes
const ALG_ED25519: Algorithm = Algorithm::Assigned(EdDSA);
const ALG_SECP256K1: Algorithm = Algorithm::Assigned(ES256K);

static SCOPE_NAME: ClaimName = ClaimName::Assigned(iana::CwtClaimName::Scope);

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
        let cs1 = CoseSign1::from_slice(sign1_token)
            .map_err(|err| format!("invalid COSE sign1 token: {}", err))?;

        match cs1.protected.header.alg {
            Some(ALG_SECP256K1) => {
                Self::secp256k1_verify(secp256k1_pub_keys, &cs1.tbs_data(aad), &cs1.signature)?;
            }
            Some(ALG_ED25519) => {
                Self::ed25519_verify(ed25519_pub_keys, &cs1.tbs_data(aad), &cs1.signature)?;
            }
            alg => {
                Err(format!("unsupported algorithm: {:?}", alg))?;
            }
        }

        Self::from_cwt_bytes(&cs1.payload.unwrap_or_default(), now_sec)
    }

    pub fn to_cwt(self, now_sec: i64, expiration_sec: i64) -> ClaimsSet {
        ClaimsSet {
            issuer: None,
            subject: Some(self.subject.to_text()),
            audience: Some(self.audience.to_text()),
            expiration_time: Some(Timestamp::WholeSeconds(now_sec + expiration_sec)),
            not_before: Some(Timestamp::WholeSeconds(now_sec)),
            issued_at: Some(Timestamp::WholeSeconds(now_sec)),
            cwt_id: None,
            rest: vec![(SCOPE_NAME.clone(), self.policies.into())],
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
        let claims =
            ClaimsSet::from_slice(data).map_err(|err| format!("invalid claims: {}", err))?;
        if let Some(ref exp) = claims.expiration_time {
            let exp = match exp {
                Timestamp::WholeSeconds(v) => *v,
                Timestamp::FractionalSeconds(v) => (*v).to_i64().unwrap_or_default(),
            };
            if exp < now_sec - CLOCK_SKEW {
                return Err("token expired".to_string());
            }
        }
        if let Some(ref nbf) = claims.not_before {
            let nbf = match nbf {
                Timestamp::WholeSeconds(v) => *v,
                Timestamp::FractionalSeconds(v) => (*v).to_i64().unwrap_or_default(),
            };
            if nbf > now_sec + CLOCK_SKEW {
                return Err("token not yet valid".to_string());
            }
        }
        Self::try_from(claims)
    }
}

/// algorithm: EdDSA | ES256K
pub fn cose_sign1(
    cs: ClaimsSet,
    alg: iana::Algorithm,
    key_id: Option<Vec<u8>>,
) -> Result<CoseSign1, String> {
    let payload = cs.to_vec().map_err(|err| err.to_string())?;
    let mut protected = HeaderBuilder::new().algorithm(alg);
    if let Some(key_id) = key_id {
        protected = protected.key_id(key_id);
    }

    Ok(CoseSign1Builder::new()
        .protected(protected.build())
        .payload(payload)
        .build())
}

impl TryFrom<ClaimsSet> for Token {
    type Error = String;

    fn try_from(claims: ClaimsSet) -> Result<Self, Self::Error> {
        let scope = claims
            .rest
            .iter()
            .find(|(key, _)| key == &SCOPE_NAME)
            .ok_or("missing scope")?;
        let scope = scope.1.as_text().ok_or("invalid scope text")?;

        Ok(Token {
            subject: Principal::from_text(claims.subject.as_ref().ok_or("missing subject")?)
                .map_err(|err| format!("invalid subject: {}", err))?,
            audience: Principal::from_text(claims.audience.as_ref().ok_or("missing audience")?)
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
        let tbs_data = sign1.tbs_data(BUCKET_TOKEN_AAD);
        let sig = signing_key.sign(&tbs_data).to_bytes();
        sign1.signature = sig.to_vec();
        let sign1_token = sign1.to_vec().unwrap();
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
