.PHONY: build fmt lint test check

build:
	cargo build

fmt:
	cargo fmt --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test

check: fmt lint test
