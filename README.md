# Optimized RandomX for SP1

This repository is the source of truth for the optimized SP1 RandomX
implementation. The guest accepts both the RandomX key and hashing blob at
runtime, constructs the complete 256 MiB Argon2d cache, derives each requested
dataset item, executes all eight RandomX programs, and commits the hash.

The implementation is universal with respect to RandomX inputs: it does not
embed an epoch key or a hashing blob. Canonical RandomX specifies keys of
0–60 bytes and permits empty blobs. Longer keys are accepted, but their
behavior is implementation-defined; see the review below for the known
difference at key lengths of 2^32 bytes and above.

Current source targets SP1 6.10.0. The rebuilt standalone candidate is retained
under `artifacts/`, with local validation recorded alongside it. No
network proof or live RPC test has been performed for this candidate. Older
release artifacts have been removed.

## Correctness

**As part of the test suite, the complete RandomX hash ran and matched the
expected results for 20 consecutive real Monero blocks.**

The implementation is also checked against:

- all 84 portable checks from the canonical RandomX v1.2.3 `randomx-tests`
  program (the 11 JIT-, SIMD-, and alternate-implementation checks are not
  applicable and are itemized in `evidence/canonical-v1.2.3-test-port.md`);
- 42 official RandomX v1.2.3 light-mode hashes across seven key shapes and six
  blob shapes;
- reference/optimized lockstep state comparisons;
- complete 256 MiB cache digests for multiple keys; and
- software-floating-point comparisons against Berkeley SoftFloat.

These tests and differential checks are the general correctness evidence.
A proof authenticates one execution; it does not establish correctness for
every possible input.

The exhaustive 32-hash reference/optimized lockstep audit is intentionally
ignored by the default suite. Run it explicitly when changing the interpreter:

```bash
cargo test --release --locked -p randomx-sp1-audit \
  --test differential -- --ignored
```

The detailed corpus is under `evidence/`. The SP1-specific unsafe-code,
syscall, dependency, and provenance review is recorded in
`evidence/sp1-program-safety-review.md`.

## Quickstart for dependent SP1 programs

Use `randomx-sp1` as a Rust source dependency inside the dependent program's
own SP1 guest. The crate is not published to crates.io. Replace
`<reviewed-commit>` below with the full commit returned by `git rev-parse HEAD`
in a reviewed checkout of this SP1 6.10.0 source:

```toml
[dependencies]
randomx-sp1 = { git = "https://github.com/chainjump/randomx-sp1.git", rev = "<reviewed-commit>" }
sp1-zkvm = "=6.10.0"
```

The only stable library entry point is `randomx_sp1::hash`:

```rust
#![no_main]

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let randomx_key = sp1_zkvm::io::read_vec();
    let hashing_blob = sp1_zkvm::io::read_vec();
    let digest: [u8; 32] = randomx_sp1::hash(&randomx_key, &hashing_blob);
    sp1_zkvm::io::commit_slice(&digest);
}
```

The library accepts byte slices and performs no SP1 input/output itself. The
parent program therefore controls how inputs arrive and which values become
public. In the example, only the digest is public; the key and blob are private.
If a verifier must bind the result to visible inputs, the parent must also
commit those inputs or suitable commitments to them. Each `hash` call builds
the complete 256 MiB RandomX cache, so a parent should account for that cost
and bound untrusted input lengths as part of its own interface.

Commit the dependent program's `Cargo.lock`, then build and identify the whole
parent guest—not this repository's standalone guest:

```bash
cargo prove build --docker --tag v6.10.0 --no-docker-cache --locked
SP1_PROVER=light cargo prove vkey --elf /path/to/parent-elf
```

Before release, the dependent project must execute and test that complete ELF,
reproduce it from its pinned source and lockfile, derive and publish its own
vkey, generate its own proof, and verify that proof. If EVM verification is a
requirement, it must also repeat the verifier `eth_call` with its own vkey,
public values, and EVM-encoded proof.

The verification boundary is the complete ELF. Linking `randomx-sp1` into a
parent changes that ELF and gives the parent a different vkey. Consequently:

| Check | What it establishes | Required in a dependent project |
| --- | --- | --- |
| Standalone ELF reproduction and vkey derivation | Source identity of this repository's standalone wrapper | Reproduce the complete parent ELF and derive its vkey |
| Network proof and Ethereum `eth_call` (pending for this guest) | Execution of that standalone ELF and acceptance by the deployed verifier | Generate and verify a proof for the complete parent ELF |

The current standalone ELF and vkey do not authenticate a dependent program.
A downstream composition must perform its own identity and proof checks unless
it deliberately implements a separate recursive-proof design. No proof is
retained for the current SP1 6.10.0 guest.

The features whose names contain `audit` and all internal crates are validation
implementation details, not supported downstream APIs. Upstream lineage and
retained licenses are recorded in `ATTRIBUTION.md`; the public library is
GPL-3.0-only.

## Reproduce the current ELF and vkey

Build from this source and its checked-in lockfile with SP1 6.10.0:

```bash
cargo prove build --docker --tag v6.10.0 --no-docker-cache --locked \
  --binaries randomx-sp1-program --output-directory /tmp/randomx-sp1-rebuild
cmp artifacts/randomx-sp1-program /tmp/randomx-sp1-rebuild/randomx-sp1-program
SP1_PROVER=light cargo prove vkey --elf artifacts/randomx-sp1-program
(cd artifacts && sha256sum --check SHA256SUMS)
sha256sum --check artifacts/SOURCE_SHA256SUMS
```

`artifacts/MANIFEST.json` records the current toolchain, guest identity,
reproducibility and local execution. Older ELFs, keys and proof bundles have
been removed. A new proof and gateway validation remain pending for this guest.

The upgrade validation passed all 45 native tests (one deep audit remains
intentionally ignored), strict workspace and feature Clippy checks, and public
documentation generation. All seven requester tests and its strict release
Clippy check also passed. The rebuilt guest matched the selected Monero PoW
hash at 6,485,505,563 cycles and 7,857,197,602 PGU, within the existing limits.
Builds and checks ran offline with bounded memory; no proof was generated.

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
- `program/`: the universal SP1 guest and opt-in cache/no-memory profiling
  guests.
- `softfp/`: exact four-mode binary64 arithmetic for SP1 RV64IM and its
  opt-in validation guest.
- `argon2/`: the RandomX-only subset of an in-tree `rust-argon2` fork, with
  fixed-parameter Argon2d cache construction and upstream differential tests.
- `artifacts/`: the current ELF, vkey, build manifest, checksums and dependency
  audit. Superseded binaries and proof bundles have been removed.
- `executor/`: lightweight execution, cycle-region profiling, and calibrated
  PGU estimation.
- `network-prover/`: fixed-block Succinct Network request, recovery, local
  proof verification, and EVM `eth_call` verification client.
- `audit/`: official-RandomX and reference/optimized differential checks.

## Execute and profile the guest

The executor is optional developer tooling. It runs the ELF without generating
or verifying a proof so developers can check public output, SP1 cycle count,
PGU estimates, and profiling regions. It plays no role in the reproducible
ELF/vkey identity or the proof verification procedure above.

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

To report `cycle-tracker-report` regions, build the same executor with its
profiling feature and add `--profile` before the ELF path:

```bash
cargo build --release --locked -p randomx-sp1-executor --features profiling
target/release/randomx-sp1-executor --profile \
  <guest-elf> <expected-public-values-hex> [input-hex ...]
```

SP1's profiling support selects its portable executor backend. Omit the
feature for ordinary execution and gas estimation so the faster native backend
remains available.

The phase-isolation guests live beside the universal guest but are excluded
from ordinary builds and tests. Build either one explicitly from `program/`:

```bash
cargo prove build --docker --tag v6.10.0 --no-docker-cache --locked \
  --features profile-probes --binaries randomx-sp1-cache-probe
cargo prove build --docker --tag v6.10.0 --no-docker-cache --locked \
  --features profile-probes --binaries randomx-sp1-no-memory-probe
```

## Continuous integration

`.github/workflows/ci.yml` is manual-only. It does not run for commits, pushes,
pull requests, or tags. A maintainer starts it from **Actions → CI → Run
workflow** (or with an equivalent GitHub API/CLI dispatch) when validation is
needed. Its independent jobs check formatting, Clippy, public documentation,
the complete serial release suite, the separate prover-client tests, both
lockfiles, and consumption through a pinned Git revision. CI never reads a
prover key, builds a release ELF, executes the multi-billion-cycle guest,
contacts the prover network, or simulates an EVM transaction.

On a non-GitHub host, configure its CI runner to execute the same commands in
`CONTRIBUTING.md`. The release-only build and proof sequence is documented in
`RELEASING.md`.

## Licensing and security

The public library and derived RandomX core are GPL-3.0-only. Support crates
declare their applicable licenses in their Cargo manifests. The imported
Argon2 fork retains its upstream MIT and Apache-2.0 license texts alongside
the code at `argon2/LICENSE-MIT` and `argon2/LICENSE-APACHE`. See `LICENSE`
and `ATTRIBUTION.md`. Report security issues privately according to
`SECURITY.md`.

The canonical Git remote is
[`https://github.com/chainjump/randomx-sp1`](https://github.com/chainjump/randomx-sp1).
Consumers should use the public HTTPS URL and pin a full commit rather than a
moving branch.

## Shared Cargo intermediates

Sibling checkouts use `../.cargo-build` through `.cargo/config.toml`. Final host
binaries remain under each repository's `target/`; existing deployment paths
still apply. Compatible dependencies share intermediates. Incremental builds
and development/test debug information are disabled, and release profiles are
aligned to avoid unnecessary variants. Keep host lockfiles aligned when updating
SP1. Do not export `CARGO_BUILD_BUILD_DIR` globally: SP1's native executor runs
a nested Cargo build with a separate lock.

The cache is disposable and can be removed when no builds are running. Guest
reproduction uses isolated directories so it cannot reuse a previous guest build.
