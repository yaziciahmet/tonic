default:
    @just --list

# Cargo build everything.
build:
    cargo build --workspace --all-targets --all-features

# Cargo check everything.
check:
    cargo check --workspace --all-targets --all-features

# Lint everything.
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features

# Format everything.
fmt:
    cargo fmt --all

# Test everything
test:
    cargo test --workspace --all-targets --all-features

# Find unused dependencies
udeps:
    cargo +nightly udeps

# Deterministic simulation tests. TODO: carry this to script or smh.
sim_test:
    MADSIM_TEST_NUM=200 MADSIM_TEST_JOBS=8 RUSTFLAGS="--cfg madsim" cargo test --package tonic-consensus-poa --lib --features test-helpers --release -- sim_tests::ibft_run --exact --show-output --nocapture
