fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

check: fmt-check lint

.PHONY:fmt fmt-check lint check

test-wasm:
	cd contracts/escrow && stellar contract build
	cargo clippy -p escrow --features wasm-tests --all-targets -- -D warnings
	cargo test -p escrow --features wasm-tests

.PHONY:test-wasm
