# Contributing

The repository uses Rust 1.97.1 for host-side checks and SP1 6.3.1's pinned
Docker image for production guest builds. `rust-toolchain.toml` installs
`rustfmt` and Clippy automatically through rustup.

Before opening a change, run:

```bash
cargo fmt --all -- --check
cargo fmt --manifest-path network-prover/Cargo.toml --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p randomx-sp1-executor --all-targets \
  --features profiling --locked -- -D warnings
cargo clippy -p randomx-softfp --all-targets \
  --features sp1-guest --locked -- -D warnings
cargo clippy -p randomx-sp1-program --all-targets \
  --features profile-probes --locked -- -D warnings
cargo clippy -p randomx-sp1 --all-targets \
  --features differential-audit --locked -- -D warnings
CARGO_TARGET_DIR=target cargo clippy \
  --manifest-path network-prover/Cargo.toml --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p randomx-sp1 --no-deps --locked
cargo test --workspace --release --locked -- --test-threads=1
CARGO_TARGET_DIR=target cargo test \
  --manifest-path network-prover/Cargo.toml --release --locked
cargo audit -D unsound
cargo audit --file network-prover/Cargo.lock \
  --ignore RUSTSEC-2026-0002 -D unsound
```

The serial test setting prevents multiple 256 MiB RandomX cache tests from
running concurrently. Changes to manifests, source formatting, package names,
or guest code can change the SP1 ELF and vkey; make them before the final
reproducible build.

The deep reference/optimized lockstep audit is deliberately ignored by the
default suite. Run it when changing interpreter behavior:

```bash
cargo test --release --locked -p randomx-sp1-audit \
  --test differential -- --ignored
```

CI never submits a proof or reads a prover key. Network proof requests require
separate, explicit approval and follow `RELEASING.md`.
