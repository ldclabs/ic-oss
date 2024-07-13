# Example: `ai_canister`

## Running the project locally

If you want to test your project locally, you can use the following commands:

```bash
cd examples/ai_canister
# Starts the replica, running in the background
dfx start --background

# deploy the canister
dfx deploy ai_canister
# canister: aovwi-4maaa-aaaaa-qaagq-cai

dfx canister call ai_canister state '()'

MYID=$(dfx identity get-principal)
ic-oss-cli -i debug/uploader.pem identity
# principal: nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe

dfx canister call ai_canister admin_set_managers "(vec {principal \"$MYID\"; principal \"nprym-ylvyz-ig3fr-lgcmn-zzzt4-tyuix-3v6bm-fsel7-6lq6x-zh2w7-zqe\"})"

dfx canister call ai_canister set_max_file_size "(10737418240)" # 10GB
dfx canister call ai_canister admin_set_visibility "(1)" # public

ic-oss-cli -i debug/uploader.pem upload -b aovwi-4maaa-aaaaa-qaagq-cai --file Qwen1.5-0.5B-Chat/config.json
# ... file id: 1 ...
ic-oss-cli -i debug/uploader.pem upload -b aovwi-4maaa-aaaaa-qaagq-cai --file Qwen1.5-0.5B-Chat/tokenizer.json
# ... file id: 2 ...
ic-oss-cli -i debug/uploader.pem upload -b aovwi-4maaa-aaaaa-qaagq-cai --file Qwen1.5-0.5B-Chat/model.safetensors
# ... file id: 3 ...

dfx canister call ai_canister admin_load_model '(record {config_id=1;tokenizer_id=2;model_id=3})'

dfx canister call ai_canister list_files '(0, null, null, null)'
```

## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for the full license text.