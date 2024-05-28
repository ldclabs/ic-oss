# `ic-oss-bucket`

[![CI](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/ldclabs/ic-oss/actions/workflows/ci.yml)

A bucket canister of [ic-oss](https://github.com/ldclabs/ic-oss).
`ic-oss` is a fully open-source decentralized object storage service running on the Internet Computer.

## Running locally

Deploy to local network:
```bash
dfx deploy ic-oss-bucket
# Output:
# ...
# Installing code for canister ic-oss-bucket, with canister ID mmrxu-fqaaa-aaaap-ahhna-cai
# Deployed canisters.
# URLs:
#   Backend canister via Candid interface:
#     ic-oss-bucket: http://127.0.0.1:4943/?canisterId=bd3sg-teaaa-aaaaa-qaaba-cai&id=mmrxu-fqaaa-aaaap-ahhna-cai
```

## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](LICENSE) for the full license text.