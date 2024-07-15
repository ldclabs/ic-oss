use candid::{CandidType, Principal};
use ciborium::{from_reader, into_writer};
use getrandom::register_custom_getrandom;
use ic_oss_can::types::{Chunk, FileId};
use ic_oss_types::file::*;
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    storable::Bound,
    DefaultMemoryImpl, StableBTreeMap, StableCell, Storable,
};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{borrow::Cow, cell::RefCell, collections::BTreeSet, time::Duration};

type Memory = VirtualMemory<DefaultMemoryImpl>;

const STATE_MEMORY_ID: MemoryId = MemoryId::new(0);
const FS_DATA_MEMORY_ID: MemoryId = MemoryId::new(1);

thread_local! {
    static RNG: RefCell<Option<StdRng>> = const { RefCell::new(None) };
    static STATE: RefCell<State> = RefCell::new(State::default());
    static AI_MODEL: RefCell<Option<AIModel>> = const { RefCell::new(None) };

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static STATE_STORE: RefCell<StableCell<State, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(STATE_MEMORY_ID)),
            State::default()
        ).expect("failed to init STATE_STORE store")
    );

    // `FS_CHUNKS_STORE`` is needed by `ic_oss_can::ic_oss_fs` macro
    static FS_CHUNKS_STORE: RefCell<StableBTreeMap<FileId, Chunk, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(FS_DATA_MEMORY_ID)),
        )
    );
}

// need to define `FS_CHUNKS_STORE` before `ic_oss_can::ic_oss_fs!()`
ic_oss_can::ic_oss_fs!();

async fn set_rand() {
    let (rr,) = ic_cdk::api::management_canister::main::raw_rand()
        .await
        .expect("failed to get random bytes");
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&rr);
    RNG.with(|rng| {
        *rng.borrow_mut() = Some(StdRng::from_seed(seed));
    });
}

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    RNG.with(|rng| rng.borrow_mut().as_mut().unwrap().fill_bytes(buf));
    Ok(())
}

pub fn init_rand() {
    ic_cdk_timers::set_timer(Duration::from_secs(0), || ic_cdk::spawn(set_rand()));
    register_custom_getrandom!(custom_getrandom);
}

#[derive(Default)]
pub struct AIModel {
    pub config: Vec<u8>,
    pub tokenizer: Vec<u8>,
    pub model: Vec<u8>,
}

#[derive(CandidType, Clone, Default, Deserialize, Serialize)]
pub struct State {
    pub ai_config: u32,
    pub ai_tokenizer: u32,
    pub ai_model: u32,
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

#[derive(CandidType, Clone, Deserialize)]
pub struct LoadModelInput {
    pub config_id: u32,
    pub tokenizer_id: u32,
    pub model_id: u32,
}

pub mod state {
    use super::*;

    pub fn with<R>(f: impl FnOnce(&State) -> R) -> R {
        STATE.with(|r| f(&r.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut State) -> R) -> R {
        STATE.with(|r| f(&mut r.borrow_mut()))
    }

    pub fn load() {
        STATE_STORE.with(|r| {
            STATE.with(|h| {
                *h.borrow_mut() = r.borrow().get().clone();
            });
        })
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

    pub fn load_model(args: &LoadModelInput) -> Result<(), String> {
        AI_MODEL.with(|r| {
            // let start = ic_cdk::api::performance_counter(1);
            *r.borrow_mut() = Some(AIModel {
                config: fs::get_full_chunks(args.config_id)?,
                tokenizer: fs::get_full_chunks(args.tokenizer_id)?,
                model: fs::get_full_chunks(args.model_id)?,
            });
            // ic_cdk::println!(
            //     "load_model_instructions: {}",
            //     ic_cdk::api::performance_counter(1) - start
            // );
            Ok(())
        })
    }

    // pub fn run_ai<W>(
    //     args: &ai::Args,
    //     prompt: &str,
    //     sample_len: usize,
    //     w: &mut W,
    // ) -> Result<u32, String>
    // where
    //     W: std::io::Write,
    // {
    //     AI_MODEL.with(|r| match r.borrow().as_ref() {
    //         None => Err("AI model not loaded".to_string()),
    //         Some(m) => {
    //             let mut ai = ai::TextGeneration::load(args, &m.config, &m.tokenizer, &m.model)
    //                 .map_err(|err| format!("{:?}", err))?;
    //             ai.run(prompt, sample_len, w)
    //                 .map_err(|err| format!("{:?}", err))
    //         }
    //     })
    // }
}

#[ic_cdk::query]
fn state() -> Result<State, ()> {
    Ok(state::with(|r| r.clone()))
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_managers(args: BTreeSet<Principal>) -> Result<(), String> {
    fs::set_managers(args);
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
fn admin_set_visibility(visibility: u8) -> Result<(), String> {
    fs::set_visibility(visibility);
    Ok(())
}

#[ic_cdk::update(guard = "is_controller")]
fn set_max_file_size(size: u64) -> Result<(), String> {
    fs::set_max_file_size(size);
    Ok(())
}

#[ic_cdk::update(guard = "is_controller_or_manager")]
fn admin_load_model(args: LoadModelInput) -> Result<u64, String> {
    state::load_model(&args)?;
    state::with_mut(|s| {
        s.ai_config = args.config_id;
        s.ai_tokenizer = args.tokenizer_id;
        s.ai_model = args.model_id;
    });

    Ok(ic_cdk::api::performance_counter(1))
}

#[ic_cdk::init]
fn init() {
    init_rand();
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    state::save();
    fs::save();
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    init_rand();
    state::load();
    fs::load();
    state::with(|s| {
        if s.ai_model > 0 {
            let _ = state::load_model(&LoadModelInput {
                config_id: s.ai_config,
                tokenizer_id: s.ai_tokenizer,
                model_id: s.ai_model,
            })
            .map_err(|err| ic_cdk::trap(&format!("failed to load model: {:?}", err)));
        }
    });
}

fn is_controller() -> Result<(), String> {
    let caller = ic_cdk::caller();
    if ic_cdk::api::is_controller(&caller) {
        Ok(())
    } else {
        Err("user is not a controller".to_string())
    }
}

fn is_controller_or_manager() -> Result<(), String> {
    let caller = ic_cdk::caller();
    if ic_cdk::api::is_controller(&caller) || fs::is_manager(&caller) {
        Ok(())
    } else {
        Err("user is not a controller or manager".to_string())
    }
}

ic_cdk::export_candid!();
