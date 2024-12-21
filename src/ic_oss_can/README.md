# `ic-oss-can`

![License](https://img.shields.io/crates/l/ic-oss.svg)
[![Crates.io](https://img.shields.io/crates/d/ic-oss-can.svg)](https://crates.io/crates/ic-oss-can)
[![CI](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml)
[![Docs.rs](https://img.shields.io/docsrs/ic-oss-can?label=docs.rs)](https://docs.rs/ic-oss-can)
[![Latest Version](https://img.shields.io/crates/v/ic-oss-can.svg)](https://crates.io/crates/ic-oss-can)

A Rust library for implementing large file storage in Internet Computer (ICP) canisters. Part of the [ic-oss](https://github.com/ldclabs/ic-oss).

## Features

- Simple integration with the `ic_oss_fs!` macro
- Automatic generation of filesystem APIs in Candid format
- Using given `FS_CHUNKS_STORE` stable storage
- File chunk management and retrieval
- Access control with manager roles
- Compatible with `ic-oss-cli` for file uploads

## Quick Start

Add the following dependencies to your `Cargo.toml`:

```toml
[dependencies]
ic-oss-can = "0.9"
ic-oss-types = "0.9"
```

### Basic Implementation

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

## Available APIs

### Rust Module APIs

```rust
// File Management
fs::get_file(id: u32) -> Option<FileMetadata>;
fs::list_files(prev: u32, take: u32) -> Vec<FileInfo>;
fs::add_file(file: FileMetadata) -> Result<u32, String>;
fs::update_file(change: UpdateFileInput, now_ms: u64) -> Result<(), String>;
fs::delete_file(id: u32) -> Result<bool, String>;

// Chunk Operations
fs::get_chunk(id: u32, chunk_index: u32) -> Option<FileChunk>;
fs::get_full_chunks(id: u32) -> Result<Vec<u8>, String>;
fs::update_chunk(id: u32, chunk_index: u32, now_ms: u64, chunk: Vec<u8>) -> Result<u64, String>;

// Configuration
fs::set_max_file_size(size: u64);
fs::set_visibility(visibility: u8);
fs::set_managers(managers: BTreeSet<Principal>);
fs::is_manager(caller: &Principal) -> bool;
fs::with<R>(f: impl FnOnce(&Files) -> R) -> R;
fs::load();
fs::save();
```

### Candid Interface

```candid
create_file : (CreateFileInput, opt blob) -> (Result_2);
delete_file : (nat32, opt blob) -> (Result_3);
list_files : (nat32, opt nat32, opt nat32, opt blob) -> (Result_4) query;
update_file_chunk : (UpdateFileChunkInput, opt blob) -> (Result_6);
update_file_info : (UpdateFileInput, opt blob) -> (Result_7);
```

For complete API definitions and examples, see:
- [Full Example](https://github.com/ldclabs/ic-oss/tree/main/examples/ai_canister)

## License
Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

Licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for details.
