use candid::Principal;
use cbor2::{from_reader, to_writer};
use ed25519_dalek::{SigningKey, VerifyingKey};
use ic_oss_types::{
    cluster::{AddWasmInput, BucketDeploymentInfo, ClusterInfo},
    cose::sha256,
    permission::Policies,
};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, StableLog, Storable,
};
use serde::{Deserialize, Serialize};
use serde_bytes::{ByteArray, ByteBuf};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    ops::Bound::{Excluded, Unbounded},
};

use crate::chain_key;

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct State {
    #[serde(default, rename = "n", alias = "name")]
    pub name: String,
    #[serde(rename = "k", alias = "ecdsa_key_name")]
    pub ecdsa_key_name: String,
    #[serde(rename = "t", alias = "ecdsa_token_public_key")]
    pub ecdsa_token_public_key: String,
    #[serde(rename = "e", alias = "token_expiration")]
    pub token_expiration: u64, // in seconds
    #[serde(rename = "m", alias = "managers")]
    pub managers: BTreeSet<Principal>,
    #[serde(default, rename = "lv", alias = "bucket_latest_version")]
    pub bucket_latest_version: ByteArray<32>,
    // Legacy fields kept only so existing canisters can migrate their heap-backed
    // indexes to stable maps during the first upgrade to this version.
    #[serde(default, rename = "p", alias = "bucket_upgrade_path")]
    pub legacy_bucket_upgrade_path: BTreeMap<ByteArray<32>, ByteArray<32>>,
    #[serde(default, rename = "dl", alias = "bucket_deployed_list")]
    pub legacy_bucket_deployed_list: BTreeMap<Principal, (u64, ByteArray<32>)>,
    #[serde(default, rename = "up", alias = "bucket_upgrade_process")]
    pub bucket_upgrade_process: Option<ByteBuf>,
    #[serde(default, rename = "uc")]
    pub bucket_upgrade_cursor: Option<Principal>,
    #[serde(default, rename = "tt", alias = "bucket_topup_threshold")]
    pub bucket_topup_threshold: u128,
    #[serde(default, rename = "ta", alias = "bucket_topup_amount")]
    pub bucket_topup_amount: u128,
    #[serde(default, rename = "sk")]
    pub schnorr_key_name: String,
    #[serde(default, rename = "st")]
    pub schnorr_ed25519_token_public_key: String,
    #[serde(default, rename = "wk")]
    pub weak_ed25519_secret_key: ByteArray<32>, // should not be exposed
    #[serde(default, rename = "wt")]
    pub weak_ed25519_token_public_key: String,
    #[serde(default, rename = "gov")]
    pub governance_canister: Option<Principal>,
    #[serde(default, rename = "c")]
    pub committers: BTreeSet<Principal>,
}

impl Storable for State {
    const BOUND: Bound = Bound::Unbounded;

    fn into_bytes(self) -> Vec<u8> {
        let mut buf = vec![];
        to_writer(&self, &mut buf).expect("failed to encode State data");
        buf
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut buf = vec![];
        to_writer(self, &mut buf).expect("failed to encode State data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode State data")
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct PoliciesTable(pub BTreeMap<Principal, String>);

impl PoliciesTable {
    pub fn attach(&mut self, audience: Principal, mut policies: Policies) {
        self.0
            .entry(audience)
            .and_modify(|e| {
                let mut p = Policies::try_from(e.as_str()).expect("failed to parse policies");
                p.append(&mut policies);
                *e = p.to_string();
            })
            .or_insert_with(|| policies.to_string());
    }

    pub fn detach(&mut self, audience: Principal, policies: Policies) {
        let Some(e) = self.0.get(&audience) else {
            return;
        };
        let mut p = Policies::try_from(e.as_str()).expect("failed to parse policies");
        p.remove(&policies);
        // drop the entry entirely once nothing is left, an empty policies string
        // would otherwise keep the subject alive and still yield an access token
        if p.is_empty() {
            self.0.remove(&audience);
        } else {
            self.0.insert(audience, p.to_string());
        }
    }
}

impl Storable for PoliciesTable {
    const BOUND: Bound = Bound::Unbounded;

    fn into_bytes(self) -> Vec<u8> {
        let mut buf = vec![];
        to_writer(&self, &mut buf).expect("failed to encode Policies data");
        buf
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut buf = vec![];
        to_writer(self, &mut buf).expect("failed to encode Policies data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Policies data")
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Wasm {
    #[serde(rename = "a", alias = "created_at")]
    pub created_at: u64, // in milliseconds
    #[serde(rename = "b", alias = "created_by")]
    pub created_by: Principal,
    #[serde(rename = "d", alias = "description")]
    pub description: String,
    #[serde(rename = "w", alias = "wasm")]
    pub wasm: ByteBuf,
}

impl Storable for Wasm {
    const BOUND: Bound = Bound::Unbounded;

    fn into_bytes(self) -> Vec<u8> {
        let mut buf = vec![];
        to_writer(&self, &mut buf).expect("failed to encode Wasm data");
        buf
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut buf = vec![];
        to_writer(self, &mut buf).expect("failed to encode Wasm data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Wasm data")
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DeployLog {
    #[serde(rename = "d", alias = "deploy_at")]
    pub deploy_at: u64, // in milliseconds
    #[serde(rename = "c", alias = "canister")]
    pub canister: Principal,
    #[serde(rename = "p", alias = "prev_hash")]
    pub prev_hash: ByteArray<32>,
    #[serde(rename = "w", alias = "wasm_hash")]
    pub wasm_hash: ByteArray<32>,
    #[serde(rename = "a", alias = "args")]
    pub args: ByteBuf,
    #[serde(rename = "e", alias = "error")]
    pub error: Option<String>,
}

impl Storable for DeployLog {
    const BOUND: Bound = Bound::Unbounded;

    fn into_bytes(self) -> Vec<u8> {
        let mut buf = vec![];
        to_writer(&self, &mut buf).expect("failed to encode DeployLog data");
        buf
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut buf = vec![];
        to_writer(self, &mut buf).expect("failed to encode DeployLog data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode DeployLog data")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeploymentRecord {
    pub log_id: u64,
    pub wasm_hash: [u8; 32],
}

impl Storable for DeploymentRecord {
    const BOUND: Bound = Bound::Bounded {
        max_size: 40,
        is_fixed_size: true,
    };

    fn into_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(40);
        bytes.extend_from_slice(&self.log_id.to_le_bytes());
        bytes.extend_from_slice(&self.wasm_hash);
        bytes
    }

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Owned(self.into_bytes())
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        let bytes = bytes.as_ref();
        assert_eq!(bytes.len(), 40, "invalid DeploymentRecord length");

        let log_id = u64::from_le_bytes(bytes[..8].try_into().expect("invalid log id"));
        let wasm_hash = bytes[8..].try_into().expect("invalid wasm hash");
        Self { log_id, wasm_hash }
    }
}

const STATE_MEMORY_ID: MemoryId = MemoryId::new(0);
const AUTH_MEMORY_ID: MemoryId = MemoryId::new(1);
const WASM_MEMORY_ID: MemoryId = MemoryId::new(2);
const INSTALL_LOG_INDEX_MEMORY_ID: MemoryId = MemoryId::new(3);
const INSTALL_LOG_DATA_MEMORY_ID: MemoryId = MemoryId::new(4);
const UPGRADE_PATH_MEMORY_ID: MemoryId = MemoryId::new(5);
const DEPLOYED_BUCKET_MEMORY_ID: MemoryId = MemoryId::new(6);

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static STATE_STORE: RefCell<StableCell<State, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(STATE_MEMORY_ID)),
            State::default()
        )
    );

    static AUTH_STORE: RefCell<StableBTreeMap<Principal, PoliciesTable, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(AUTH_MEMORY_ID)),
        )
    );

    static WASM_STORE: RefCell<StableBTreeMap<[u8; 32], Wasm, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(WASM_MEMORY_ID)),
        )
    );

    static INSTALL_LOGS: RefCell<StableLog<DeployLog, Memory, Memory>> = RefCell::new(
        StableLog::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(INSTALL_LOG_INDEX_MEMORY_ID)),
            MEMORY_MANAGER.with_borrow(|m| m.get(INSTALL_LOG_DATA_MEMORY_ID)),
        )
    );

    static UPGRADE_PATH_STORE: RefCell<StableBTreeMap<[u8; 32], [u8; 32], Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(UPGRADE_PATH_MEMORY_ID)),
        )
    );

    static DEPLOYED_BUCKET_STORE: RefCell<StableBTreeMap<Principal, DeploymentRecord, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(DEPLOYED_BUCKET_MEMORY_ID)),
        )
    );
}

pub mod state {
    use super::*;

    pub fn is_controller(caller: &Principal) -> bool {
        with(|s| s.governance_canister.as_ref() == Some(caller))
    }

    pub fn is_manager(caller: &Principal) -> bool {
        with(|s| s.managers.contains(caller))
    }

    pub fn is_committer(caller: &Principal) -> bool {
        with(|s| s.committers.contains(caller))
    }

    pub fn get_cluster_info() -> ClusterInfo {
        with(|s| ClusterInfo {
            name: s.name.clone(),
            ecdsa_key_name: s.ecdsa_key_name.clone(),
            schnorr_key_name: s.schnorr_key_name.clone(),
            ecdsa_token_public_key: s.ecdsa_token_public_key.clone(),
            schnorr_ed25519_token_public_key: s.schnorr_ed25519_token_public_key.clone(),
            weak_ed25519_token_public_key: s.weak_ed25519_token_public_key.clone(),
            token_expiration: s.token_expiration,
            managers: s.managers.clone(),
            committers: s.committers.clone(),
            subject_authz_total: AUTH_STORE.with(|r| r.borrow().len()),
            bucket_latest_version: s.bucket_latest_version,
            bucket_wasm_total: WASM_STORE.with(|r| r.borrow().len()),
            bucket_deployed_total: DEPLOYED_BUCKET_STORE.with(|r| r.borrow().len()),
            bucket_deployment_logs: INSTALL_LOGS.with(|r| r.borrow().len()),
            governance_canister: s.governance_canister,
        })
    }

    pub fn with<R>(f: impl FnOnce(&State) -> R) -> R {
        STATE_STORE.with(|r| f(r.borrow().get()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut State) -> R) -> R {
        STATE_STORE.with(|r| {
            let mut store = r.borrow_mut();
            let mut state = store.get().clone();
            let result = f(&mut state);
            if store.get() != &state {
                store.set(state);
            }
            result
        })
    }

    /// Moves the two legacy, heap-backed indexes into dedicated stable maps.
    /// The migration is idempotent and the whole post-upgrade message commits
    /// atomically, so a trap cannot leave a half-migrated index behind.
    pub fn migrate_legacy_collections() {
        let (upgrade_path, deployed_buckets) = with_mut(|s| {
            (
                std::mem::take(&mut s.legacy_bucket_upgrade_path),
                std::mem::take(&mut s.legacy_bucket_deployed_list),
            )
        });

        UPGRADE_PATH_STORE.with(|r| {
            let mut store = r.borrow_mut();
            for (prev, next) in upgrade_path {
                store.insert(*prev, *next);
            }
        });

        DEPLOYED_BUCKET_STORE.with(|r| {
            let mut store = r.borrow_mut();
            for (canister, (log_id, wasm_hash)) in deployed_buckets {
                store.insert(
                    canister,
                    DeploymentRecord {
                        log_id,
                        wasm_hash: *wasm_hash,
                    },
                );
            }
        });
    }

    pub async fn try_init_public_key() {
        let (
            (ecdsa_key_name, ecdsa_token_public_key),
            (schnorr_key_name, schnorr_ed25519_token_public_key),
            weak_ed25519_token_public_key,
        ) = with(|s| {
            (
                (s.ecdsa_key_name.clone(), s.ecdsa_token_public_key.clone()),
                (
                    s.schnorr_key_name.clone(),
                    s.schnorr_ed25519_token_public_key.clone(),
                ),
                s.weak_ed25519_token_public_key.clone(),
            )
        });

        if ecdsa_token_public_key.is_empty() {
            let pk = chain_key::ecdsa_public_key(ecdsa_key_name.clone())
                .await
                .unwrap_or_else(|err| {
                    ic_cdk::trap(format!("failed to retrieve ECDSA public key: {err}"))
                });
            with_mut(|r| {
                r.ecdsa_token_public_key = hex::encode(pk.public_key);
            });
        }

        if schnorr_ed25519_token_public_key.is_empty() {
            let pk = chain_key::ed25519_public_key(schnorr_key_name)
                .await
                .unwrap_or_else(|err| {
                    ic_cdk::trap(format!("failed to retrieve schnorr public key: {err}"))
                });
            with_mut(|r| {
                r.schnorr_ed25519_token_public_key = hex::encode(pk.public_key);
            });
        }

        if weak_ed25519_token_public_key.is_empty() {
            let mut data = ic_cdk_management_canister::raw_rand()
                .await
                .expect("failed to generate weak_ed25519_secret_key");
            data.truncate(32);
            let secret_key: [u8; 32] = data
                .try_into()
                .expect("failed to generate weak_ed25519_secret_key");
            with_mut(|r| {
                let signing_key = SigningKey::from_bytes(&secret_key);
                let pub_key: &VerifyingKey = signing_key.as_ref();
                r.weak_ed25519_secret_key = secret_key.into();
                r.weak_ed25519_token_public_key = hex::encode(pub_key.to_bytes());
            });
        }
    }
}

pub mod auth {
    use super::*;

    pub fn get_all_policies(subject: &Principal) -> Option<PoliciesTable> {
        AUTH_STORE.with(|r| r.borrow().get(subject))
    }

    pub fn attach_policies(subject: Principal, audience: Principal, policies: Policies) {
        AUTH_STORE.with(|r| {
            let mut m = r.borrow_mut();
            let mut pt = m.get(&subject).unwrap_or_default();
            pt.attach(audience, policies);
            m.insert(subject, pt);
        });
    }

    pub fn detach_policies(subject: Principal, audience: Principal, policies: Policies) {
        AUTH_STORE.with(|r| {
            let mut m = r.borrow_mut();
            if let Some(mut pt) = m.get(&subject) {
                pt.detach(audience, policies);
                if pt.0.is_empty() {
                    m.remove(&subject);
                } else {
                    m.insert(subject, pt);
                }
            }
        });
    }
}

pub mod deployment {
    use super::*;

    pub fn contains(canister: &Principal) -> bool {
        DEPLOYED_BUCKET_STORE.with(|r| r.borrow().contains_key(canister))
    }

    pub fn ids() -> Vec<Principal> {
        DEPLOYED_BUCKET_STORE.with(|r| r.borrow().iter().map(|entry| *entry.key()).collect())
    }

    pub fn record(canister: Principal, log_id: u64, wasm_hash: ByteArray<32>) {
        DEPLOYED_BUCKET_STORE.with(|r| {
            r.borrow_mut().insert(
                canister,
                DeploymentRecord {
                    log_id,
                    wasm_hash: *wasm_hash,
                },
            );
        });
    }

    /// Finds the next bucket that has a known successor version. The cursor
    /// avoids repeatedly scanning already processed stable-map entries. Once
    /// the end is reached we wrap around, allowing multi-hop upgrade paths.
    pub fn next_upgrade(
        after: Option<Principal>,
    ) -> Option<(Principal, ByteArray<32>, ByteArray<32>)> {
        UPGRADE_PATH_STORE.with(|paths| {
            DEPLOYED_BUCKET_STORE.with(|deployments| {
                let paths = paths.borrow();
                let deployments = deployments.borrow();
                let find = |after: Option<Principal>| {
                    let visit = |canister: Principal, deployment: DeploymentRecord| {
                        paths.get(&deployment.wasm_hash).map(|next| {
                            (
                                canister,
                                ByteArray::from(deployment.wasm_hash),
                                ByteArray::from(next),
                            )
                        })
                    };

                    match after {
                        Some(cursor) => deployments
                            .range((Excluded(cursor), Unbounded))
                            .find_map(|entry| visit(*entry.key(), entry.value())),
                        None => deployments
                            .iter()
                            .find_map(|entry| visit(*entry.key(), entry.value())),
                    }
                };

                find(after).or_else(|| after.and_then(|_| find(None)))
            })
        })
    }

    pub fn get_deployed_buckets() -> Vec<BucketDeploymentInfo> {
        DEPLOYED_BUCKET_STORE.with(|deployments| {
            INSTALL_LOGS.with(|logs| {
                let deployments = deployments.borrow();
                let logs = logs.borrow();
                deployments
                    .iter()
                    .filter_map(|entry| {
                        let deployment = entry.value();
                        logs.get(deployment.log_id).map(|log| BucketDeploymentInfo {
                            deploy_at: log.deploy_at,
                            canister: log.canister,
                            prev_hash: log.prev_hash,
                            wasm_hash: log.wasm_hash,
                            args: None,
                            error: log.error,
                        })
                    })
                    .collect()
            })
        })
    }
}

pub mod wasm {
    use ic_oss_types::format_error;

    use super::*;

    pub fn add_wasm(
        caller: Principal,
        now_ms: u64,
        args: AddWasmInput,
        force_prev_hash: Option<ByteArray<32>>,
        dry_run: bool,
    ) -> Result<ByteArray<32>, String> {
        let hash: ByteArray<32> = sha256(&args.wasm).into();
        if WASM_STORE.with(|r| r.borrow().contains_key(&hash)) {
            return Err("wasm already exists".to_string());
        }

        let prev_hash = match force_prev_hash {
            Some(prev_hash) => {
                if !UPGRADE_PATH_STORE.with(|r| r.borrow().contains_key(&prev_hash)) {
                    return Err("force_prev_hash not exists".to_string());
                }
                prev_hash
            }
            None => state::with(|s| s.bucket_latest_version),
        };

        if dry_run {
            return Ok(hash);
        }

        UPGRADE_PATH_STORE.with(|r| {
            r.borrow_mut().insert(*prev_hash, *hash);
        });
        state::with_mut(|s| s.bucket_latest_version = hash);
        WASM_STORE.with(|r| {
            r.borrow_mut().insert(
                *hash,
                Wasm {
                    created_at: now_ms,
                    created_by: caller,
                    description: args.description,
                    wasm: args.wasm,
                },
            );
        });
        Ok(hash)
    }

    pub fn get_latest() -> Result<(ByteArray<32>, Wasm), String> {
        let hash = get_latest_hash()?;
        WASM_STORE.with(|r| {
            r.borrow()
                .get(&hash)
                .map(|wasm| (hash, wasm))
                .ok_or_else(|| "NotFound: latest wasm not found".to_string())
        })
    }

    pub fn get_latest_hash() -> Result<ByteArray<32>, String> {
        let hash = state::with(|s| s.bucket_latest_version);
        if contains_wasm(&hash) {
            Ok(hash)
        } else {
            Err("NotFound: latest wasm not found".to_string())
        }
    }

    pub fn contains_wasm(hash: &ByteArray<32>) -> bool {
        WASM_STORE.with(|r| r.borrow().contains_key(hash))
    }

    pub fn get_wasm(hash: &ByteArray<32>) -> Option<Wasm> {
        WASM_STORE.with(|r| r.borrow().get(hash))
    }

    pub fn next_version(prev_hash: ByteArray<32>) -> Result<(ByteArray<32>, Wasm), String> {
        let hash = UPGRADE_PATH_STORE
            .with(|r| r.borrow().get(&prev_hash))
            .ok_or_else(|| "no next version".to_string())?;
        WASM_STORE.with(|r| {
            let wasm = r
                .borrow()
                .get(&hash)
                .ok_or_else(|| "NotFound: next version not found".to_string())?;
            Ok((ByteArray::from(hash), wasm))
        })
    }

    pub fn add_log(log: DeployLog) -> Result<u64, String> {
        INSTALL_LOGS.with(|r| r.borrow_mut().append(&log).map_err(format_error))
    }

    pub fn bucket_deployment_logs(prev: Option<u64>, take: usize) -> Vec<BucketDeploymentInfo> {
        INSTALL_LOGS.with(|r| {
            let logs = r.borrow();
            let latest = logs.len();
            if latest == 0 {
                return vec![];
            }

            let prev = prev.unwrap_or(latest);
            if prev > latest || prev == 0 {
                return vec![];
            }

            if take == 0 {
                return vec![];
            }

            let mut idx = prev.saturating_sub(1);
            let mut res: Vec<BucketDeploymentInfo> = Vec::with_capacity(take);
            while let Some(log) = logs.get(idx) {
                res.push(BucketDeploymentInfo {
                    deploy_at: log.deploy_at,
                    canister: log.canister,
                    prev_hash: log.prev_hash,
                    wasm_hash: log.wasm_hash,
                    args: Some(log.args),
                    error: log.error,
                });

                if idx == 0 || res.len() >= take {
                    break;
                }
                idx -= 1;
            }
            res
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default, Serialize)]
    struct LegacyState {
        #[serde(rename = "n")]
        name: String,
        #[serde(rename = "k")]
        ecdsa_key_name: String,
        #[serde(rename = "t")]
        ecdsa_token_public_key: String,
        #[serde(rename = "e")]
        token_expiration: u64,
        #[serde(rename = "m")]
        managers: BTreeSet<Principal>,
        #[serde(rename = "lv")]
        bucket_latest_version: ByteArray<32>,
        #[serde(rename = "p")]
        bucket_upgrade_path: HashMap<ByteArray<32>, ByteArray<32>>,
        #[serde(rename = "dl")]
        bucket_deployed_list: BTreeMap<Principal, (u64, ByteArray<32>)>,
        #[serde(rename = "up")]
        bucket_upgrade_process: Option<ByteBuf>,
        #[serde(rename = "tt")]
        bucket_topup_threshold: u128,
        #[serde(rename = "ta")]
        bucket_topup_amount: u128,
        #[serde(rename = "sk")]
        schnorr_key_name: String,
        #[serde(rename = "st")]
        schnorr_ed25519_token_public_key: String,
        #[serde(rename = "wk")]
        weak_ed25519_secret_key: ByteArray<32>,
        #[serde(rename = "wt")]
        weak_ed25519_token_public_key: String,
        #[serde(rename = "gov")]
        governance_canister: Option<Principal>,
        #[serde(rename = "c")]
        committers: BTreeSet<Principal>,
    }

    #[test]
    fn test_state_decodes_legacy_hash_map_indexes() {
        let canister = Principal::from_slice(&[41, 1]);
        let prev_hash = ByteArray::from([3u8; 32]);
        let next_hash = ByteArray::from([4u8; 32]);
        let mut legacy = LegacyState::default();
        legacy.bucket_upgrade_path.insert(prev_hash, next_hash);
        legacy.bucket_deployed_list.insert(canister, (9, prev_hash));

        let mut bytes = Vec::new();
        to_writer(&legacy, &mut bytes).expect("failed to encode legacy state");
        let state = State::from_bytes(Cow::Owned(bytes));

        assert_eq!(
            state.legacy_bucket_upgrade_path.get(&prev_hash),
            Some(&next_hash)
        );
        assert_eq!(
            state.legacy_bucket_deployed_list.get(&canister),
            Some(&(9, prev_hash))
        );
    }

    #[test]
    fn test_deployment_record_roundtrip() {
        let record = DeploymentRecord {
            log_id: u64::MAX - 7,
            wasm_hash: [0xabu8; 32],
        };
        let bytes = record.to_bytes();
        assert_eq!(bytes.len(), 40);
        assert_eq!(DeploymentRecord::from_bytes(bytes), record);
    }

    #[test]
    fn test_legacy_indexes_migrate_to_stable_maps() {
        let canister = Principal::from_slice(&[42, 1]);
        let prev_hash = ByteArray::from([1u8; 32]);
        let next_hash = ByteArray::from([2u8; 32]);

        state::with_mut(|s| {
            s.legacy_bucket_upgrade_path.insert(prev_hash, next_hash);
            s.legacy_bucket_deployed_list
                .insert(canister, (7, prev_hash));
        });
        state::migrate_legacy_collections();

        assert!(state::with(
            |s| s.legacy_bucket_upgrade_path.is_empty() && s.legacy_bucket_deployed_list.is_empty()
        ));
        assert!(deployment::contains(&canister));
        assert_eq!(
            deployment::next_upgrade(None),
            Some((canister, prev_hash, next_hash))
        );

        deployment::record(canister, 8, next_hash);
        assert_eq!(deployment::next_upgrade(None), None);
    }

    #[test]
    fn test_policies_table_attach_detach() {
        let audience = Principal::anonymous();
        let mut pt = PoliciesTable::default();

        pt.attach(
            audience,
            Policies::try_from("Folder.Read.Folder:1").unwrap(),
        );
        pt.attach(
            audience,
            Policies::try_from("Folder.List.Folder:1").unwrap(),
        );
        let stored = pt.0.get(&audience).unwrap().clone();
        assert!(stored.contains("Folder.Read.Folder:1"), "{}", stored);
        assert!(stored.contains("Folder.List.Folder:1"), "{}", stored);

        pt.detach(
            audience,
            Policies::try_from("Folder.List.Folder:1").unwrap(),
        );
        assert_eq!(
            pt.0.get(&audience).map(|s| s.as_str()),
            Some("Folder.Read.Folder:1")
        );

        // removing the last policy drops the audience entry rather than leaving
        // an empty string behind, which would still hand out an access token
        pt.detach(
            audience,
            Policies::try_from("Folder.Read.Folder:1").unwrap(),
        );
        assert!(pt.0.is_empty());

        // detaching from an audience that was never attached is a no-op
        pt.detach(
            audience,
            Policies::try_from("Folder.Read.Folder:1").unwrap(),
        );
        assert!(pt.0.is_empty());
    }
}
