# Optimized RandomX for SP1

This repository is the source of truth for the optimized SP1 RandomX
implementation. The guest accepts both the RandomX key and hashing blob at
runtime, constructs the complete 256 MiB Argon2d cache, derives each requested
dataset item, executes all eight RandomX programs, and commits the hash.

The retained ELF is universal with respect to RandomX inputs: it does not
embed an epoch key or a hashing blob. Arbitrary key lengths and empty blobs are
supported and covered by the differential corpus.

No SP1 proof or paid prover-network request has been created.

## Retained artifact

The only retained generated ELF is `artifacts/randomx-program`. Representative
SP1 6.3.1 measurements of this exact ELF are:

| Runtime input | SP1 cycles | Result |
|---|---:|---|
| Original benchmark | 6,273,393,504 | official hash matched |
| Monero block 3,727,315 | 6,388,938,325 | official hash matched |
| Official v1.2.3 `test key 000` / text | 6,534,888,255 | official hash matched |
| Official v1.2.3 `test key 000` / empty blob | 6,466,608,707 | official hash matched |
| Official v1.2.3 empty key / empty blob | 6,463,496,330 | official hash matched |

The calibrated gas estimate for block 3,727,315 is **7,744,392,823 PGU**.
It was measured in one uninterrupted 6:57.26 execution using SP1's canonical
gas chunks. This is a local estimate, not a proof or network billing receipt.

Artifact identity:

```text
ac3eff37cbae4583f57cdbc193cca776a80672c77a63c09eb507dc35d154c317  artifacts/randomx-program
size: 295352 bytes
```

## Current optimization

The hot dataset path retains 16-byte decoded superscalar instructions with
precomputed 64-bit immediates. Register operands are stored as byte offsets,
and adjacent opcodes are predecoded to one of 100 static pair handlers. The
interpreter processes 16 pairs per outer iteration. All program generation
still happens from the runtime key inside the guest; no generated RandomX
program is compiled into the ELF.

Against the runtime-key control, the selected block fell from 6,766,247,702 to
6,388,938,325 cycles: **377,309,377 cycles (5.58%)**. A fixed-epoch code
specialization was rejected and reverted because it could not support one
stable program identity across arbitrary RandomX keys.

## Layout

- `program/`: the single universal SP1 guest.
- `compact/`: compact RandomX decoder and VM executor.
- `softfp/`: exact four-mode binary64 arithmetic for SP1 RV64IM.
- `rustdom-x/`: RandomX state, program generation, and dataset derivation.
- `argon2/`: an in-tree `rust-argon2` fork retaining the generic API and tests,
  with optimized RandomX Argon2d cache construction for the SP1 guest.
- `executor/`: lightweight execution and calibrated PGU estimation.
- `network-prover/`: fixed-block Succinct Network request, recovery, local
  proof verification, and EVM `eth_call` verification client.
- `audit/`: official-RandomX and rich/compact differential checks.
- `argon2-native-compare/`: complete-cache differential checks.
- `profile-probes/`, `softfp-guest/`, and `softfp-runner/`: profiling and
  arithmetic validation tools.

## Reproduce the ELF

The repository pins SP1 6.3.1 in `Cargo.lock`. From `program/` run:

```bash
cargo prove build --locked \
  --elf-name randomx-program \
  --output-directory ../artifacts
```

Build the executor from the repository root:

```bash
cargo build --release --locked -p randomx-executor
```

Reproduce block 3,727,315. After the ELF path and expected public hash, the
first input is the runtime RandomX key and the second is the hashing blob:

```bash
target/release/randomx-executor \
  artifacts/randomx-program \
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
- SP1 executions using unrelated runtime keys, including an empty key and
  empty blob;
- rich/compact lockstep state comparisons;
- complete 256 MiB cache digests for multiple keys; and
- software-floating-point comparisons against Berkeley SoftFloat.

Current evidence is under `evidence/`. The SP1-specific unsafe-code, syscall,
ELF-layout, dependency, and provenance review is recorded in
`evidence/sp1-program-safety-review.md`. Rejected candidates and earlier
artifacts remain recoverable from local Git history. There is no configured
Git remote, so preserving or backing up this repository's `.git` directory is
required for that recovery path.
