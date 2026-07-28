# Accepted light-mode dataset read

Date: 2026-07-28 UTC

Baseline checkpoint: `528c295` (`perf: carry Argon previous block offset`)

## Change

The compact verifier always constructs `VmMemory::light` (or the zero-memory
test provider), but its 16,384 VM iterations called the general dataset reader.
That function checked the runtime `cache` flag and retained the full-dataset
`RwLock` lookup and population paths in the guest.

`VmMemory::dataset_read_light` now directly derives and mixes the requested
dataset item. The compact verifier calls this light-only entry point. The
existing `dataset_read` method and full-dataset behavior remain unchanged for
the rich implementation. A debug assertion catches accidental use with the
cached provider; deriving the item directly is still value-equivalent in a
release build.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,682,339,771 | 6,681,946,555 | 393,216 (0.005884405964%) |
| CFROUND-heavy | 6,875,591,058 | 6,875,197,842 | 393,216 (0.005719013779%) |

The reduction is exactly 24 cycles for each of the 16,384 dataset reads and is
identical for both complete guests. Relative to the post-correctness baseline
at `3161c60`, the accepted optimizations now save 3,581,849 cycles
(0.053576154098%) on the real-block fixture.

Both guests exited zero and retained their exact official hashes:

```text
real:     043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
CFROUND:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
ac9b205fdb16a60be316ac03eef1f1176be71a73e29c1908ad684e3556c5e308  artifacts/randomx-real-light-dataset-candidate     (282984 bytes)
f55a4575ac8e52efecebce8f44ecf61adef7a74bc0c7035c96a2e4a40bcab68a  artifacts/randomx-cfround-light-dataset-candidate  (283032 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all four compact verifier tests pass, including the 1,024 opcode-boundary
  comparisons and directed CFROUND hash;
- all nine `rustdom-x` tests pass with the verifier's
  `unchecked-superscalar` feature;
- the fixed 20-block recent Monero mainnet test passes, comparing each compact
  result with its frozen official RandomX v1.2.3 hash, rich-VM state, complete
  scratchpad, block ID, chain link, seed epoch, and network difficulty;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
79767d66e2c036c79bbf61c248863ea9bbb19231ed2d38a242b9294c2ceb5e39  compact/src/lib.rs
94bd64776a75222d4237d76038dbbccffcc358e4fc78a66308a10289f95f1dc2  rustdom-x/src/memory.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
