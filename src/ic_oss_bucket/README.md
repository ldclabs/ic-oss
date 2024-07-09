# `ic_oss_bucket`

[![CI](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml)

A bucket canister of [ic-oss](https://github.com/ldclabs/ic-oss).
`ic-oss` is a fully open-source decentralized object storage service running on the Internet Computer.

## Running locally

Deploy to local network:
```bash
dfx deploy ic_oss_bucket --argument "(opt variant {Init =
  record {
    name = \"LDC Labs\";
    file_id = 0;
    max_file_size = 0;
    max_folder_depth = 10;
    max_children = 1000;
    visibility = 0;
    max_custom_data_size = 4096;
    enable_hash_index = true;
  }
})"

dfx canister call ic_oss_bucket get_bucket_info '(null)'

MYID=$(dfx identity get-principal)
ic-oss-cli -i debug/uploader.pem identity
# principal: nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe

dfx canister call ic_oss_bucket admin_set_managers "(vec {principal \"$MYID\"; principal \"nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe\"})"

dfx canister call ic_oss_bucket list_files '(0, null, null, null)'
dfx canister call ic_oss_bucket list_folders '(0, null)'

ic-oss-cli -i debug/uploader.pem upload -b mmrxu-fqaaa-aaaap-ahhna-cai --file README.md

dfx canister call ic_oss_bucket get_file_info '(1, null)'

dfx canister call ic_oss_bucket update_file_info "(record {
  id = 1;
  status = opt 0;
}, null)"

dfx canister call ic_oss_bucket create_folder "(record {
  parent = 0;
  name = \"home\";
}, null)"
dfx canister call ic_oss_bucket list_folders '(0, null)'

dfx canister call ic_oss_bucket create_folder "(record {
  parent = 1;
  name = \"jarvis\";
}, null)"

dfx canister call ic_oss_bucket move_file "(record {
  id = 1;
  from = 0;
  to = 2;
}, null)"

dfx canister call ic_oss_bucket list_files '(2, null, null, null)'

dfx canister call ic_oss_bucket delete_file '(1, null)'
```

Download the file in browser:
- by file id: `http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/f/1`
- by file hash:  `http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/h/b7bb9040d64479a7ca56c8e03ae2daddc819859f7b858488c0b998eeded6dede`


## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](LICENSE) for the full license text.