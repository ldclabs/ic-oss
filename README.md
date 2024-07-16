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

| Library                                                                                  | Description                                                                                                                  |
| :--------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------- |
| [ic_oss_bucket](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_bucket)           | An ICP smart contract and a storage bucket in the ic-oss cluster for storing files and folders.                              |
| [ic_oss_cluster](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_cluster)         | An ICP smart contract and the manager of the ic-oss cluster.                                                                 |
| [ic-oss-can](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_can)                 | A Rust library for implementing large file storage in ICP canisters.                                                         |
| [ic-oss-types](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_types)             | A Rust types library used for integrating with ic-oss cluster.                                                               |
| [ic-oss-cose](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_cose)               | A Rust library based on COSE (RFC9052) and CWT (RFC8392) for issuing and verifying access tokens for the ic-oss cluster.     |
| [ic-oss](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss)                         | The Rust version of the client SDK for the ic-oss cluster.                                                                   |
| [ic_oss_ts](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_ts)                   | The Typescript version of the client SDK for the ic-oss cluster.                                                             |
| [ic-oss-cli](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_cli)                 | A command-line tool implemented in Rust for the ic-oss cluster.                                                              |
| [examples/ai_canister](https://github.com/ldclabs/ic-oss/tree/main/examples/ai_canister) | A Rust demonstration project used to show how to implement large file storage in the ICP canister by using `ic-oss-can`.     |
| [examples/upload_js](https://github.com/ldclabs/ic-oss/tree/main/examples/upload_js)     | A Javascript demonstration project used to show how to upload files to ic_oss_bucket canister by using `@ldclabs/ic_oss_ts`. |

## Integration Workflow

![IC-OSS Sequence](./ic-oss-sequence.webp)

How to integrate `ic-oss`:
1. The backend of the Dapp calls the API of `ic_oss_cluster` to add access control policies for the user.
2. The frontend of the Dapp uses the `ic-oss-ts` SDK to obtain the `access_token` of the target `ic_oss_bucket` from `ic_oss_cluster`.
3. The frontend of the Dapp uses the `access_token` to call the API of the target `ic_oss_bucket` to operate the authorized files and folders.

## Access Control

`ic-oss` provides 4 different access control mechanisms for file resources, which can adapt to scenarios ranging from minimalist ones to complex enterprise-level permission control scenarios.

### The `managers` and `auditors` attributes of the Bucket

Managers can operate on all files and folders in the bucket, including creating, deleting, moving, modifying, etc.

Auditors can view all files and folders in the bucket, including archived (status == -1) resources, but cannot perform modification operations.

The `admin_set_managers` interface and the `admin_set_auditors` interface can set the managers and auditors of the bucket, and the `admin_update_bucket` can update other attributes of the bucket. However, only the controllers of the canister have the permission to call these 3 interfaces.

### The `visibility` attribute of the Bucket

`visibility` controls the visibility of the bucket and has 2 values:
- 0: Private, only users with access permission to the bucket can access it.
- 1: Public, any user without permission can view all files and folders in the bucket, but does not include archived (status == -1) resources, and cannot perform modification operations.

The `admin_update_bucket` can update the `visibility` attribute of the bucket.

### The `status` attribute of the Bucket

`status` controls the status of the bucket and has 3 values:
- 0: Normal, all operations can be performed.
- 1: Read-only, only read operations can be performed, and write operations cannot be performed.
- -1: Archived. Only `managers` and `auditors` can view all files and folders in the bucket, and no other operations can be performed.
Files and folders also have a `status` attribute, and its definition is similar to the above.

### Access Control based on `access_token` and Permissions Policy

Based on `access_token` and permissions policy, more complex and fine-grained access control can be achieved for files and folders in the bucket.

`ic_oss_cluster` records the user's permissions policies and issues `access_token` for the user. The `access_token` contains the user's permission information. `ic_oss_bucket` verifies the `access_token` and determines whether the user has the permission to perform the operation based on the permission information in it.

The managers of `ic_oss_cluster` can use the `admin_attach_policies` and `admin_detach_policies` interfaces to assign or cancel permissions for the user.

#### Access Token

The `access_token` implemented by `ic-oss` based on COSE (RFC9052) and CWT (RFC8392) supports two signature algorithms: Secp256k1 and Ed25519. The permissions policies are stored in the `scope (9)` field of the `access_token`. The core information of the Token is as follows:

```rust
pub struct Token {
    pub subject: Principal,  // the owner of the token
    pub audience: Principal, // the canister id of the bucket
    pub policies: String,    // the permission policies
}
```
For the complete implementation, please refer to the [ic-oss-cose](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_cose) library.

#### Permissions Policy

`ic-oss` has designed a set of simple yet powerful permission policy patterns, which can achieve from simple read and write permissions to complex enterprise-level permission control.

The basic expression of Permission is as follows:

```shell
Resource.Operation[.Constraint]
```

Permission examples:
```shell
*                 # == *.*
File.Read         # == File.Read.*
Folder.Write      # == Folder.Write.*
Bucket.Read       # == Bucket.Read.*
Bucket.Read.Info
Bucket.*.File
```

The basic expression of Permission Policy is as follows:

```shell
Permission:Resource1,Resource2,...
```

Permission Policy examples:
```shell
File.*:*         # == File.*
File.Read:*      # == File.Read
Folder.Write:1,2
Bucket.Read:*    # == Bucket.Read
```

The scope of `access_token` contains 1 to n Permission Policies, separated by spaces.

Permission Policies examples:
```shell
scope = "File.*:1 Folder.*:2,3,5 Folder.Read Bucket.Read"
```

For the complete implementation, please refer to the [ic-oss-types](https://github.com/ldclabs/ic-oss/tree/main/src/ic_oss_types) library.

## License
Copyright © 2024 [LDC Labs](https://github.com/ldclabs).

`ldclabs/ic-oss` is licensed under the MIT License. See [LICENSE](LICENSE-MIT) for the full license text.