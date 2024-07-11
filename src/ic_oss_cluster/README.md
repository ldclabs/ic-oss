# `ic_oss_cluster`

[![CI](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml)

A cluster canister of [ic-oss](https://github.com/ldclabs/ic-oss).
`ic-oss` is a fully open-source decentralized object storage service running on the Internet Computer.

## Running locally

Deploy to local network:
```bash
dfx deploy ic_oss_cluster --argument "(opt variant {Init =
  record {
    ecdsa_key_name = \"dfx_test_key\";
  }
})"

dfx canister call ic_oss_cluster get_state '(null)'

MYID=$(dfx identity get-principal)

dfx canister call ic_oss_cluster admin_set_managers "(vec {principal \"$MYID\"})"

dfx canister call ic_oss_cluster admin_sign_access_token '(record {
  subject = principal "z7wjp-v6fe3-kksu5-26f64-dedtw-j7ndj-57onx-qga6c-et5e3-njx53-tae";
  audience = principal "mmrxu-fqaaa-aaaap-ahhna-cai";
  scope = "Folder.*:1 Bucket.Read.*";
})'
```

## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](LICENSE) for the full license text.