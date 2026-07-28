# Accepted direct scratchpad byte offsets

Date: 2026-07-28 UTC

Baseline checkpoint: `d23214a` (`perf: predecode scratchpad memory masks`)

## Change and safety argument

RandomX scratchpad masks are byte-address masks with their low three bits
cleared. The compact memory helpers converted the masked address to a `u64`
word index by shifting right three; Rust slice addressing immediately shifted
that index left three again to form a byte address. RV64IM retained both
operations in every memory-form effect.

Instruction memory operands now apply their decoded mask and use the resulting
aligned byte offset directly against the scratchpad allocation. Iteration
mixing continues to use ordinary word indices.

The unsafe load/store boundary checks the following invariants in debug builds:

- byte offsets are eight-byte aligned;
- the selected eight-byte word ends within the allocation;
- `calculate_hash` already requires exactly 262,144 scratchpad words at its
  public boundary;
- compile-time assertions bind the largest L3 mask to that allocation size.

All masks and decoded instructions remain private to the compact crate.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,659,521,821 | 6,657,658,031 | 1,863,790 (0.027986844252%) |
| Rounding regression | 6,855,046,799 | 6,853,244,607 | 1,802,192 (0.026290002867%) |

Relative to the post-correctness baseline at `3161c60`, accepted
optimizations now save 27,870,373 cycles (0.416876143751%) on the real-block
fixture and 25,535,084 cycles (0.371215319389%) on the rounding regression.

Both guests exited zero and retained their exact official hashes:

```text
real:                 043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
rounding regression:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
ffab35b8ec87601ce8a4981299642098f43c5605572eebcd6f44082ca2001009  artifacts/randomx-real-byte-offset-candidate     (280248 bytes)
087f9788be39b7e3909efc453b3e038113f9fa2481be6d327473ba92785e8065  artifacts/randomx-cfround-byte-offset-candidate  (280296 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all six compact verifier tests pass, including exhaustive opcode/memory-mask
  boundaries and both complete fixed hashes;
- the lockstep audit passes all 32 hashes, 256 programs, 524,288 iteration
  states, and every executed instruction and memory state;
- all 20 fixed recent Monero mainnet blocks pass with official hashes, rich
  state, complete scratchpads, ordinary CFROUND, block IDs, chain links, seed
  epoch, and difficulty;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
3e03b47a8928037c8e504d7ed9d90a0f10eed58874e89f3003748fcdbc93a290  compact/src/lib.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
