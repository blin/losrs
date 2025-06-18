MAKEFLAGS += --no-silent

.PHONY: lint autofix

lint:
	cargo clippy
	cargo test
	rustup run nightly cargo fmt --check

fix:
	rustup run nightly cargo fmt --quiet
