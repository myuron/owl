fmt:
  cargo fmt

fmt-check:
  cargo fmt --check

lint:
  cargo clippy -- -D warnings

test:
  cargo test

build:
  nix build

ci: fmt-check lint test build