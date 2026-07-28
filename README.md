# Generic RandomX SP1 optimization

This is the isolated development repository for reducing the SP1 cycle count
of a complete, generic RandomX light-mode hash. It keeps the claim intact:
the guest derives the full 256 MiB Argon2d cache from the runtime-relevant
RandomX key, derives every requested dataset item, executes all eight RandomX
programs and every opcode, and implements all four `CFROUND` modes.

No SP1 proof has been generated. Executor cycle counts are not proof cost or
prover gas units.

## Layout

- `program/`: real-block regression fixture.
- `program-cfround/`: CFROUND-heavy four-mode regression fixture.
- `compact/`: compact RandomX VM decoder and executor.
- `softfp/`: four-mode binary64 arithmetic used by the compact VM.
- `rustdom-x/`: optimized pure-Rust RandomX implementation.
- `argon2/`: RandomX-specialized Argon2 implementation.
- `audit/`: rich/compact VM differential harness.
- `argon2-native-compare/`: complete-cache differential harness.
- `profile-probes/`: cache-only and no-memory SP1 phase probes.
- `softfp-guest/` and `softfp-runner/`: SP1 software-floating-point checks.
- `executor/`: lightweight SP1 ELF executor and output checker.
- `docs/` and `evidence/`: imported handoff and frozen baseline notes.

The legacy tree under `/root/experiment` remains untouched because it contains
historical artifacts, source archives, controls, and rejected-candidate
evidence. The exact imported source is preserved by the repository's first
commit, `4f1f4a76a2329c677ef3e4743146c6b4d23796a3`.

## Bounded commands

Every build, test, audit, and execution must remain below 60 seconds. Examples:

```bash
timeout --signal=INT --kill-after=1s 55s cargo check --workspace --locked
timeout --signal=INT --kill-after=1s 55s cargo test --locked -p randomx-softfp
timeout --signal=INT --kill-after=1s 55s cargo run --release --locked -p randomx-compact-vm-audit
```

In this managed workspace, a fresh host-runner build needs the already-built
SP1 helper because the Cargo registry is read-only:

```bash
SP1_RUNNER=/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/\
sp1-core-executor-runner-6.3.1/target/sp1-native-bins/debug/\
sp1-core-executor-runner-binary

timeout --signal=INT --kill-after=1s 55s \
  env SP1_CORE_RUNNER_OVERRIDE_BINARY="$SP1_RUNNER" \
  cargo check --workspace --locked --offline
```

This override is specific to the managed filesystem; ordinary writable Cargo
installations can let SP1 build its helper normally.

Build an SP1 fixture from its package directory and execute it with the local
lightweight runner:

```bash
timeout --signal=INT --kill-after=1s 55s \
  cargo prove build --locked --elf-name randomx-cfround \
  --output-directory ../artifacts

timeout --signal=INT --kill-after=1s 55s \
  cargo run --release --locked -p randomx-executor -- \
  artifacts/randomx-cfround \
  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

The nearest-only feature is a negative audit control and must never be enabled
in a verifier.

## Independent official audit

`audit/src/bin/official_randomx.rs` can compare the rich and compact VMs
directly with a locally built official RandomX v1.2.3 library. It is opt-in so
the normal workspace has no external native-library requirement:

```bash
timeout --signal=INT --kill-after=1s 55s \
  env RANDOMX_LIB_DIR=/path/to/RandomX/build \
  cargo run --release --locked --offline \
  -p randomx-compact-vm-audit --features official-randomx \
  --bin official_randomx -- pattern-257
```

Run one key name per command to remain under the time limit. The audited key
names and the v1.2.3 reference fingerprint are recorded in
`evidence/official-v1.2.3-corpus.md`.
