# Current optimized RandomX result

Measurement date: 2026-07-28 UTC

## Artifact

```text
path:    artifacts/randomx-program
size:    295352 bytes
sha256:  ac3eff37cbae4583f57cdbc193cca776a80672c77a63c09eb507dc35d154c317
SP1:     6.3.1
```

The artifact was rebuilt from the current locked repository with:

```text
cd program
cargo prove build --locked \
  --elf-name randomx-program \
  --output-directory ../artifacts
```

The guest reads two vectors at runtime: the RandomX key followed by the
hashing blob. Neither value is embedded in the ELF.

## Complete execution

The retained measurement uses Monero mainnet block 3,727,315:

```text
RandomX key:
11c798e5ac6515218bc3efcb5416e5b68c599e42a61b86efe5746bb78eb4be8e

hashing blob:
101093ba9fd306c6afa883a69ae61498cc8edc5b04ad42664ca1d70cb4f6e14f609d65af22e8a6a8910b0f68387e426a3e54920779782c55fab88b05da6b989d97e67ced90f93a2237a17601

public RandomX hash:
50966d5e6f491b5c2dccefb149c314d996fe36e73e4a18ae4d9f0d0100000000

SP1 cycles:       6388938325
guest exit code:  0
```

The runtime-key control took 6,766,247,702 cycles. The accepted interpreter
changes save 377,309,377 cycles, or 5.576345910134%. The same ELF executes the
original benchmark in 6,273,393,504 cycles and returns its official hash.

## Seed-independent optimization

The eight superscalar programs remain runtime-generated from the supplied
key. The accepted hot-path changes are:

1. Store register operands as prevalidated byte offsets, avoiding repeated
   RV64IM index scaling.
2. Predecode adjacent opcode pairs to one of 100 static handlers. Each handler
   performs two runtime-selected operations with one indirect dispatch.
3. Process 16 pairs per outer loop iteration, with a generic remainder path.

The instruction record remains 16 bytes so it can retain precomputed 64-bit
reciprocals and signed immediates. Porting the canonical 8-byte record plus a
separate reciprocal table was correct but regressed to 6,835,961,711 cycles.

Selected A/B measurements on the same runtime key and blob:

| Candidate | SP1 cycles | Change from control |
|---|---:|---:|
| Runtime-key control | 6,766,247,702 | — |
| Four-way opcode-loop unroll | 6,593,707,798 | -172,539,904 |
| Eight-way unroll plus register byte offsets | 6,472,112,948 | -294,134,754 |
| Paired handlers, 4 pairs/iteration | 6,397,785,685 | -368,462,017 |
| Paired handlers, 8 pairs/iteration | 6,390,937,173 | -375,310,529 |
| Paired handlers, 16 pairs/iteration | **6,388,938,325** | **-377,309,377** |
| Paired handlers, 32 pairs/iteration | 6,389,200,469 | -377,047,233 |

Six- and sixteen-instruction direct unrolls, extra two-instruction tail
unrolling, a fixed program-count loop, and an explicit cache-line XOR loop
were also measured and rejected. A build-time fixed-key specialization was
reverted because its program identity was not valid for arbitrary keys.

## Calibrated PGU measurement

Command:

```text
target/release/randomx-executor --estimate-gas \
  artifacts/randomx-program \
  50966d5e6f491b5c2dccefb149c314d996fe36e73e4a18ae4d9f0d0100000000 \
  11c798e5ac6515218bc3efcb5416e5b68c599e42a61b86efe5746bb78eb4be8e \
  101093ba9fd306c6afa883a69ae61498cc8edc5b04ad42664ca1d70cb4f6e14f609d65af22e8a6a8910b0f68387e426a3e54920779782c55fab88b05da6b989d97e67ced90f93a2237a17601
```

The command ran once from guest start to halt. One shared-memory trace slot
bounded memory while preserving SP1's canonical 134,217,728-entry gas chunk
threshold.

```text
chunk  global cycle end  displayed chunk PGU
    1         942896945           1187232469
    2        1865618344           1215254845
    3        2723189863           1122822113
    4        3581271745           1123978017
    5        4438804117           1122946052
    6        5226688698           1011452961
    7        5628867917            332496638
    8        6010581292            314518536
    9        6375230009            298299852
   10        6388938325             15391335
```

SP1 sums raw chunk gas and normalizes once. The final result is five PGU above
the sum of the individually displayed, already-rounded chunk values:

```text
SP1 PGU:             7744392823
PGU per guest cycle: 1.212156454
wall time:           417.26 seconds
maximum RSS:         2635112 KiB
gas trace chunks:    10
guest exit code:     0
```

This is 380,816,327 PGU (4.686849531744%) below the previous retained
8,125,209,150-PGU estimate. It is a deterministic local SP1 6.3.1 gas
estimate for this exact ELF and input, not a cryptographic proof, paid
prover-network request, price quote, or billing receipt.

## Current validation

- `cargo test --release --locked -p rustdom-x --features unchecked-superscalar --lib`:
  10 passed, including all 100 paired-handler combinations.
- `cargo test --release --locked -p rustdom-x-compact-vm --lib`: 7 passed.
- Twenty-real-block rich/compact regression: passed; all official hashes,
  final registers, and complete scratchpads matched.
- Official RandomX v1.2.3 comparisons for `test key 000` and an empty key:
  12 passed.
- The final SP1 ELF matched official outputs for `test key 000` with both a
  text blob and an empty blob, plus the empty-key/empty-blob case. The retained
  program is not tied to the benchmark epoch key.
