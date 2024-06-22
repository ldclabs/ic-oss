# `ic-oss`

A decentralized Object Storage Service on the Internet Computer.

## Overview

`ic-oss` is a fully open-source decentralized object storage service running on the Internet Computer. It provides a simple and efficient way to store and retrieve files, supports large files, and offers unlimited horizontal scalability. It can serve as a reliable decentralized file infrastructure for NFT, chain blocks, verifiable credentials, blogs, documents, knowledge bases, games and other decentralized applications.

In decentralized enterprise applications, `ic-oss` will be an essential infrastructure.

`ic-oss` is a file infrastructure service, not a user-facing file product, but it will provide a simple management interface.

> [!NOTE]
> `ic-oss` is in development and is not suitable for production use yet.

## Features

- [x] Supports large file uploads and downloads through file sharding, concurrent high-speed uploads, resumable uploads, and segmented downloads.
- [x] Provides data verification based on ICP's verification mechanisms to ensure file integrity during reading.
- [ ] Supports file directory tree.
- [ ] Access control with permissions for public, private, read-only, and write-only for files, folders, and buckets.
- [ ] Based on a file bucket and cluster architecture, with each bucket corresponding to a ICP canister, allowing for unlimited horizontal scalability.
- [ ] Compatible with S3 core API protocol and supports S3 SDK.
- [ ] Implements file encryption storage using ICP's vetKeys mechanism.
- [ ] Integrates with external storage, supporting file storage in decentralized file services like IPFS and Arweave, with `ic-oss` managing file metadata.

## Running the project locally

If you want to test your project locally, you can use the following commands:

Deploy the bucket canister:
```bash
dfx canister create --specified-id mmrxu-fqaaa-aaaap-ahhna-cai ic-oss-bucket
dfx deploy ic-oss-bucket
# Output:
# ...
# Installing code for canister ic-oss-bucket, with canister ID mmrxu-fqaaa-aaaap-ahhna-cai
# Deployed canisters.
# URLs:
#   Backend canister via Candid interface:
#     ic-oss-bucket: http://127.0.0.1:4943/?canisterId=bd3sg-teaaa-aaaaa-qaaba-cai&id=mmrxu-fqaaa-aaaap-ahhna-cai
```

Build cli tool:
```bash
cargo build -p ic-oss-cli

# Run the cli tool
./target/debug/ic-oss-cli --help
./target/debug/ic-oss-cli identity --help
./target/debug/ic-oss-cli upload --help

# Generate a new identity
./target/debug/ic-oss-cli identity --new --file myid.pem
# Output:
# principal: lxph3-nvpsv-yrevd-im4ug-qywcl-5ir34-rpsbs-6olvf-qtugo-iy5ai-jqe
# new identity: myid.pem
```

Add a manager to the bucket canister:
```bash
dfx canister call mmrxu-fqaaa-aaaap-ahhna-cai admin_set_managers '(vec {principal "lxph3-nvpsv-yrevd-im4ug-qywcl-5ir34-rpsbs-6olvf-qtugo-iy5ai-jqe"})'
```

Upload a file to the bucket canister:
```bash
./target/debug/ic-oss-cli -i myid.pem upload -b mmrxu-fqaaa-aaaap-ahhna-cai --file test.tar.gz
# Output:
# ...
# 2024-05-18 18:42:38 uploaded: 99.48%
# 2024-05-18 18:42:38 uploaded: 99.66%
# 2024-05-18 18:42:38 uploaded: 99.82%
# 2024-05-18 18:42:38 uploaded: 100.00%
# upload success, file id: 1, size: 147832281, chunks: 564, retry: 0, time elapsed: PT69.149941S
```

List files:
```bash
dfx canister call ic-oss-bucket list_files '(0, null, null, null)'
```

Get file info:
```bash
dfx canister call ic-oss-bucket get_file_info '(1, null)'
# Output:
# (
#   variant {
#     Ok = record {
#       id = 1 : nat32;
#       parent = 0 : nat32;
#       status = 0 : int8;
#       updated_at = 1_716_028_957_265 : nat;
#       hash = opt blob "\b7\bb\90\40\d6\44\79\a7\ca\56\c8\e0\3a\e2\da\dd\c8\19\85\9f\7b\85\84\88\c0\b9\98\ee\de\d6\de\de";
#       name = "test.tar.gz";
#       size = 147_832_281 : nat;
#       content_type = "application/gzip";
#       created_at = 1_716_028_890_649 : nat;
#       filled = 147_832_281 : nat;
#       chunks = 564 : nat32;
#       ert = null;
#     }
#   },
# )

dfx canister call ic-oss-bucket get_file_info_by_hash '(blob "\b7\bb\90\40\d6\44\79\a7\ca\56\c8\e0\3a\e2\da\dd\c8\19\85\9f\7b\85\84\88\c0\b9\98\ee\de\d6\de\de", null)'
```

Delete file:
```bash
dfx canister call ic-oss-bucket delete_file '(1, null)'
```

Download the file in browser:
- by file id: `http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/f/1`
- by file hash:  `http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/h/b7bb9040d64479a7ca56c8e03ae2daddc819859f7b858488c0b998eeded6dede`

## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](LICENSE-MIT) for the full license text.