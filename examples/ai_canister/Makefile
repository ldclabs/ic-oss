.PHONY: lint fix test build-wasm build-did

lint:
	@cargo fmt
	@cargo clippy --all-targets --all-features

fix:
	@cargo clippy --fix --workspace --tests

test:
	@cargo test --workspace -- --nocapture

build-wasm:
	@cargo build --release --target wasm32-unknown-unknown --package ai_canister

# cargo install candid-extractor
build-did: build-wasm
	did=$$(candid-extractor ../../target/wasm32-unknown-unknown/release/ai_canister.wasm) && printf '%s\n' "$$did" > ai_canister.did
