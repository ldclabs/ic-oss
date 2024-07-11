BUILD_ENV := rust

.PHONY: build-wasm build-did

lint:
	@cargo fmt
	@cargo clippy --all-targets --all-features

fix:
	@cargo clippy --fix --workspace --tests

test:
	@cargo test --workspace -- --nocapture

# cargo install ic-wasm
build-wasm:
	@cargo build --release --target wasm32-unknown-unknown --package ic_oss_bucket
	@cargo build --release --target wasm32-unknown-unknown --package ic_oss_cluster

# cargo install candid-extractor
build-did:
	candid-extractor target/wasm32-unknown-unknown/release/ic_oss_bucket.wasm > src/ic_oss_bucket/ic_oss_bucket.did
	candid-extractor target/wasm32-unknown-unknown/release/ic_oss_cluster.wasm > src/ic_oss_cluster/ic_oss_cluster.did
