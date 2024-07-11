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

dfx canister call ic_oss_bucket admin_update_bucket '(record {
  name = null;
  max_file_size = null;
  max_folder_depth = null;
  max_children = null;
  max_custom_data_size = null;
  enable_hash_index = null;
  status = null;
  visibility = null;
  trusted_ecdsa_pub_keys = opt vec {blob "\02\bd\ef\d5\d8\91\7a\81\cc\91\60\ba\19\95\69\d4\47\d9\d4\7e\e6\71\6c\b8\dc\18\aa\d2\be\8c\4c\cd\eb"};
  trusted_eddsa_pub_keys = opt vec {vec {19; 152; 246; 44; 109; 26; 69; 124; 81; 186; 106; 75; 95; 61; 189; 47; 105; 252; 169; 50; 22; 33; 141; 200; 153; 126; 65; 107; 209; 125; 147; 202}};
}, null)'

dfx canister call ic_oss_bucket list_files '(2, null, null, opt blob "\84\44\a1\01\38\2e\a0\58\ac\a7\01\78\1b\61\6a\75\71\34\2d\72\75\61\61\61\2d\61\61\61\61\61\2d\71\61\61\67\61\2d\63\61\69\02\78\3f\7a\37\77\6a\70\2d\76\36\66\65\33\2d\6b\6b\73\75\35\2d\32\36\66\36\34\2d\64\65\64\74\77\2d\6a\37\6e\64\6a\2d\35\37\6f\6e\78\2d\71\67\61\36\63\2d\65\74\35\65\33\2d\6e\6a\78\35\33\2d\74\61\65\03\78\1b\6d\6d\72\78\75\2d\66\71\61\61\61\2d\61\61\61\61\70\2d\61\68\68\6e\61\2d\63\61\69\04\1a\66\8f\ce\68\05\1a\66\8f\c0\58\06\1a\66\8f\c0\58\09\78\18\46\6f\6c\64\65\72\2e\2a\3a\31\20\42\75\63\6b\65\74\2e\52\65\61\64\2e\2a\58\40\52\66\3e\e7\55\7e\99\2c\66\6d\65\56\54\9f\30\a1\2e\aa\56\69\66\b6\c6\e9\75\d7\c9\02\4c\24\1d\5d\7e\83\7d\c1\13\c6\00\91\56\d9\6a\ae\34\c3\a5\c9\b4\99\b3\47\b7\68\54\8d\dd\9c\9a\9b\a0\f9\1a\f5")'
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