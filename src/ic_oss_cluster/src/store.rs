use candid::Principal;
use ciborium::{from_reader, into_writer};
use ic_oss_cose::{sha256, CLUSTER_TOKEN_AAD};
use ic_oss_types::{
    cluster::{AddWasmInput, BucketDeploymentInfo, ClusterInfo},
    permission::Policies,
    ByteN,
};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, StableLog, Storable,
};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
};

use crate::ecdsa;

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct State {
    #[serde(default)]
    pub name: String,
    pub ecdsa_key_name: String,
    pub ecdsa_token_public_key: String,
    pub token_expiration: u64, // in seconds
    pub managers: BTreeSet<Principal>,
    #[serde(default)]
    pub bucket_latest_version: ByteN<32>,
    #[serde(default)]
    pub bucket_upgrade_path: HashMap<ByteN<32>, ByteN<32>>,
    #[serde(default)]
    pub bucket_deployed_list: BTreeMap<Principal, (u64, ByteN<32>)>,
    #[serde(default)]
    pub bucket_upgrade_process: Option<ByteBuf>,
}

impl Storable for State {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode State data");
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
        self.0.entry(audience).and_modify(|e| {
            let mut p = Policies::try_from(e.as_str()).expect("failed to parse policies");
            p.remove(&policies);
            *e = p.to_string();
        });
    }
}

impl Storable for PoliciesTable {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode Policies data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Policies data")
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Wasm {
    pub created_at: u64, // in milliseconds
    pub created_by: Principal,
    pub description: String,
    pub wasm: ByteBuf,
}

impl Storable for Wasm {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode Wasm data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode Wasm data")
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DeployLog {
    pub deploy_at: u64, // in milliseconds
    pub canister: Principal,
    pub prev_hash: ByteN<32>,
    pub wasm_hash: ByteN<32>,
    pub args: ByteBuf,
    pub error: Option<String>,
}

impl Storable for DeployLog {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Cow<[u8]> {
        let mut buf = vec![];
        into_writer(self, &mut buf).expect("failed to encode DeployLog data");
        Cow::Owned(buf)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        from_reader(&bytes[..]).expect("failed to decode DeployLog data")
    }
}

const STATE_MEMORY_ID: MemoryId = MemoryId::new(0);
const AUTH_MEMORY_ID: MemoryId = MemoryId::new(1);
const WASM_MEMORY_ID: MemoryId = MemoryId::new(2);
const INSTALL_LOG_INDEX_MEMORY_ID: MemoryId = MemoryId::new(3);
const INSTALL_LOG_DATA_MEMORY_ID: MemoryId = MemoryId::new(4);

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static STATE_STORE: RefCell<StableCell<State, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(STATE_MEMORY_ID)),
            State::default()
        ).expect("failed to init STATE store")
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
        ).expect("failed to init INSTALL_LOGS store")
    );
}

pub mod state {
    use super::*;

    pub fn is_manager(caller: &Principal) -> bool {
        STATE.with(|r| r.borrow().managers.contains(caller))
    }

    pub fn get_cluster_info() -> ClusterInfo {
        with(|s| ClusterInfo {
            name: s.name.clone(),
            ecdsa_key_name: s.ecdsa_key_name.clone(),
            ecdsa_token_public_key: s.ecdsa_token_public_key.clone(),
            token_expiration: s.token_expiration,
            managers: s.managers.clone(),
            subject_authz_total: AUTH_STORE.with(|r| r.borrow().len()),
            bucket_latest_version: s.bucket_latest_version,
            bucket_wasm_total: WASM_STORE.with(|r| r.borrow().len()),
            bucket_deployed_total: s.bucket_deployed_list.len() as u64,
            bucket_deployment_logs: INSTALL_LOGS.with(|r| r.borrow().len()),
        })
    }

    pub fn with<R>(f: impl FnOnce(&State) -> R) -> R {
        STATE.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut State) -> R) -> R {
        STATE.with(|r| f(&mut r.borrow_mut()))
    }

    pub async fn init_ecdsa_public_key() {
        let ecdsa_key_name = with(|r| {
            if r.ecdsa_token_public_key.is_empty() && !r.ecdsa_key_name.is_empty() {
                Some(r.ecdsa_key_name.clone())
            } else {
                None
            }
        });

        if let Some(ecdsa_key_name) = ecdsa_key_name {
            let pk = ecdsa::public_key_with(&ecdsa_key_name, vec![CLUSTER_TOKEN_AAD.to_vec()])
                .await
                .unwrap_or_else(|err| {
                    ic_cdk::trap(&format!("failed to retrieve ECDSA public key: {err}"))
                });
            with_mut(|r| {
                r.ecdsa_token_public_key = hex::encode(pk.public_key);
            });
        }
    }

    pub fn load() {
        STATE_STORE.with(|r| {
            STATE.with(|h| {
                let s = r.borrow().get().to_owned();
                *h.borrow_mut() = s;
            });
        });
    }

    pub fn save() {
        STATE.with(|h| {
            STATE_STORE.with(|r| {
                r.borrow_mut()
                    .set(h.borrow().clone())
                    .expect("failed to set STATE data");
            });
        });
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

pub mod wasm {
    use ic_oss_types::format_error;

    use super::*;

    pub fn add_wasm(
        caller: Principal,
        now_ms: u64,
        args: AddWasmInput,
        force_prev_hash: Option<ByteN<32>>,
        dry_run: bool,
    ) -> Result<(), String> {
        WASM_STORE.with(|r| {
            if dry_run {
                let m = r.borrow();
                let hash: ByteN<32> = sha256(&args.wasm).into();
                if m.contains_key(&hash) {
                    return Err("wasm already exists".to_string());
                }

                return state::with(|s| {
                    if let Some(force_prev_hash) = force_prev_hash {
                        if !s.bucket_upgrade_path.contains_key(&force_prev_hash) {
                            Err("force_prev_hash not exists".to_string())?
                        }
                    };

                    Ok::<(), String>(())
                });
            }

            let mut m = r.borrow_mut();
            let hash: ByteN<32> = sha256(&args.wasm).into();
            if m.contains_key(&hash) {
                return Err("wasm already exists".to_string());
            }

            state::with_mut(|s| {
                let prev_hash = if let Some(force_prev_hash) = force_prev_hash {
                    if !s.bucket_upgrade_path.contains_key(&force_prev_hash) {
                        Err("force_prev_hash not exists".to_string())?
                    }
                    force_prev_hash
                } else {
                    s.bucket_latest_version
                };
                s.bucket_upgrade_path.insert(prev_hash, hash);
                s.bucket_latest_version = hash;
                Ok::<(), String>(())
            })?;
            m.insert(
                *hash,
                Wasm {
                    created_at: now_ms,
                    created_by: caller,
                    description: args.description,
                    wasm: args.wasm,
                },
            );
            Ok(())
        })
    }

    pub fn get_wasm(hash: &ByteN<32>) -> Option<Wasm> {
        WASM_STORE.with(|r| r.borrow().get(hash))
    }

    pub fn next_version(prev_hash: ByteN<32>) -> Result<(ByteN<32>, Wasm), String> {
        state::with(|s| {
            let h = s
                .bucket_upgrade_path
                .get(&prev_hash)
                .ok_or_else(|| "no next version".to_string())?;
            WASM_STORE.with(|r| {
                let w = r
                    .borrow()
                    .get(h)
                    .ok_or_else(|| "next version not found".to_string())?;
                Ok((*h, w))
            })
        })
    }

    pub fn add_log(log: DeployLog) -> Result<u64, String> {
        INSTALL_LOGS.with(|r| r.borrow_mut().append(&log).map_err(format_error))
    }

    pub fn get_deployed_buckets() -> Vec<BucketDeploymentInfo> {
        state::with(|s| {
            INSTALL_LOGS.with(|r| {
                let logs = r.borrow();
                s.bucket_deployed_list
                    .iter()
                    .filter_map(|(_, (id, _))| {
                        logs.get(*id).map(|log| BucketDeploymentInfo {
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
