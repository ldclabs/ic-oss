.PHONY: lint fix test build-wasm build-did

lint:
	@cargo fmt
	@cargo clippy --all-targets --all-features

fix:
	@cargo clippy --fix --workspace --tests

test:
	@cargo test --workspace -- --nocapture

build-wasm:
	cargo build --release --target wasm32-unknown-unknown -p ic_object_store_canister -p ic_oss_bucket -p ic_oss_cluster

# cargo install candid-extractor
build-did: build-wasm
	@set -e; for canister in ic_object_store_canister ic_oss_bucket ic_oss_cluster; do \
	  did=$$(candid-extractor "target/wasm32-unknown-unknown/release/$$canister.wasm"); \
	  printf '%s\n' "$$did" > "src/$$canister/$$canister.did"; \
	done
	npm run generate-candid --workspace=@ldclabs/ic_oss_ts
