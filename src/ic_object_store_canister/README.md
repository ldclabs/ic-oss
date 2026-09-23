# `IC Object Store`

Native Rust implementation of Apache Arrow object store on the Internet Computer.

More detail: https://github.com/apache/arrow-rs-object-store

## Features

- Full implementation of Apache Arrow object store APIs.
- AES256-GCM encryption.

## Demo

Try it online: https://a4gq6-oaaaa-aaaab-qaa4q-cai.raw.icp0.io/?id=6at64-oyaaa-aaaap-anvza-cai

## Quick Start

### Local Deployment

1. Deploy the canister:
```bash
# dfx canister create --specified-id 6at64-oyaaa-aaaap-anvza-cai ic_object_store_canister
dfx deploy ic_object_store_canister
```

Or with custom configuration:
```bash
dfx deploy ic_object_store_canister --argument "(opt variant {Init =
  record {
    name = \"LDC Labs\";
    governance_canister = null;
  }
})"
```

2. Set up permissions:
```bash
# Get your principal
MYID=$(dfx identity get-principal)
# Get the uploader principal
ic-oss-cli -i debug/uploader.pem identity
# principal: jjn6g-sh75l-r3cxb-wxrkl-frqld-6p6qq-d4ato-wske5-op7s5-n566f-bqe

# Add managers
dfx canister call ic_object_store_canister admin_add_managers "(vec {principal \"$MYID\"; principal \"jjn6g-sh75l-r3cxb-wxrkl-frqld-6p6qq-d4ato-wske5-op7s5-n566f-bqe\"})"

dfx canister call ic_object_store_canister get_state '()'
```

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
complete_multipart : (text, text, PutMultipartOptions) -> (Result)

# Admin Operations
admin_add_managers : (vec principal) -> (Result)
admin_remove_managers : (vec principal) -> (Result)
```

Full Candid API definition: [ic_object_store_canister.did](https://github.com/ldclabs/ic-oss/tree/main/src/ic_object_store_canister/ic_object_store_canister.did)

## Uploads, paths, and upgrades

Multipart uploads remain separate from the visible object until completion.
Starting or aborting an upload leaves an existing object at the same path intact.
Use `abort_multipart` to reclaim an unfinished upload; deleting the visible object
is a separate operation. Uploads started by older canister versions can still be
completed or aborted after upgrading.

Object paths are normalized by removing a leading and trailing slash. Upgrades
normalize existing keys as well. If two historical keys normalize to the same
path, the upgrade fails without overwriting either object; resolve the duplicate
paths before retrying the upgrade.

The low-level list endpoints return pages of up to 1,000 entries. Use
`list_with_offset` to continue a flat listing. The Rust `ObjectStoreClient`
automatically traverses all pages, including when constructing delimiter listings.

## License
Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for the full license text.
