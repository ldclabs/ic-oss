# `ic_oss_cluster`

[ic-oss](https://github.com/ldclabs/ic-oss) is a decentralized Object Storage Service on the Internet Computer.

`ic_oss_cluster` is an ICP smart contract and the manager of the `ic-oss` cluster.

**Online Demo**: https://a4gq6-oaaaa-aaaab-qaa4q-cai.raw.icp0.io/?id=x5573-nqaaa-aaaap-ahopq-cai

## Features

- [x] Manages access control policies and issue access tokens for users.
- [x] Manages `ic_oss_bucket` smart contract versions, including deploying and upgrading buckets.

## Candid API

```shell
access_token : (principal) -> (Result);
admin_add_committers : (vec principal) -> (Result_1);
admin_add_managers : (vec principal) -> (Result_1);
admin_add_wasm : (AddWasmInput, opt blob) -> (Result_1);
admin_attach_policies : (Token) -> (Result_1);
admin_batch_call_buckets : (vec principal, text, opt blob) -> (Result_2);
admin_create_bucket : (opt CanisterSettings, opt blob) -> (Result_3);
admin_deploy_bucket : (DeployWasmInput, opt blob) -> (Result_1);
admin_detach_policies : (Token) -> (Result_1);
admin_ed25519_access_token : (Token) -> (Result);
admin_remove_committers : (vec principal) -> (Result_1);
admin_remove_managers : (vec principal) -> (Result_1);
admin_set_managers : (vec principal) -> (Result_1);
admin_sign_access_token : (Token) -> (Result);
admin_topup_all_buckets : () -> (Result_4);
admin_update_bucket_canister_settings : (UpdateSettingsArgument) -> (
    Result_1,
  );
admin_upgrade_all_buckets : (opt blob) -> (Result_1);
admin_weak_access_token : (Token, nat64, nat64) -> (Result) query;
bucket_deployment_logs : (opt nat, opt nat) -> (Result_5) query;
ed25519_access_token : (principal) -> (Result);
get_bucket_wasm : (blob) -> (Result_6) query;
get_buckets : () -> (Result_7) query;
get_canister_status : (opt principal) -> (Result_8);
get_cluster_info : () -> (Result_9) query;
get_deployed_buckets : () -> (Result_5) query;
get_subject_policies : (principal) -> (Result_10) query;
get_subject_policies_for : (principal, principal) -> (Result_11) query;
validate2_admin_add_wasm : (AddWasmInput, opt blob) -> (Result_11);
validate2_admin_batch_call_buckets : (vec principal, text, opt blob) -> (
    Result_11,
  );
validate2_admin_deploy_bucket : (DeployWasmInput, opt blob) -> (Result_11);
validate2_admin_set_managers : (vec principal) -> (Result_11);
validate2_admin_upgrade_all_buckets : (opt blob) -> (Result_11);
validate_admin_add_committers : (vec principal) -> (Result_11);
validate_admin_add_managers : (vec principal) -> (Result_11);
validate_admin_add_wasm : (AddWasmInput, opt blob) -> (Result_1);
validate_admin_batch_call_buckets : (vec principal, text, opt blob) -> (
    Result_2,
  );
validate_admin_create_bucket : (opt CanisterSettings, opt blob) -> (
    Result_11,
  );
validate_admin_deploy_bucket : (DeployWasmInput, opt blob) -> (Result_1);
validate_admin_remove_committers : (vec principal) -> (Result_11);
validate_admin_remove_managers : (vec principal) -> (Result_11);
validate_admin_set_managers : (vec principal) -> (Result_1);
validate_admin_update_bucket_canister_settings : (UpdateSettingsArgument) -> (
    Result_11,
  );
validate_admin_upgrade_all_buckets : (opt blob) -> (Result_1);
```

The complete Candid API definition can be found in the [ic_oss_cluster.did](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_bucket/ic_oss_cluster.did) file.

## Running locally

Deploy to local network:
```bash
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

dfx canister call ic_oss_cluster get_cluster_info '()'

MYID=$(dfx identity get-principal)
ic-oss-cli -i debug/uploader.pem identity
# principal: nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe

# add managers
dfx canister call ic_oss_cluster admin_set_managers "(vec {principal \"$MYID\"; principal \"nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe\"})"

# add managers
dfx canister call ic_oss_cluster admin_set_managers "(vec {principal \"$MYID\"})"

# sign a access token
dfx canister call ic_oss_cluster admin_sign_access_token '(record {
  subject = principal "z7wjp-v6fe3-kksu5-26f64-dedtw-j7ndj-57onx-qga6c-et5e3-njx53-tae";
  audience = principal "mmrxu-fqaaa-aaaap-ahhna-cai";
  scope = "Folder.*:1 Bucket.Read.*";
})'

# attach policies
dfx canister call ic_oss_cluster admin_attach_policies '(record {
  subject = principal "z7wjp-v6fe3-kksu5-26f64-dedtw-j7ndj-57onx-qga6c-et5e3-njx53-tae";
  audience = principal "mmrxu-fqaaa-aaaap-ahhna-cai";
  scope = "Folder.*  Bucket.List.*";
})'

# detach policies
dfx canister call ic_oss_cluster admin_detach_policies '(record {
  subject = principal "z7wjp-v6fe3-kksu5-26f64-dedtw-j7ndj-57onx-qga6c-et5e3-njx53-tae";
  audience = principal "mmrxu-fqaaa-aaaap-ahhna-cai";
  scope = "Folder.*:1";
})'

# get access token for a audience
dfx canister call ic_oss_cluster access_token '(principal "mmrxu-fqaaa-aaaap-ahhna-cai")'


# Add a wasm file to the cluster:
ic-oss-cli -i debug/uploader.pem cluster-add-wasm -c x5573-nqaaa-aaaap-ahopq-cai --path debug/ic_oss_bucket.wasm.gz --description "ic_oss_bucket v0.9.8"

# get wasm file
shasum -a 256 debug/ic_oss_bucket.wasm.gz
dfx canister call ic_oss_cluster get_bucket_wasm '(blob "\2d\f4\25\d7\ed\ea\ba\3c\27\39\f0\f5\66\73\90\66\69\5c\f1\8c\53\fd\38\cf\9b\ef\cb\14\e9\f6\22\57")'

# create a bucket with default settings
dfx canister call ic_oss_cluster admin_create_bucket '(null, null)'
# (variant { Ok = principal "ctiya-peaaa-aaaaa-qaaja-cai" })

# get canister status
dfx canister call ic_oss_cluster get_canister_status '(null)'
dfx canister call ic_oss_cluster get_canister_status '(opt principal "ctiya-peaaa-aaaaa-qaaja-cai")'
```

## License
Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for the full license text.