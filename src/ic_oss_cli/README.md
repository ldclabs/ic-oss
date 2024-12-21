# `ic-oss-cli`
![License](https://img.shields.io/crates/l/ic-oss-cli.svg)
[![Crates.io](https://img.shields.io/crates/d/ic-oss-cli.svg)](https://crates.io/crates/ic-oss-cli)
[![Test](https://github.com/ldclabs/ic-oss/actions/workflows/test.yml/badge.svg)](https://github.com/ldclabs/ic-oss/actions/workflows/test.yml)
[![Docs.rs](https://img.shields.io/docsrs/ic-oss-cli?label=docs.rs)](https://docs.rs/ic-oss-cli)
[![Latest Version](https://img.shields.io/crates/v/ic-oss-cli.svg)](https://crates.io/crates/ic-oss-cli)

A command-line tool implemented in Rust for [ic-oss](https://github.com/ldclabs/ic-oss), a decentralized Object Storage Service on the Internet Computer.

## Installation

### Via Cargo
```sh
cargo install ic-oss-cli
# get help info
ic-oss-cli --help
```

### From Source
```sh
git clone https://github.com/ldclabs/ic-oss.git
cd ic-oss
cargo build -p ic-oss-cli --release
# get help info
target/release/ic-oss-cli --help
```

## Quick Start

### Identity Management
```sh
# Generate a new identity
ic-oss-cli identity --new --path myid.pem

# Expected output:
# principal: lxph3-nvpsv-yrevd-im4ug-qywcl-5ir34-rpsbs-6olvf-qtugo-iy5ai-jqe
# new identity: myid.pem
```

### File Operations
```sh
# Upload to local canister
ic-oss-cli -i myid.pem put -b mmrxu-fqaaa-aaaap-ahhna-cai --path test.tar.gz

# Upload to mainnet canister
ic-oss-cli -i myid.pem put -b mmrxu-fqaaa-aaaap-ahhna-cai --path test.tar.gz --ic

# Add WASM to cluster
ic-oss-cli -i debug/uploader.pem cluster-add-wasm \
    -c x5573-nqaaa-aaaap-ahopq-cai \
    --path target/wasm32-unknown-unknown/release/ic_oss_bucket.wasm
```

## Documentation
For detailed usage instructions:
```sh
ic-oss-cli --help
ic-oss-cli identity --help
ic-oss-cli upload --help
```

## License

Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

Licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for details.