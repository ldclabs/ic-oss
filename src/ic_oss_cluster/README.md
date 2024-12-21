# `ic_oss_cluster`

A decentralized Object Storage Service manager on the Internet Computer, part of [ic-oss](https://github.com/ldclabs/ic-oss).

## Features

- Bucket permission policies management and access_token issuance
- Bucket deployment management
- Bucket recharge management

## Demo

Try it online: https://a4gq6-oaaaa-aaaab-qaa4q-cai.raw.icp0.io/?id=x5573-nqaaa-aaaap-ahopq-cai

## Quick Start

### Deploy Locally

```bash
# Deploy the cluster
# dfx canister create --specified-id x5573-nqaaa-aaaap-ahopq-cai ic_oss_cluster
dfx deploy ic_oss_cluster --argument "(opt variant {Init =
  record {
    name = \"LDC Labs\";
    ecdsa_key_name = \"dfx_test_key\";
    schnorr_key_name = \"dfx_test_key\";
    token_expiration = 3600;
    bucket_topup_threshold = 1_000_000_000_000;
    bucket_topup_amount = 5_000_000_000_000;
  }
})"

# Get cluster info
dfx canister call ic_oss_cluster get_cluster_info '()'
```

### Common Operations

```bash
# Add managers
MYID=$(dfx identity get-principal)
dfx canister call ic_oss_cluster admin_add_managers "(vec {principal \"$MYID\"})"

# Add a wasm file to the cluster:
ic-oss-cli -i debug/uploader.pem cluster-add-wasm -c x5573-nqaaa-aaaap-ahopq-cai --path debug/ic_oss_bucket.wasm.gz --description "ic_oss_bucket v0.9.8"

# create a bucket with default settings
dfx canister call ic_oss_cluster admin_create_bucket '(null, null)'
# (variant { Ok = principal "ctiya-peaaa-aaaaa-qaaja-cai" })

# Get bucket status
dfx canister call ic_oss_cluster get_canister_status '(opt principal "YOUR_BUCKET_ID")'
```

### Access Control Examples

```bash
# Sign access token
dfx canister call ic_oss_cluster admin_ed25519_access_token '(record {
  subject = principal "USER_ID";
  audience = principal "YOUR_BUCKET_ID";
  scope = "Folder.*:1 Bucket.Read.*";
})'

# Attach policies
dfx canister call ic_oss_cluster admin_attach_policies '(record {
  subject = principal "USER_ID";
  audience = principal "YOUR_BUCKET_ID";
  scope = "Folder.* Bucket.List.*";
})'
```

## API Reference

The canister exposes a comprehensive Candid API. Key endpoints include:

```candid
# Permissions Operations
admin_attach_policies : (Token) -> (Result_1)
get_subject_policies : (principal) -> (Result_10) query
admin_ed25519_access_token : (Token) -> (Result)
admin_weak_access_token : (Token, nat64, nat64) -> (Result) query
access_token : (principal) -> (Result)

# Buckets Operations
admin_add_wasm : (AddWasmInput, opt blob) -> (Result_1)
admin_create_bucket : (opt CanisterSettings, opt blob) -> (Result_3)
admin_deploy_bucket : (DeployWasmInput, opt blob) -> (Result_1)
admin_upgrade_all_buckets : (opt blob) -> (Result_1)
admin_topup_all_buckets : () -> (Result_4)
bucket_deployment_logs : (opt nat, opt nat) -> (Result_5) query

# Admin Operations
admin_add_managers : (vec principal) -> (Result_1)
admin_add_committers : (vec principal) -> (Result_1)
```

Full Candid API definition: [ic_oss_bucket.did](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_cluster/ic_oss_cluster.did)

## License

Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

Licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for details.