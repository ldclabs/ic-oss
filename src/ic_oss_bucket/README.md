# `ic_oss_bucket`

A decentralized Object Storage Service bucket on the Internet Computer, part of [ic-oss](https://github.com/ldclabs/ic-oss).

## Overview

`ic_oss_bucket` is an ICP smart contract that functions as a storage bucket in the `ic-oss` cluster. Multiple buckets can be deployed for horizontal scaling, all managed by `ic_oss_cluster`.

## Features

- Supports large file uploads and downloads through file sharding, concurrent high-speed uploads, resumable uploads, and segmented downloads.
- Enables HTTP streaming and HTTP range downloads.
- Ensures file deduplication and retrieval using file hash indexing.
- Supports encrypted file storage and file-level encryption keys.
- Allows custom metadata for files.
- Provides a directory tree structure, enabling file and folder movement within the same bucket.
- Offers public and private modes for a bucket.
- Supports archive, read-write, and read-only status for files, folders, and buckets.
- Enables fine-grained access control for reading, writing, and deleting files, folders, and buckets.
- Includes auditors with the ability to read all contents within a bucket.

## Demo

Try it online: https://a4gq6-oaaaa-aaaab-qaa4q-cai.raw.icp0.io/?id=mmrxu-fqaaa-aaaap-ahhna-cai

Access file through HTTPs:
```
# Direct download
https://mmrxu-fqaaa-aaaap-ahhna-cai.icp0.io/f/1                   # By ID
https://mmrxu-fqaaa-aaaap-ahhna-cai.icp0.io/h/<file-hash>         # By hash, when enable_hash_index = true

# Download with filename
https://mmrxu-fqaaa-aaaap-ahhna-cai.icp0.io/f/1?filename=mydoc.md

# Download with token
https://mmrxu-fqaaa-aaaap-ahhna-cai.icp0.io/f/1?filename=mydoc.md&token=<access_token>

# Inline viewing
https://mmrxu-fqaaa-aaaap-ahhna-cai.icp0.io/f/1?inline
https://mmrxu-fqaaa-aaaap-ahhna-cai.icp0.io/f/2?inline
```

## Quick Start

### Local Deployment

1. Deploy the canister:
```bash
dfx deploy ic_oss_bucket
```

Or with custom configuration:
```bash
# dfx canister create --specified-id mmrxu-fqaaa-aaaap-ahhna-cai ic_oss_bucket
dfx deploy ic_oss_bucket --argument "(opt variant {Init =
  record {
    name = \"LDC Labs\";
    file_id = 0;
    max_file_size = 0;
    max_folder_depth = 10;
    max_children = 1000;
    visibility = 0;
    max_custom_data_size = 4096;
    enable_hash_index = false;
  }
})"
```

2. Set up permissions:
```bash
# Get your principal
MYID=$(dfx identity get-principal)
# Get the uploader principal
ic-oss-cli -i debug/uploader.pem identity
# principal: nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe

# Add managers
dfx canister call ic_oss_bucket admin_add_managers "(vec {principal \"$MYID\"; principal \"nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe\"})"

# Configure public keys and visibility
dfx canister call ic_oss_bucket admin_update_bucket '(record {
  visibility = opt 1;
  trusted_eddsa_pub_keys = opt vec {blob "..."}; # Your public key here
}, null)'
```

3. Basic operations:
```bash
# Get bucket info
dfx canister call ic_oss_bucket get_bucket_info '(null)'

# Upload a file
ic-oss-cli -i debug/uploader.pem put -b mmrxu-fqaaa-aaaap-ahhna-cai --path README.md

# Create folders
dfx canister call ic_oss_bucket create_folder '(record { parent = 0; name = "home"; }, null)'

# List contents
dfx canister call ic_oss_bucket list_files '(0, null, null, null)'    # Files
dfx canister call ic_oss_bucket list_folders '(0, null, null, null)'  # Folders
```

## API Reference

The canister exposes a comprehensive Candid API. Key endpoints include:

```candid
# File Operations
create_file : (CreateFileInput, opt blob) -> (Result_2)
update_file_chunk : (UpdateFileChunkInput, opt blob) -> (Result_13)
update_file_info : (UpdateFileInput, opt blob) -> (Result_12)
get_file_info : (nat32, opt blob) -> (Result_8) query
get_file_chunks : (nat32, nat32, opt nat32, opt blob) -> (Result_7) query
list_files : (nat32, opt nat32, opt nat32, opt blob) -> (Result_10) query
delete_file : (nat32, opt blob) -> (Result_3)

# Folder Operations
create_folder : (CreateFolderInput, opt blob) -> (Result_2)
list_folders : (nat32, opt nat32, opt nat32, opt blob) -> (Result_11) query
delete_folder : (nat32, opt blob) -> (Result_3)

# Admin Operations
admin_add_managers : (vec principal) -> (Result)
admin_update_bucket : (UpdateBucketInput) -> (Result)
```

Full Candid API definition: [ic_oss_bucket.did](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_bucket/ic_oss_bucket.did)

## License

Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

Licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for details.