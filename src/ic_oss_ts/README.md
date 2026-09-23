# `@ldclabs/ic_oss_ts`
![License](https://img.shields.io/crates/l/ic-oss.svg)
[![Test](https://github.com/ldclabs/ic-oss/actions/workflows/test.yml/badge.svg)](https://github.com/ldclabs/ic-oss/actions/workflows/test.yml)
[![NPM version](http://img.shields.io/npm/v/@ldclabs/ic_oss_ts.svg)](https://www.npmjs.com/package/@ldclabs/ic_oss_ts)

[ic-oss](https://github.com/ldclabs/ic-oss) is a decentralized Object Storage Service on the Internet Computer.

`@ldclabs/ic_oss_ts` is the Typescript version of the client SDK for the ic-oss cluster.

## Development

Use Node.js 20.19 or newer and the repository's npm workspace lockfile:

```sh
npm ci --ignore-scripts
npm run typecheck --workspace=@ldclabs/ic_oss_ts
npm test --workspace=@ldclabs/ic_oss_ts
```

`Uploader` accepts native and polyfilled readable streams. On a failed chunk
upload it waits for calls already in flight before returning the resume state.
Pass that state's `uploadedChunks` to `upload_chunks` when resuming from the
beginning of the original file; the final hash covers the entire file, including
chunks that did not need to be sent again.

## License

Copyright © 2024-2025 [LDC Labs](https://github.com/ldclabs).

Licensed under the MIT License. See [LICENSE](../../LICENSE-MIT) for details.
