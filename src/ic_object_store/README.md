# `ic_object_store`
![License](https://img.shields.io/crates/l/ic_object_store.svg)
[![Crates.io](https://img.shields.io/crates/d/ic_object_store.svg)](https://crates.io/crates/ic_object_store)
[![Test](https://github.com/ldclabs/ic-oss/actions/workflows/test.yml/badge.svg)](https://github.com/ldclabs/ic-oss/actions/workflows/test.yml)
[![Docs.rs](https://img.shields.io/docsrs/ic_object_store?label=docs.rs)](https://docs.rs/ic_object_store)
[![Latest Version](https://img.shields.io/crates/v/ic_object_store.svg)](https://crates.io/crates/ic_object_store)

[IC Object Store](https://github.com/ldclabs/ic-oss/tree/main/src/ic_object_store_canister) is a native Rust implementation of Apache Arrow object store on the Internet Computer.

`ic_object_store` is the Rust version of the client SDK for the IC Object Store canister.

## Overview

This library provides a Rust client SDK for interacting with the IC Object Store canister, which implements the Apache [Object Store](https://github.com/apache/arrow-rs-object-store) interface on the Internet Computer. It allows developers to seamlessly integrate with the decentralized storage capabilities of the Internet Computer using familiar Apache Object Store APIs.

## Features

- Full implementation of Apache Arrow object store APIs
- Secure data storage with AES256-GCM encryption
- Asynchronous stream operations for efficient data handling
- Seamless integration with the Internet Computer ecosystem
- Compatible with the broader Apache ecosystem

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
ic_object_store = "1.1"
```

## Usage

```rust
use ic_object_store::{Client, ObjectStoreClient, build_agent};
use object_store::ObjectStore;

let secret = [8u8; 32];
// backend: IC Object Store Canister
let canister = Principal::from_text("6at64-oyaaa-aaaap-anvza-cai").unwrap();
let sk = SigningKey::from(secret);
let id = BasicIdentity::from_signing_key(sk);
println!("id: {:?}", id.sender().unwrap().to_text());
// jjn6g-sh75l-r3cxb-wxrkl-frqld-6p6qq-d4ato-wske5-op7s5-n566f-bqe

let agent = build_agent("https://ic0.app", Arc::new(id))
    .await
    .unwrap();
let client = Arc::new(Client::new(Arc::new(agent), canister, Some(secret)));
let storage = ObjectStoreClient::new(client);

let path = Path::from("test/hello.txt");
let payload = "Hello Anda!".as_bytes().to_vec();
let res = storage
    .put_opts(&path, payload.into(), Default::default())
    .await
    .unwrap();
println!("put result: {:?}", res);

let res = storage.get_opts(&path, Default::default()).await.unwrap();
println!("get result: {:?}", res);
```

## Documentation

For detailed documentation, please visit: https://docs.rs/ic_object_store

## Related Projects

- [IC Object Store Canister](https://github.com/ldclabs/ic-oss/tree/main/src/ic_object_store_canister) - The canister implementation
- [IC-OSS](https://github.com/ldclabs/ic-oss) - A decentralized Object Storage Service on the Internet Computer

## License

Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

Licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for details.
