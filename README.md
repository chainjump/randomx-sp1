# Optimized RandomX for SP1

This repository is the source of truth for the optimized SP1 RandomX
implementation. The guest accepts both the RandomX key and hashing blob at
runtime, constructs the complete 256 MiB Argon2d cache, derives each requested
dataset item, executes all eight RandomX programs, and commits the hash.

The implementation is universal with respect to RandomX inputs: it does not
embed an epoch key or a hashing blob. Arbitrary key lengths and empty blobs are
supported and covered by the differential corpus.

No generated ELF, vkey, or proof is retained for the current source tree.
Release-facing cleanup intentionally invalidated the previous build, and the
prover client fails closed until a final reproducible build is reviewed and
approved. No proof submission is authorized at this stage.

## Current optimization

The hot dataset path retains 16-byte decoded superscalar instructions with
precomputed 64-bit immediates. Register operands are stored as byte offsets,
and adjacent opcodes are predecoded to one of 100 static pair handlers. The
interpreter processes 16 pairs per outer iteration. All program generation
still happens from the runtime key inside the guest; no generated RandomX
program is compiled into the ELF.

A fixed-epoch code specialization was rejected because it could not support
one stable program identity across arbitrary RandomX keys. All retained
optimizations generate their state from the runtime key.

## Layout

- `randomx-sp1/`: supported library API and optimized RandomX VM executor.
- `randomx-core/`: internal cache, dataset, program-generation, and VM state.
- `program/`: the single universal SP1 guest.
- `softfp/`: exact four-mode binary64 arithmetic for SP1 RV64IM.
- `argon2/`: an in-tree `rust-argon2` fork retaining the generic API and tests,
  with optimized RandomX Argon2d cache construction for the SP1 guest.
- `executor/`: lightweight execution and calibrated PGU estimation.
- `network-prover/`: fixed-block Succinct Network request, recovery, local
  proof verification, and EVM `eth_call` verification client.
- `audit/`: official-RandomX and reference/optimized differential checks.
- `randomx-sp1-argon2-audit/`: complete-cache differential checks.
- `profile-probes/`, `softfp-guest/`, and `softfp-runner/`: profiling and
  arithmetic validation tools.

Consumers should depend on `randomx-sp1` and use its stable entry point:

```rust
let digest: [u8; 32] = randomx_sp1::hash(&randomx_key, &hashing_blob);
```

Use an immutable release commit rather than a branch:

```toml
[dependencies]
randomx-sp1 = { git = "https://git.example/repository.git", rev = "<full-commit-hash>" }
```

The consuming application must commit its own `Cargo.lock` and use `--locked`
for reproducible builds. A parent SP1 program links this library into a new
ELF and must prove that combined ELF; the standalone program's vkey or proof is
not reusable as a subproof.

The default crate exposes one supported API: `randomx_sp1::hash`. Features
whose names contain `audit` exist only for this repository's validation tools
and carry no compatibility guarantee. The internal crates are not separately
supported APIs. Their upstream lineage and retained licenses are recorded in
`ATTRIBUTION.md`.

## Reproduce the ELF

The repository pins SP1 6.3.1 in `Cargo.lock`. From `program/` run:

```bash
cargo prove build --docker --tag v6.3.1 --locked \
  --elf-name randomx-sp1-program \
  --output-directory ../artifacts
```

`--docker` performs the guest build in SP1's build container. `--locked`
requires Cargo to use the exact dependency versions in `Cargo.lock` and fail
instead of changing the lockfile. The explicit `v6.3.1` image tag is also the
SP1 6.3.1 CLI default, but spelling it out makes the build recipe visible.

The output is an SP1 guest ELF for the `riscv64im-succinct-zkvm-elf` target,
not a native Linux, macOS, or Windows executable. It can be loaded by a
compatible SP1 executor or prover on any host platform supported by that SP1
release. A proof and vkey are tied to the exact built ELF and compatible SP1
circuit.

Build the executor from the repository root:

```bash
cargo build --release --locked -p randomx-sp1-executor
```

Reproduce block 3,727,315. After the ELF path and expected public hash, the
first input is the runtime RandomX key and the second is the hashing blob:

```bash
target/release/randomx-sp1-executor \
  artifacts/randomx-sp1-program \
  50966d5e6f491b5c2dccefb149c314d996fe36e73e4a18ae4d9f0d0100000000 \
  11c798e5ac6515218bc3efcb5416e5b68c599e42a61b86efe5746bb78eb4be8e \
  101093ba9fd306c6afa883a69ae61498cc8edc5b04ad42664ca1d70cb4f6e14f609d65af22e8a6a8910b0f68387e426a3e54920779782c55fab88b05da6b989d97e67ced90f93a2237a17601
```

Add `--estimate-gas` before the ELF path for the calibrated estimator. It uses
canonical gas boundaries and one shared-memory trace slot, so memory remains
bounded without changing the PGU result.

## Correctness

The current implementation is checked against:

- all 84 portable checks from the canonical RandomX v1.2.3 `randomx-tests`
  program (the 11 JIT-, SIMD-, and alternate-implementation checks are not
  applicable and are itemized in `evidence/canonical-v1.2.3-test-port.md`);
- 20 consecutive real Monero blocks;
- 42 official RandomX v1.2.3 light-mode hashes across seven key shapes and six
  blob shapes;
- reference/optimized lockstep state comparisons;
- complete 256 MiB cache digests for multiple keys; and
- software-floating-point comparisons against Berkeley SoftFloat.

Current source evidence is under `evidence/`. The SP1-specific unsafe-code,
syscall, dependency, and provenance review is recorded in
`evidence/sp1-program-safety-review.md`.

## Continuous integration

`.github/workflows/ci.yml` runs on every pull request, every push to `main`,
every `v*` tag, and manual dispatch. Its independent jobs check formatting,
Clippy, public documentation, the complete serial release suite, the separate
prover-client tests, both lockfiles, and consumption through a pinned Git
revision. CI never reads a prover key, builds a release ELF, executes the
multi-billion-cycle guest, contacts the prover network, or simulates an EVM
transaction.

On a non-GitHub host, configure its CI runner to execute the same commands in
`CONTRIBUTING.md`. The release-only build and proof sequence is documented in
`RELEASING.md`.

## Licensing and security

The public library and derived RandomX core are GPL-3.0-only. Independently
licensed support components retain their MIT and Apache-2.0 terms. See
`LICENSE`, `LICENSE-MIT`, `LICENSE-APACHE`, and `ATTRIBUTION.md`. Report
security issues privately according to `SECURITY.md`.

No Git remote is configured yet, so preserving or independently backing up the
repository's `.git` directory remains necessary for recovery.
