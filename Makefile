.PHONY: fmt fmt-check lint test clean

export RUSTC_WRAPPER = sccache

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

lint:
	cargo clippy -- -D warnings

test:
	cargo test

ci: fmt-check lint test

clean:
	cargo clean
