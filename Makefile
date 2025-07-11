MAKEFLAGS += --no-silent

.PHONY: lint autofix

fix:
	rustup run nightly cargo fmt --quiet

test: fix
	cargo clippy
	cargo test
	rustup run nightly cargo fmt --check

