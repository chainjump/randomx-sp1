# Accepted predecoded scratchpad memory mask

Date: 2026-07-28 UTC

Baseline checkpoint: `ef1c249` (`perf: narrow validated rounding state`)

## Change

Memory-form VM effects stored a three-value memory level and converted it to
the L1, L2, or L3 scratchpad mask on every dynamic execution. RV64IM compiled
that match into a level bounds check, jump-table address calculation, table
load, and mask operation.

The 32-byte `CompactInstr` had five usable padding bytes. Four now hold the
already-selected `u32` mask, while the total size and power-of-two stride stay
unchanged. Only `new_memory` can install the mask; ordinary decoded
instructions retain zero. The memory-index helpers consume the stored mask
directly and assert its presence in debug builds.

The exhaustive opcode-boundary test now also checks all 1,024 decoded cases:
exactly the memory opcode ranges receive a nonzero mask, and each stored mask
matches its decoded memory level.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,664,284,743 | 6,659,521,821 | 4,762,922 (0.071469365186%) |
| Rounding regression | 6,859,655,768 | 6,855,046,799 | 4,608,969 (0.067189508568%) |

Relative to the post-correctness baseline at `3161c60`, accepted
optimizations now save 26,006,583 cycles (0.388998167810%) on the real-block
fixture and 23,732,892 cycles (0.345016021244%) on the rounding regression.

Both guests exited zero and retained their exact official hashes:

```text
real:                 043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
rounding regression:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
9f5f908c1f4e2c481e6762378a6c5eb594cf910e0301a3ed299b0f4998944143  artifacts/randomx-real-memory-mask-candidate     (280328 bytes)
bf737e3951dd1c7721555f1fda66405948a3482ee1455d56e7fb9dfd5b9f2944  artifacts/randomx-cfround-memory-mask-candidate  (280376 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all six compact verifier tests pass, including 1,024 opcode and memory-mask
  boundary cases and both complete fixed hashes;
- the lockstep audit passes all 32 hashes, 256 programs, 524,288 iteration
  states, and every executed instruction state;
- all 20 fixed recent Monero mainnet blocks pass with official hashes, rich
  state, complete scratchpads, ordinary CFROUND, block IDs, chain links, seed
  epoch, and difficulty;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
70a5e44a433439c994cb12f9786909f50fdfcf29e53ac68808b8d368d1863e00  compact/src/lib.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
