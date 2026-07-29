# Contributing

The repository uses Rust 1.97.1 for host-side checks and SP1 6.3.1's pinned
Docker image for production guest builds. `rust-toolchain.toml` installs
`rustfmt` and Clippy automatically through rustup.

Before opening a change, run:

```bash
cargo fmt --all -- --check
cargo fmt --manifest-path network-prover/Cargo.toml --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p randomx-sp1 --all-targets \
  --features differential-audit --locked -- -D warnings
CARGO_TARGET_DIR=target cargo clippy \
  --manifest-path network-prover/Cargo.toml --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p randomx-sp1 --no-deps --locked
cargo test --workspace --release --locked -- --test-threads=1
CARGO_TARGET_DIR=target cargo test \
  --manifest-path network-prover/Cargo.toml --release --locked
```

The serial test setting prevents multiple 256 MiB RandomX cache tests from
running concurrently. Changes to manifests, source formatting, package names,
or guest code can change the SP1 ELF and vkey; make them before the final
reproducible build.

CI never submits a proof or reads a prover key. Network proof requests require
separate, explicit approval and follow `RELEASING.md`.
