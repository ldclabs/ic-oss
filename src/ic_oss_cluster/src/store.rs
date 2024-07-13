use candid::{CandidType, Principal};
use ciborium::{from_reader, into_writer};
use ic_oss_cose::CLUSTER_TOKEN_AAD;
use ic_oss_types::permission::Policies;
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, Storable,
};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use crate::ecdsa;

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Clone, Default, Deserialize, Serialize)]
pub struct State {
    #[serde(default)]
    pub name: String,
    pub ecdsa_key_name: String,
    pub ecdsa_token_public_key: String,
    pub token_expiration: u64, // in seconds
    pub managers: BTreeSet<Principal>,
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

const STATE_MEMORY_ID: MemoryId = MemoryId::new(0);
const AUTH_MEMORY_ID: MemoryId = MemoryId::new(0);

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

}

pub mod state {
    use super::*;

    pub fn is_manager(caller: &Principal) -> bool {
        STATE.with(|r| r.borrow().managers.contains(caller))
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
            let s = r.borrow_mut().get().clone();
            STATE.with(|h| {
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
