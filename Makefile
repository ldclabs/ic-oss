BUILD_ENV := rust

.PHONY: build-wasm build-did

lint:
	@cargo fmt
	@cargo clippy --all-targets --all-features

fix:
	@cargo clippy --fix --workspace --tests

# cargo install ic-wasm
build-wasm:
	cargo build --release --target wasm32-unknown-unknown --package ic-oss-bucket

# cargo install candid-extractor
build-did:
	candid-extractor target/wasm32-unknown-unknown/release/ic_oss_bucket.wasm > src/ic-oss-bucket/ic-oss-bucket.did
