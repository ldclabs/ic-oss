# `ic_oss_cluster` (WIP)

[ic-oss](https://github.com/ldclabs/ic-oss) is a decentralized Object Storage Service on the Internet Computer.

`ic_oss_cluster` is an ICP smart contract and the manager of the `ic-oss` cluster.

**Online Demo**: https://a4gq6-oaaaa-aaaab-qaa4q-cai.raw.icp0.io/?id=x5573-nqaaa-aaaap-ahopq-cai

## Features

- [x] Manages access control policies and issue access tokens for users.
- [ ] Manages `ic_oss_bucket` smart contract versions, including deploying and upgrading buckets.
- [ ] Manages associated keys for file data encryption.
- [ ] Manages extension schemas for integrating external file systems.

## Candid API

```shell
access_token : (principal) -> (Result);
admin_attach_policies : (Token) -> (Result_1);
admin_detach_policies : (Token) -> (Result_1);
admin_set_managers : (vec principal) -> (Result_1);
admin_sign_access_token : (Token) -> (Result);
get_cluster_info : () -> (Result_2) query;
validate_admin_set_managers : (vec principal) -> (Result_1);
```

The complete Candid API definition can be found in the [ic_oss_cluster.did](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_bucket/ic_oss_cluster.did) file.

## Running locally

Deploy to local network:
```bash
dfx deploy ic_oss_cluster --argument "(opt variant {Init =
  record {
    name = \"LDC Labs\";
    ecdsa_key_name = \"dfx_test_key\";
    token_expiration = 3600;
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
```

## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for the full license text.