# Deep RandomX Argon2 SP1 optimization

This isolated workspace starts from the authoritative full RandomX guest at
8,270,833,431 SP1 cycles. It preserves the complete zero-stdin statement:

```text
fixed seed -> 256 MiB Argon2d cache -> superscalar dataset derivation
           -> eight RandomX VM programs -> fixed Monero PoW hash
```

No proof was run and no authoritative source under `/root/experiment` was
edited.

## Accepted result

The final ELF executes in **6,777,323,550 cycles**, a reduction of
**1,493,509,881 cycles (18.057549985%)** and **1.220368685x fewer cycles** than
the 8,270,833,431-cycle starting point.

It commits the unchanged expected hash and exits successfully:

```text
SP1 cycles: 6777323550
public RandomX hash: 043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
guest exit code: 0
```

The cache-only starting measurement was 6,769,970,423 cycles. Because this
candidate changes only Argon2, its full-guest saving of 1,493,509,881 cycles is
attributable to that phase. The cache-only candidate was not separately
instrumented in this pass.

## Changes

For each of the 786,430 cache blocks compressed after the two initial blocks:

1. Construct each 16-word `R = previous XOR reference` column directly in
   registers, update the destination with `R`, permute the column, and write
   only `P(R)` to scratch. This removes a complete 1 KiB scratch write/read
   pair.
2. Use a `[MaybeUninit<u64>; 128]` scratch array and consume its initialized
   words directly during the row permutation. This removes both the 1 KiB
   scratch clear and an otherwise generated 1 KiB copy.
3. Fold every final row-permutation result directly into the destination. This
   removes another scratch write and final full-block XOR traversal.
4. Use validated raw flat-block access for the fixed one-lane RandomX loop and
   a documented raw word destination inside the compressor. This removes
   repeated slice splitting and residual destination bounds work.

The implementation changes are limited to:

- `argon2/src/core.rs`: 202 insertions and 88 deletions versus the active
  specialized Argon2 source, plus one safety-coverage unit test.
- `argon2/src/block.rs`: `Block` is explicitly `repr(transparent)` and exposes
  its internal word pointer to the crate (six added lines).

The root, guest, compact-VM, and Rustdom manifests differ only to make this
copy self-contained. `native-compare/` is a copied differential harness.

## Safety argument

The optimized scratch has a mechanically simple initialization proof:

- Column `i` writes `16*i .. 16*i+16` for `i = 0..8`. Those eight disjoint
  ranges partition exactly `0..128`.
- Row `i` reads word pairs `2*i + {0,16,32,48,64,80,96,112}` and each adjacent
  word. Across `i = 0..8`, those indices also partition exactly `0..128`.
- No scratch word is read before the entire column loop finishes. Scratch
  elements are `u64`, so copying them has no drop behavior.
- The unit test `fused_scratch_index_maps_are_exact_partitions` independently
  checks both partitions.

For raw cache access, the fixed RandomX context is asserted to contain exactly
262,144 blocks. Both source indices are lane-masked and therefore in bounds;
the Argon2 reference-area formula and previous-block rule exclude the current
block. Previous and reference blocks may coincide, but both are shared reads;
the current block is the only mutable reference. Existing boundary tests check
the specialized index formula against the generic formula and assert the
reference is in range and distinct from the current block.

The raw compressor receives 128 aligned writable words. Its safe wrapper
obtains them from an initialized `Block`; when `with_xor` is false it writes all
128 words during the column loop before the row loop reads them. When
`with_xor` is true, the wrapper guarantees the destination was initialized.

## Measurements

| Candidate | Full SP1 cycles | Result |
|---|---:|---|
| Authoritative starting point | 8,270,833,431 | baseline |
| Safe `array::from_fn` scratch | 8,538,744,083 | rejected regression |
| Fused final row stores | 7,779,314,741 | accepted intermediate |
| Fused column construction, no scratch copy | 6,782,645,781 | accepted intermediate |
| Validated raw cache access | 6,779,420,718 | accepted intermediate |
| **Final raw destination access** | **6,777,323,550** | **accepted** |

## Correctness evidence

- 61 Argon2 unit tests pass.
- 21 Argon2 integration/vector tests pass across Argon2d/i/id and versions
  1.0/1.3.
- 12 documentation tests pass.
- Complete 256 MiB cache digests match the crates.io generic Argon2
  implementation for three keys:

```text
selected Monero seed  152add6ff4fd241ba703f004dcea77fea6c2d55d8b20100aae1578e7bca88a5c
32 zero bytes          f303edc0c3dc803869f25bb11178193805d767427e11f519bb2ac123ea1ef63e
empty key              faf16925e389d546a2ebf79d1329ed4f8f217902ba00a5641447773725306d15
```

- The exact full SP1 guest returns the expected fixed Monero hash and exit code
  zero.

Every build, test, comparison, and execution command used a watchdog below 60
seconds.

## Reproduction

Build from `program-full/`:

```bash
timeout --signal=INT --kill-after=1s 55s \
  env CARGO_TARGET_DIR=/root/experiment/target \
      PATH=/root/.sp1/bin:/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  cargo prove build --locked --elf-name argon-deep-final \
  --output-directory /root/experiment/optimization-argon-deep/artifacts
```

Execute from the workspace root:

```bash
timeout --signal=INT --kill-after=1s 55s \
  /root/experiment/target/release/execute-fast \
  /root/experiment/optimization-argon-deep/artifacts/argon-deep-final
```

Run vector tests:

```bash
timeout --signal=INT --kill-after=1s 55s \
  env CARGO_TARGET_DIR=/root/experiment/optimization-argon-deep/host-target \
      PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  cargo test --locked -p rustdom-x-argon2 --all-targets
```

Run full-cache differential comparison:

```bash
timeout --signal=INT --kill-after=1s 55s \
  env CARGO_TARGET_DIR=/root/experiment/optimization-argon-deep/host-target \
      PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  cargo run --offline --release -p argon2-native-compare
```

## Checksums

```text
5c9fb4fc76e38e7aa88ad61b426a72af2ab5e5e66277138379bc3540703de21f  artifacts/argon-deep-final
1af4f793500a40fe82a6f0f597be5a453facd2f12fa654975582750b4386f1fa  argon2/src/core.rs
005a7910f881f938725da0dee172b471c0ee50d32fe5df8ec02e8302f55b416a  argon2/src/block.rs
```

The source checksums above were taken before adding this README; the core
checksum includes the scratch-partition safety test.
