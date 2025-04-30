# `IC Object Store`

Native Rust implementation of Apache Arrow object store on the Internet Computer.

More detail: https://github.com/apache/arrow-rs-object-store

## Features

- Full implementation of Apache Arrow object store APIs.
- AES256-GCM encryption.

## Demo

Try it online: https://a4gq6-oaaaa-aaaab-qaa4q-cai.raw.icp0.io/?id=6at64-oyaaa-aaaap-anvza-cai

## API Reference

The canister exposes a comprehensive Candid API. Key endpoints include:

```candid
# Object Operations
put_opts : (text, blob, PutOptions) -> (Result)
head : (text) -> (Result) query
get_opts : (text, GetOptions) -> (Result) query
get_ranges : (text, vec record { nat64; nat64 }) -> (Result) query
copy : (text, text) -> (Result)
rename : (text, text) -> (Result)
list : (opt text) -> (Result) query
list_with_delimiter : (opt text) -> (Result) query
list_with_offset : (opt text, text) -> (Result) query
create_multipart : (text) -> (Result)
put_part : (text, text, nat64, blob) -> (Result)
complete_multipart : (text, text, PutMultipartOpts) -> (Result)

# Admin Operations
admin_add_managers : (vec principal) -> (Result)
admin_remove_managers : (vec principal) -> (Result)
```

Full Candid API definition: [ic_object_store_canister.did](https://github.com/ldclabs/ic-oss/tree/main/src/ic_object_store_canister/ic_object_store_canister.did)

## License
Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for the full license text.
