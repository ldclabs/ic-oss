# `ic_oss_bucket`

[ic-oss](https://github.com/ldclabs/ic-oss) is a decentralized Object Storage Service on the Internet Computer.

`ic_oss_bucket` is an ICP smart contract and a storage bucket in the `ic-oss` cluster for storing files and folders. The `ic-oss` cluster can deploy one or more buckets for horizontal file storage scaling, managed by `ic_oss_cluster`.

## Features

- [x] Supports large file uploads and downloads through file sharding, concurrent high-speed uploads, resumable uploads, and segmented downloads.
- [x] Provides data verification based on ICP's verification mechanisms to ensure file integrity during reading.
- [x] Supports file directory tree.
- [x] Access control with permissions for public, private, read-only, and write-only for files, folders, and buckets.

## Candid API

```shell
admin_set_auditors : (vec principal) -> (Result);
admin_set_managers : (vec principal) -> (Result);
admin_update_bucket : (UpdateBucketInput) -> (Result);
batch_delete_subfiles : (nat32, vec nat32, opt blob) -> (Result_1);
create_file : (CreateFileInput, opt blob) -> (Result_2);
create_folder : (CreateFolderInput, opt blob) -> (Result_2);
delete_file : (nat32, opt blob) -> (Result_3);
delete_folder : (nat32, opt blob) -> (Result_3);
get_bucket_info : (opt blob) -> (Result_4) query;
get_file_ancestors : (nat32, opt blob) -> (Result_5) query;
get_file_chunks : (nat32, nat32, opt nat32, opt blob) -> (Result_6) query;
get_file_info : (nat32, opt blob) -> (Result_7) query;
get_file_info_by_hash : (blob, opt blob) -> (Result_7) query;
get_folder_ancestors : (nat32, opt blob) -> (Result_5) query;
get_folder_info : (nat32, opt blob) -> (Result_8) query;
http_request : (HttpRequest) -> (HttpStreamingResponse) query;
list_files : (nat32, opt nat32, opt nat32, opt blob) -> (Result_9) query;
list_folders : (nat32, opt blob) -> (Result_10) query;
move_file : (MoveInput, opt blob) -> (Result_11);
move_folder : (MoveInput, opt blob) -> (Result_11);
update_file_chunk : (UpdateFileChunkInput, opt blob) -> (Result_12);
update_file_info : (UpdateFileInput, opt blob) -> (Result_11);
update_folder_info : (UpdateFolderInput, opt blob) -> (Result_11);
validate_admin_set_auditors : (vec principal) -> (Result) query;
validate_admin_set_managers : (vec principal) -> (Result) query;
validate_admin_update_bucket : (UpdateBucketInput) -> (Result) query;
```

The complete Candid API definition can be found in the [ic_oss_bucket.did](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_bucket/ic_oss_bucket.did) file.

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

# add managers
dfx canister call ic_oss_bucket admin_set_managers "(vec {principal \"$MYID\"; principal \"nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe\"})"

# add public keys to verify the access tokens
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

# list files in the folder 2, with a access token
dfx canister call ic_oss_bucket list_files '(2, null, null, opt blob "\84\44\a1\01\38\2e\a0\58\ac\a7\01\78\1b\61\6a\75\71\34\2d\72\75\61\61\61\2d\61\61\61\61\61\2d\71\61\61\67\61\2d\63\61\69\02\78\3f\7a\37\77\6a\70\2d\76\36\66\65\33\2d\6b\6b\73\75\35\2d\32\36\66\36\34\2d\64\65\64\74\77\2d\6a\37\6e\64\6a\2d\35\37\6f\6e\78\2d\71\67\61\36\63\2d\65\74\35\65\33\2d\6e\6a\78\35\33\2d\74\61\65\03\78\1b\6d\6d\72\78\75\2d\66\71\61\61\61\2d\61\61\61\61\70\2d\61\68\68\6e\61\2d\63\61\69\04\1a\66\8f\ce\68\05\1a\66\8f\c0\58\06\1a\66\8f\c0\58\09\78\18\46\6f\6c\64\65\72\2e\2a\3a\31\20\42\75\63\6b\65\74\2e\52\65\61\64\2e\2a\58\40\52\66\3e\e7\55\7e\99\2c\66\6d\65\56\54\9f\30\a1\2e\aa\56\69\66\b6\c6\e9\75\d7\c9\02\4c\24\1d\5d\7e\83\7d\c1\13\c6\00\91\56\d9\6a\ae\34\c3\a5\c9\b4\99\b3\47\b7\68\54\8d\dd\9c\9a\9b\a0\f9\1a\f5")'

# list folders in the root folder 0
dfx canister call ic_oss_bucket list_folders '(0, null)'

# upload a file to the bucket
ic-oss-cli -i debug/uploader.pem upload -b mmrxu-fqaaa-aaaap-ahhna-cai --file README.md

# read the file info
dfx canister call ic_oss_bucket get_file_info '(1, null)'

# update the file 1's status to 0
dfx canister call ic_oss_bucket update_file_info "(record {
  id = 1;
  status = opt 0;
}, null)"

# create a folder in the root folder
dfx canister call ic_oss_bucket create_folder "(record {
  parent = 0;
  name = \"home\";
}, null)"
dfx canister call ic_oss_bucket list_folders '(0, null)'

dfx canister call ic_oss_bucket create_folder "(record {
  parent = 1;
  name = \"jarvis\";
}, null)"

# move the file 1 from the root folder 0 to the folder 2
dfx canister call ic_oss_bucket move_file "(record {
  id = 1;
  from = 0;
  to = 2;
}, null)"

dfx canister call ic_oss_bucket list_files '(2, null, null, null)'

# delete the file 1
dfx canister call ic_oss_bucket delete_file '(1, null)'
```

Download the file in browser:
- by file id: `http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/f/1`
- by file hash:  `http://mmrxu-fqaaa-aaaap-ahhna-cai.localhost:4943/h/b7bb9040d64479a7ca56c8e03ae2daddc819859f7b858488c0b998eeded6dede`


## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for the full license text.