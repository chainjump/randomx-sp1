# Current optimized RandomX result

Measurement date: 2026-07-28 UTC

## Artifact

```text
path:    artifacts/randomx-program
size:    273688 bytes
sha256:  a55aa6f4a1b6535bf7771cfa5dc53d38f85e4795eb6dacd339f5fd3581d1c308
SP1:     6.3.1
```

The artifact was rebuilt from the current locked repository with:

```text
cd program
cargo prove build --locked \
  --elf-name randomx-program \
  --output-directory ../artifacts
```

## Complete execution

The retained measurement uses Monero mainnet block 3,727,315:

```text
hashing blob:
101093ba9fd306c6afa883a69ae61498cc8edc5b04ad42664ca1d70cb4f6e14f609d65af22e8a6a8910b0f68387e426a3e54920779782c55fab88b05da6b989d97e67ced90f93a2237a17601

public RandomX hash:
50966d5e6f491b5c2dccefb149c314d996fe36e73e4a18ae4d9f0d0100000000

SP1 cycles:       6766247664
guest exit code:  0
CFROUND by mode:  5472/5042/5087/4931
CFROUND total:    20532
```

The four counters are nearest, down, up, and toward-zero respectively. The
official RandomX hash and the SP1 guest output match exactly.

The same runtime-input ELF executes the original benchmark blob in
6,650,702,843 cycles and returns its official hash.

## Calibrated PGU measurement

Command:

```text
target/release/randomx-executor --estimate-gas \
  artifacts/randomx-program \
  50966d5e6f491b5c2dccefb149c314d996fe36e73e4a18ae4d9f0d0100000000 \
  101093ba9fd306c6afa883a69ae61498cc8edc5b04ad42664ca1d70cb4f6e14f609d65af22e8a6a8910b0f68387e426a3e54920779782c55fab88b05da6b989d97e67ced90f93a2237a17601
```

The command ran once from guest start to halt. One shared-memory trace slot
bounded memory while preserving SP1's canonical 134,217,728-entry gas chunk
boundaries.

```text
chunk  global cycle end  displayed chunk PGU
    1         942896923           1187232412
    2        1865618322           1215254784
    3        2723189841           1122822113
    4        3581271719           1123978013
    5        4438804095           1122857911
    6        5234619230           1020791833
    7        5694737277            399677400
    8        6149256709            395243245
    9        6560597709            355961329
   10        6766247664            181390105
```

SP1 sums the chunks' raw gas and normalizes once. Therefore the final result
is five PGU higher than the sum of the individually displayed, already-rounded
chunk values:

```text
SP1 PGU:             8125209150
PGU per guest cycle: 1.200844183
wall time:           503.37 seconds
maximum RSS:         2634468 KiB
gas trace chunks:    10
guest exit code:     0
```

This is a deterministic local SP1 6.3.1 gas estimate for this exact ELF and
input. It is not a cryptographic proof, a paid prover-network request, a price
quote, or a network billing receipt.

## Current validation

- `cargo check --workspace --locked --offline`: passed.
- `cargo test --release --locked --offline -p randomx-softfp`: 7 passed.
- `cargo test --release --locked --offline -p rustdom-x-compact-vm`: 7 passed.
- Twenty-real-block rich/compact regression: passed in 41.02 seconds; every
  block exercises all four rounding modes and matches official RandomX.
