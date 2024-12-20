# `ic-oss-can`

![License](https://img.shields.io/crates/l/ic-oss.svg)
[![Crates.io](https://img.shields.io/crates/d/ic-oss.svg)](https://crates.io/crates/ic-oss)
[![CI](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml)
[![Docs.rs](https://img.shields.io/docsrs/ic-oss?label=docs.rs)](https://docs.rs/ic-oss)
[![Latest Version](https://img.shields.io/crates/v/ic-oss.svg)](https://crates.io/crates/ic-oss)

[ic-oss](https://github.com/ldclabs/ic-oss) is a decentralized Object Storage Service on the Internet Computer.

`ic-oss-can` is a Rust library for implementing large file storage in ICP canisters. By including the `ic_oss_fs!` macro in your canister, a `fs` module and a set of Candid filesystem APIs will be automatically generated. You can use the `ic-oss-cli` tool to upload files to the ICP canister.

## Usage

The following example is a minimal version using the `ic_oss_fs!` macro. Its only dependency is a thread-local constant named `FS_CHUNKS_STORE` of type `RefCell<StableBTreeMap<FileId, Chunk, Memory>>`.

```rust
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    DefaultMemoryImpl, StableBTreeMap,
};
use std::cell::RefCell;

use ic_oss_can::ic_oss_fs;
use ic_oss_can::types::{Chunk, FileId, FileMetadata};

type Memory = VirtualMemory<DefaultMemoryImpl>;

const FS_DATA_MEMORY_ID: MemoryId = MemoryId::new(0);

thread_local! {

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));


    // `FS_CHUNKS_STORE`` is needed by `ic_oss_can::ic_oss_fs!` macro
    static FS_CHUNKS_STORE: RefCell<StableBTreeMap<FileId, Chunk, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with_borrow(|m| m.get(FS_DATA_MEMORY_ID)),
        )
    );
}

// need to define `FS_CHUNKS_STORE` before `ic_oss_can::ic_oss_fs!()`
ic_oss_fs!();
```

For a more complete example, refer to [examples/ai_canister](https://github.com/ldclabs/ic-oss/tree/main/examples/ai_canister).

### FS Module

```rust
fs::set_max_file_size(size: u64);
fs::set_visibility(visibility: u8);
fs::set_managers(managers: BTreeSet<Principal>);
fs::is_manager(caller: &Principal) -> bool;
fs::with<R>(f: impl FnOnce(&Files) -> R) -> R;
fs::load();
fs::save();
fs::get_file(id: u32) -> Option<FileMetadata>;
fs::list_files(prev: u32, take: u32) -> Vec<FileInfo>;
fs::add_file(file: FileMetadata) -> Result<u32, String>;
fs::update_file(change: UpdateFileInput, now_ms: u64) -> Result<(), String>;
fs::get_chunk(id: u32, chunk_index: u32) -> Option<FileChunk>;
fs::get_full_chunks(id: u32) -> Result<Vec<u8>, String>;
fs::update_chunk(id: u32, chunk_index: u32, now_ms: u64, chunk: Vec<u8>) -> Result<u64, String>;
fs::delete_file(id: u32) -> Result<bool, String>;
```

### FS Candid API

```shell
create_file : (CreateFileInput, opt blob) -> (Result_2);
delete_file : (nat32, opt blob) -> (Result_3);
list_files : (nat32, opt nat32, opt nat32, opt blob) -> (Result_4) query;
update_file_chunk : (UpdateFileChunkInput, opt blob) -> (Result_6);
update_file_info : (UpdateFileInput, opt blob) -> (Result_7);
```

The complete module API Candid API definition can be found in the [store.rs](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_can/src/store.rs) file.

## License
Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for the full license text.
