# Optimized RandomX for SP1

This repository is the sole source of truth for the optimized SP1 RandomX
implementation. The guest constructs the complete 256 MiB Argon2d cache from
the fixed epoch key, derives each requested dataset item, executes all eight
RandomX programs, and commits the hash of a runtime-supplied hashing blob.

CFROUND is an ordinary supported RandomX opcode. The VM implements all four
resulting rounding modes through the same code path used by every execution.

No SP1 proof or paid prover-network request has been created.

## Retained artifact

The only retained generated ELF is `artifacts/randomx-program`. Its cycle count
depends on the runtime hashing blob. Two useful measurements of the same ELF
and implementation are:

| Input | SP1 cycles | Result |
|---|---:|---|
| Original benchmark blob | 6,650,702,843 | official hash matched |
| Monero block 3,727,315 | 6,766,247,664 | official hash matched; all four rounding modes exercised |

Block 3,727,315 executes CFROUND 20,532 times, distributed by resulting mode
as `5472/5042/5087/4931` (nearest/down/up/toward-zero).

The calibrated SP1 6.3.1 gas estimate for that block is **8,125,209,150 PGU**.
It was measured in one uninterrupted 8:23.37 execution using ten canonical
gas chunks. This is a local estimate, not a proof or network billing receipt.

Artifact identity:

```text
a55aa6f4a1b6535bf7771cfa5dc53d38f85e4795eb6dacd339f5fd3581d1c308  artifacts/randomx-program
size: 273688 bytes
```

## Layout

- `program/`: the single SP1 guest.
- `compact/`: optimized compact RandomX decoder and VM executor.
- `softfp/`: exact four-mode binary64 arithmetic for SP1 RV64IM.
- `rustdom-x/`: RandomX state, program generation, and dataset derivation.
- `argon2/`: optimized RandomX Argon2d cache construction.
- `executor/`: lightweight execution and calibrated PGU estimation.
- `audit/`: official-RandomX and rich/compact differential checks.
- `argon2-native-compare/`: complete-cache differential checks.
- `profile-probes/`, `softfp-guest/`, and `softfp-runner/`: current profiling
  and arithmetic validation tools.

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

Reproduce the block 3,727,315 execution:

```bash
target/release/randomx-executor \
  artifacts/randomx-program \
  50966d5e6f491b5c2dccefb149c314d996fe36e73e4a18ae4d9f0d0100000000 \
  101093ba9fd306c6afa883a69ae61498cc8edc5b04ad42664ca1d70cb4f6e14f609d65af22e8a6a8910b0f68387e426a3e54920779782c55fab88b05da6b989d97e67ced90f93a2237a17601
```

Run the calibrated SP1 gas estimator in one uninterrupted streaming pass by
adding `--estimate-gas` before the ELF path. It uses canonical gas chunk
boundaries and one shared-memory trace slot, so memory stays bounded without
changing the PGU result.

## Correctness

The current implementation is checked against:

- 20 consecutive real Monero blocks, each exercising CFROUND and collectively
  covering all four modes;
- 42 official RandomX v1.2.3 light-mode hashes across varied keys and blobs;
- rich/compact lockstep state comparisons;
- complete 256 MiB cache digests for multiple keys; and
- software-floating-point comparisons against Berkeley SoftFloat.

Current evidence is under `evidence/`. Earlier optimization notes and generated
ELFs remain recoverable from local Git history rather than the working tree.
There is currently no configured Git remote, so preserving this repository's
`.git` directory is required for that recovery path.
