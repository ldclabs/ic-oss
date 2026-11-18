# JavaScript upload example

This example uses `@ldclabs/ic_oss_ts` to upload local files to an IC-OSS bucket.
It requires Node.js 20.19 or newer and a local dfx network.

From the repository root, start the local network and deploy the bucket.

```sh
dfx start --background
dfx deploy ic_oss_bucket
BUCKET_ID=$(dfx canister id ic_oss_bucket)
```

Install the example's dependencies and create a local upload identity. Add its
principal as a manager of the bucket, then upload one or more files.

```sh
cd examples/upload_js
npm install
UPLOADER=$(node upload.js --new-identity /tmp/ic-oss-uploader.json)
cd ../..
dfx canister call ic_oss_bucket admin_add_managers "(vec { principal \"$UPLOADER\" })"
cd examples/upload_js
npm run upload -- "$BUCKET_ID" /tmp/ic-oss-uploader.json README.md package.json
```

The identity command creates a new file with owner-only permissions on Unix.
Keep this file outside the repository and reuse it for subsequent uploads.
To use an existing mainnet bucket that already authorizes the identity, set
`IC_HOST=https://icp-api.io` when running the upload command.

## License

Licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for details.
