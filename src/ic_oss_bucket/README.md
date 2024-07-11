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

dfx canister call ic_oss_bucket admin_update_bucket "(record {
  name = null;
  max_file_size = null;
  max_folder_depth = null;
  max_children = null;
  max_custom_data_size = null;
  enable_hash_index = null;
  status = null;
  visibility = null;
  trusted_ecdsa_pub_keys = null;
  trusted_eddsa_pub_keys = opt vec {vec {19; 152; 246; 44; 109; 26; 69; 124; 81; 186; 106; 75; 95; 61; 189; 47; 105; 252; 169; 50; 22; 33; 141; 200; 153; 126; 65; 107; 209; 125; 147; 202}};
}, null)"

dfx canister call ic_oss_bucket list_files '(2, null, null, opt vec{132; 67; 161; 1; 39; 160; 88; 142; 166; 2; 120; 63; 122; 55; 119; 106; 112; 45; 118; 54; 102; 101; 51; 45; 107; 107; 115; 117; 53; 45; 50; 54; 102; 54; 52; 45; 100; 101; 100; 116; 119; 45; 106; 55; 110; 100; 106; 45; 53; 55; 111; 110; 120; 45; 113; 103; 97; 54; 99; 45; 101; 116; 53; 101; 51; 45; 110; 106; 120; 53; 51; 45; 116; 97; 101; 3; 120; 27; 109; 109; 114; 120; 117; 45; 102; 113; 97; 97; 97; 45; 97; 97; 97; 97; 112; 45; 97; 104; 104; 110; 97; 45; 99; 97; 105; 4; 26; 102; 143; 124; 240; 5; 26; 102; 143; 110; 224; 6; 26; 102; 143; 110; 224; 9; 120; 24; 70; 111; 108; 100; 101; 114; 46; 42; 58; 49; 32; 66; 117; 99; 107; 101; 116; 46; 82; 101; 97; 100; 46; 42; 88; 64; 210; 38; 140; 40; 73; 180; 152; 145; 49; 12; 114; 27; 202; 202; 177; 163; 235; 140; 234; 54; 118; 79; 125; 78; 80; 204; 34; 220; 129; 8; 77; 2; 199; 210; 196; 189; 235; 130; 159; 138; 88; 162; 111; 191; 48; 61; 174; 99; 187; 110; 150; 149; 191; 43; 253; 25; 38; 53; 226; 80; 52; 158; 193; 7})'
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