# Optimized RandomX for SP1

This repository is the source of truth for the optimized SP1 RandomX
implementation. The guest accepts both the RandomX key and hashing blob at
runtime, constructs the complete 256 MiB Argon2d cache, derives each requested
dataset item, executes all eight RandomX programs, and commits the hash.

The implementation is universal with respect to RandomX inputs: it does not
embed an epoch key or a hashing blob. Arbitrary key lengths and empty blobs are
supported and covered by the differential corpus.

A reproducible SP1 v6.3.1 ELF and its derived vkey define the production
program identity. Separately, one fulfilled mainnet Groth16 proof is retained
as deployment evidence for a single Monero block.

## Correctness

The implementation is checked against:

- all 84 portable checks from the canonical RandomX v1.2.3 `randomx-tests`
  program (the 11 JIT-, SIMD-, and alternate-implementation checks are not
  applicable and are itemized in `evidence/canonical-v1.2.3-test-port.md`);
- 20 consecutive real Monero blocks;
- 42 official RandomX v1.2.3 light-mode hashes across seven key shapes and six
  blob shapes;
- reference/optimized lockstep state comparisons;
- complete 256 MiB cache digests for multiple keys; and
- software-floating-point comparisons against Berkeley SoftFloat.

These tests and differential checks are the general correctness evidence. The
single proof retained later in this README demonstrates deployment of one
execution; it does not replace or strengthen this corpus.

The exhaustive 32-hash reference/optimized lockstep audit is intentionally
ignored by the default suite. Run it explicitly when changing the interpreter:

```bash
cargo test --release --locked -p randomx-sp1-audit \
  --test differential -- --ignored
```

The detailed corpus is under `evidence/`. The SP1-specific unsafe-code,
syscall, dependency, and provenance review is recorded in
`evidence/sp1-program-safety-review.md`.

## Reproduce the ELF and vkey

The ELF and vkey are the reusable production artifacts. Every proof for this
standalone program is tied to this program identity; the proof of one block is
not part of that identity.

| Artifact | Production identity |
| --- | --- |
| [SP1 ELF](artifacts/randomx-sp1-program) | 289,512 bytes; SHA-256 `d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940` |
| [Program vkey](artifacts/randomx-sp1-program.vkey) | `0x00ef0352217c1bd40da717b661a67da22554bbddc4589ee54fd836f15cc0a771` |

Check the retained production artifacts with:

```bash
(cd artifacts && sha256sum --check SHA256SUMS)
```

To independently bind the source to the ELF, install Docker and the SP1 6.3.1
CLI (`sp1up --version 6.3.1`), then build into a temporary directory:

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

`cmp` must succeed and SHA-256 must be
`d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940`.
The command was independently rerun after writing this procedure and produced
a byte-identical ELF.

Then derive the vkey from that exact ELF and compare it with the retained
value:

```bash
derived_vkey="$(
  cargo prove vkey --elf "$rebuild_dir/randomx-sp1-program" | tail -n 1
)"
test "$derived_vkey" = "$(tr -d '\r\n' < artifacts/randomx-sp1-program.vkey)"
printf '%s\n' "$derived_vkey"
```

The expected output is
`0x00ef0352217c1bd40da717b661a67da22554bbddc4589ee54fd836f15cc0a771`.
This SP1 setup operation may be silent while it initializes; on the release
host it took 5 minutes 7 seconds and about 6.2 GiB peak memory. It does not
execute RandomX or generate a proof. The optional executor described below is
not involved in either reproduction step.

The EVM never needs the full ELF or the ELF SHA-256. SP1 setup derives the
program verification key from the ELF, and on-chain verifiers receive its
32-byte `programVKey` commitment. This value is not simply the ELF SHA-256.

## One-block proof: deployment evidence

The retained proof demonstrates that the Succinct Prover Network and the
Ethereum verifier accepted one execution for Monero block 3,727,837. It is a
deployment test, not a general correctness claim or a second program identity.

| Proof item | Value |
| --- | --- |
| [Network request](https://explorer.succinct.xyz/request/0xf0ca00e4ef3e4c2f78d51977d4e0a6e66168a98ffa0f9b3b137df44ea2a95603) | `0xf0ca00e4ef3e4c2f78d51977d4e0a6e66168a98ffa0f9b3b137df44ea2a95603` |
| [Public RandomX digest](evidence/network-proof/public-values.hex) | `5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000` |
| [SDK proof](evidence/network-proof/proof.bin) | 1,726 bytes; SHA-256 `0c81249c035a3ab826f9be9e6a61aee5df54198ec7aecea2ea8d4f380fe93a2d` |
| [EVM proof encoding](evidence/network-proof/proof-evm.hex) | 356 bytes; SHA-256 `2309bc02f6babbb1d786e7e06c9b620649f0b57fa9c7d8ed0b1c5807f7d7fff6` |

The proof shows successful execution of the program identified by the vkey
with those public-value bytes. The standalone guest commits only the RandomX
digest; its key and hashing blob remain private. Consequently, the EVM does
not itself identify the Monero block or expose its RandomX inputs.

### Verify on Ethereum without repository code

This procedure uses only the external Foundry `cast` CLI, three retained data
files, and an Ethereum-mainnet RPC URL. It does not compile or run any Rust—or
any other executable code—from this repository. It assumes the program vkey
has already been accepted or independently reproduced as described above.
Install Foundry from its
[official instructions](https://getfoundry.sh/introduction/installation) if
`cast --version` is unavailable.

First verify the downloaded data files:

```bash
(cd evidence/network-proof && sha256sum --check SHA256SUMS)
```

Then issue the read-only verifier call:

```bash
export EVM_RPC_URL='<ethereum-mainnet-rpc-url>'
test "$(cast chain-id --rpc-url "$EVM_RPC_URL")" = '1'

program_vkey="$(tr -d '\r\n' < artifacts/randomx-sp1-program.vkey)"
public_values="$(tr -d '\r\n' < evidence/network-proof/public-values.hex)"
proof_bytes="$(tr -d '\r\n' < evidence/network-proof/proof-evm.hex)"

cast call 0x397a5f7f3dbd538f23de225b51f532c34448da9b \
  'verifyProof(bytes32,bytes,bytes)' \
  "$program_vkey" "$public_values" "$proof_bytes" \
  --rpc-url "$EVM_RPC_URL"
```

Success prints `0x`: the verifier function returned no data and did not
revert. `cast call` uses `eth_call`; it does not sign or broadcast a transaction
and requires no private key, ETH, or PROVE. The EVM receives only the 32-byte
program vkey, 32 public-value bytes, and 356 proof bytes.

The three files are transparent data, not executables. `proof-evm.hex` is the
verifier-ready encoding of the same proof stored in SP1's 1,726-byte
`proof.bin` container. The command above was tested independently with Foundry
1.7.1 against Ethereum mainnet and returned `0x`.

Succinct documents the interface in its
[Solidity verifier guide](https://docs.succinct.xyz/docs/sp1/verification/solidity-sdk)
and publishes the gateway in its
[canonical address list](https://docs.succinct.xyz/docs/sp1/verification/contract-addresses).
Foundry documents `cast call` in its
[CLI reference](https://getfoundry.sh/cast/reference/cast-call.md). Full request,
cost, and execution details are in `evidence/network-proof/README.md`.

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
- `artifacts/`: the approved reusable ELF/vkey identity and its checksum
  manifest.
- `evidence/network-proof/`: data for the separate one-block deployment proof.
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
cargo prove build --docker --tag v6.3.1 --locked \
  --features profile-probes --binaries randomx-sp1-cache-probe
cargo prove build --docker --tag v6.3.1 --locked \
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
[`https://github.com/chainjumper/randomx-sp1`](https://github.com/chainjumper/randomx-sp1).
Consumers should use the public HTTPS URL and pin a full commit rather than a
moving branch.
