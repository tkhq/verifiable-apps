.PHONY: codegen
codegen: 
	cargo run --manifest-path codegen/Cargo.toml

.PHONY: reshard_test
reshard_test: build_reshard
	cargo test --test reshard reshard_e2e_json -- --exact --nocapture

.PHONY: build_reshard
build_reshard:
	cargo build --features mock --bin reshard_app --bin reshard_host
