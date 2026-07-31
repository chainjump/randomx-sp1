# Optimized RandomX for SP1

This repository is the source of truth for the optimized SP1 RandomX
implementation. The guest accepts both the RandomX key and hashing blob at
runtime, constructs the complete 256 MiB Argon2d cache, derives each requested
dataset item, executes all eight RandomX programs, and commits the hash.

The implementation is universal with respect to RandomX inputs: it does not
embed an epoch key or a hashing blob. Arbitrary key lengths and empty blobs are
supported and covered by the differential corpus.

A reproducible SP1 v6.3.1 ELF, locally derived vkey, and fulfilled mainnet
Groth16 proof are retained and approved for the current source tree. Their
identities, disassembly review, real-block execution measurements, local SP1
verification, and Ethereum-mainnet `eth_call` verification are recorded in
`evidence/reproducible-build-2026-07-30.md` and
`evidence/network-proof/README.md`.

## Independently verify the retained release

The prover network generated the proof; Ethereum only verifies it. The
retained tuple has these identities:

| Item | Retained identity |
| --- | --- |
| SP1 ELF | SHA-256 `d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940` |
| Program vkey | `0x00ef0352217c1bd40da717b661a67da22554bbddc4589ee54fd836f15cc0a771` |
| Public values | `5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000` |
| SDK proof file | 1,726 bytes; SHA-256 `0c81249c035a3ab826f9be9e6a61aee5df54198ec7aecea2ea8d4f380fe93a2d` |
| Network request | `0xf0ca00e4ef3e4c2f78d51977d4e0a6e66168a98ffa0f9b3b137df44ea2a95603` |

The verification chain is:

```text
source --reproducible build--> ELF --SP1 setup--> program vkey
                                                       + public values
                                                       + Groth16 proof
                                                               |
                                                     local/EVM verifier
```

The EVM does **not** receive the ELF or its SHA-256. It receives the 32-byte
SP1 program vkey, the 32 public-value bytes, and the 356-byte on-chain encoding
of the Groth16 proof. The vkey is SP1's commitment to the program verification
key derived from the ELF; it is not simply the ELF's SHA-256. A verifier must
therefore use a trusted vkey or independently derive it from the ELF.

The proof establishes successful execution of the SP1 program identified by
that vkey with the stated public values. This standalone guest commits only
the 32-byte RandomX digest. Its RandomX key and hashing blob are private inputs,
so the EVM does not learn or independently identify those inputs.

### Minimum proof and EVM verification

These steps trust the published vkey. They require the repository's Rust
toolchain, `protoc`, and an Ethereum-mainnet JSON-RPC URL. They require no
prover key, PROVE, ETH, EVM signer, RandomX execution, or proof generation.

First check every retained byte against the committed manifest:

```bash
(cd evidence/network-proof && sha256sum --check SHA256SUMS)
```

All four entries must report `OK`. This is an integrity check; the independent
source-to-ELF and ELF-to-vkey checks are below. If `protoc` is not already
installed, install it first (on Debian or Ubuntu):

```bash
sudo apt-get update
sudo apt-get install --no-install-recommends -y protobuf-compiler libprotobuf-dev
```

Then build the existing host-side verification client:

```bash
CARGO_TARGET_DIR=target cargo build --release --locked \
  --manifest-path network-prover/Cargo.toml
```

Simulate the exact verifier call against the canonical Ethereum-mainnet
Groth16 gateway:

```bash
EVM_RPC_URL='<ethereum-mainnet-rpc-url>' \
target/release/randomx-sp1-network-prover evm-verify \
  artifacts/randomx-sp1-program \
  evidence/network-proof/program-vkey \
  evidence/network-proof/proof.bin
```

Success ends with:

```text
EVM verification simulation: true (eth_call did not revert)
EVM transaction broadcast: no
chain: Ethereum mainnet (1)
Groth16 gateway: 0x397a5f7f3dbd538f23de225b51f532c34448da9b
program vkey: 0x00ef0352217c1bd40da717b661a67da22554bbddc4589ee54fd836f15cc0a771
public values: 5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000
```

The command checks the retained ELF digest and expected public values, decodes
the committed SDK proof, constructs
`verifyProof(bytes32 programVKey, bytes publicValues, bytes proofBytes)`,
requires chain ID 1, and issues `eth_call`. The verifier has no Boolean return
value: success is a non-reverting call with empty (`0x`) return data. The client
contains no EVM signer and cannot broadcast a transaction. Succinct documents
this verifier interface and vkey model in its
[Solidity verifier guide](https://docs.succinct.xyz/docs/sp1/verification/solidity-sdk)
and publishes the gateway in its
[canonical address list](https://docs.succinct.xyz/docs/sp1/verification/contract-addresses).

### Independently bind the source, ELF, and vkey

To avoid trusting the published ELF and vkey identities, also reproduce the
ELF into a temporary directory and compare it byte-for-byte. This additionally
requires Docker and the SP1 6.3.1 CLI (`sp1up --version 6.3.1`):

```bash
rebuild_dir="$(mktemp -d)"
(
  cd program
  cargo prove build --docker --tag v6.3.1 --locked \
    --binaries randomx-sp1-program \
    --elf-name randomx-sp1-program \
    --output-directory "$rebuild_dir"
)
cmp artifacts/randomx-sp1-program "$rebuild_dir/randomx-sp1-program"
sha256sum "$rebuild_dir/randomx-sp1-program"
```

`cmp` must exit successfully and SHA-256 must be
`d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940`.
Docker is used only to make the guest build reproducible across hosts.

Then derive the program vkey from the retained ELF with the pinned SP1 CLI and
compare it with the committed value:

```bash
derived_vkey="$(
  cargo prove vkey --elf artifacts/randomx-sp1-program | tail -n 1
)"
test "$derived_vkey" = "$(tr -d '\r\n' < evidence/network-proof/program-vkey)"
printf '%s\n' "$derived_vkey"
```

The expected output is
`0x00ef0352217c1bd40da717b661a67da22554bbddc4589ee54fd836f15cc0a771`.
This is an SP1 setup operation, not a RandomX execution or proof. On the release
host it took 5 minutes 7 seconds and about 6.2 GiB peak memory; the command may
be silent while SP1 initializes. The executor is not involved in rebuilding
the ELF, deriving the vkey, or verifying the proof.

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
- `executor/`: lightweight execution, cycle-region profiling, and calibrated
  PGU estimation.
- `network-prover/`: fixed-block Succinct Network request, recovery, local
  proof verification, and EVM `eth_call` verification client.
- `audit/`: official-RandomX and reference/optimized differential checks.

Consumers should depend on `randomx-sp1` and use its stable entry point:

```rust
let digest: [u8; 32] = randomx_sp1::hash(&randomx_key, &hashing_blob);
```

Use an immutable release commit rather than a branch:

```toml
[dependencies]
randomx-sp1 = { git = "https://github.com/chainjumper/randomx-sp1.git", rev = "3dc340183d6306176c9409bb6bdab4e336b72585" }
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

## Execute and profile the guest

The executor is optional developer tooling. It runs the ELF without generating
or verifying a proof so developers can check public output, SP1 cycle count,
PGU estimates, and profiling regions. It plays no role in the reproducible
build or the ELF-to-vkey-to-proof verification chain above.

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
cargo prove build --docker --tag v6.3.1 --locked \
  --features profile-probes --binaries randomx-sp1-cache-probe
cargo prove build --docker --tag v6.3.1 --locked \
  --features profile-probes --binaries randomx-sp1-no-memory-probe
```

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

The exhaustive 32-hash reference/optimized lockstep audit is intentionally
ignored by the default suite. Run it explicitly when changing the interpreter:

```bash
cargo test --release --locked -p randomx-sp1-audit \
  --test differential -- --ignored
```

Current source evidence is under `evidence/`. The SP1-specific unsafe-code,
syscall, dependency, and provenance review is recorded in
`evidence/sp1-program-safety-review.md`.

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
[`https://github.com/chainjumper/randomx-sp1`](https://github.com/chainjumper/randomx-sp1).
Consumers should use the public HTTPS URL and pin a full commit rather than a
moving branch.
