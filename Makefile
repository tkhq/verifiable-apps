include macros.mk

REGISTRY := local
.DEFAULT_GOAL :=
.PHONY: default
default: \
	out/reshard_host/index.json \
	out/reshard_app/index.json

out/common/index.json: \
	images/common/Containerfile
	$(call build,common)

.PHONY: shell
shell: out/.common-loaded
	docker run \
		--interactive \
		--tty \
		--volume .:/home/qos \
		--workdir /home/qos \
		--user $(shell id -u):$(shell id -g) \
		tkhq/verifiable-apps/common:latest \
		/bin/bash

out/reshard_host/index.json: \
	out/common/index.json \
	images/reshard_host/Containerfile \
	$(shell git ls-files \
		Cargo.toml \
		Cargo.lock \
		apps/reshard/host)
	$(call build,reshard_host)

out/reshard_app/index.json: \
	out/common/index.json \
	images/reshard_app/Containerfile \
	$(shell git ls-files \
		Cargo.toml \
		Cargo.lock \
		apps/reshard/app)
	$(call build,reshard_app)

.PHONY: codegen
codegen: 
	cargo run --manifest-path codegen/Cargo.toml

.PHONY: reshard_test
reshard_test: build_reshard
	cargo test --test reshard reshard_e2e_json -- --exact --nocapture

.PHONY: build_reshard
build_reshard:
	cargo build --features mock --bin reshard_app --bin reshard_host
