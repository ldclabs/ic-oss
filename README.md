# `ic-oss`

🗂 A decentralized Object Storage Service on the Internet Computer.

💝 This project received a **$25k Developer Grant** from the [DFINITY Foundation](https://dfinity.org/grants).

## Overview

`ic-oss` is a fully open-source decentralized object storage service running on the Internet Computer. It provides a simple and efficient way to store and retrieve files, supports large files, and offers unlimited horizontal scalability. It can serve as a reliable decentralized file infrastructure for NFT, chain blocks, verifiable credentials, blogs, documents, knowledge bases, games and other decentralized applications.

In decentralized enterprise applications, `ic-oss` will be an essential infrastructure.

`ic-oss` is a file infrastructure service, not a user-facing product, but it will provide a simple management interface.

> [!NOTE]
> The main functions of `ic-oss` have been developed, and the cluster management function is still under development (which will be completed soon). It can be used in the production environment.

![IC-OSS](./ic-oss.webp)

## Features

- [x] Supports large file uploads and downloads through file sharding, concurrent high-speed uploads, resumable uploads, and segmented downloads.
- [x] Provides data verification based on ICP's verification mechanisms to ensure file integrity during reading.
- [x] Supports file directory tree.
- [x] Access control with permissions for public, private, read-only, and write-only for files, folders, and buckets.
- [ ] Based on a file bucket and cluster architecture, with each bucket corresponding to a ICP canister, allowing for unlimited horizontal scalability.
- [ ] Compatible with S3 core API protocol and supports S3 SDK.
- [ ] Implements file encryption storage using ICP's vetKeys mechanism.
- [ ] Integrates with external storage, supporting file storage in decentralized file services like IPFS and Arweave, with `ic-oss` managing file metadata.

## Libraries

| Library                                                                                  | Description                                                                                                              |
| :--------------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------- |
| [ic_oss_bucket](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_bucket)           | An ICP smart contract and a storage bucket in the ic-oss cluster for storing files and folders.                          |
| [ic_oss_cluster](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_cluster)         | An ICP smart contract and the manager of the ic-oss cluster.                                                             |
| [ic-oss-can](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_can)                 | A Rust library for implementing large file storage in ICP canisters.                                                     |
| [ic-oss-types](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_types)             | A Rust types library used for integrating with ic-oss cluster.                                                           |
| [ic-oss-cose](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_cose)               | A Rust library based on COSE (RFC9052) and CWT (RFC8392) for issuing and verifying access tokens for the ic-oss cluster. |
| [ic-oss](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss)                         | The Rust version of the client SDK for the ic-oss cluster.                                                               |
| [ic-oss-cli](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_cli)                 | A command-line tool implemented in Rust for the ic-oss cluster.                                                          |
| [examples/ai_canister](https://github.com/ldclabs/ic-oss/tree/main/examples/ai_canister) | A demonstration project used to show how to implement large file storage in the ICP canister by using `ic-oss-can`.      |

## Integration Workflow

![IC-OSS Sequence](./ic-oss-sequence.webp)

How to integrate `ic-oss`:
1. The backend of the Dapp calls the API of `ic_oss_cluster` to add access control policies for the user.
2. The frontend of the Dapp uses the `ic-oss-ts` SDK to obtain the `access_token` of the target `ic_oss_bucket` from `ic_oss_cluster`.
3. The frontend of the Dapp uses the `access_token` to call the API of the target `ic_oss_bucket` to operate the authorized files and folders.

## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](LICENSE-MIT) for the full license text.