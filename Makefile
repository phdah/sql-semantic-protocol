.PHONY: build fmt lint test doc check

build:
	cargo build

fmt:
	cargo fmt --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test

doc:
	cargo doc --no-deps

check: fmt lint test doc
