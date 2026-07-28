# Accepted power-of-two compact instruction stride

Date: 2026-07-28 UTC

Baseline checkpoint: `d092719` (`perf: elide masked scratchpad bounds checks`)

## Change

The decoded `CompactInstr` was 24 bytes. Every dynamic VM instruction lookup
therefore formed `pc * 24` as two shifts and a subtraction before its indirect
effect call. Across eight programs and 2,048 iterations, branching causes each
complete hash to perform roughly 4.1 million such lookups.

Eight additional reserved bytes now make the `repr(C)` instruction exactly 32
bytes without changing any live field. RV64IM indexes the decoded program with
one left shift. The transient decoded program grows from 6 KiB to 8 KiB; it is
rebuilt once per generated program and freed after that program runs.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,679,275,975 | 6,670,983,161 | 8,292,814 (0.124157379199%) |
| Rounding regression | 6,872,527,262 | 6,864,234,897 | 8,292,365 (0.120659615944%) |

The 449-cycle difference between reductions follows the input-dependent
number of dynamically executed instructions after conditional branches.
Relative to the post-correctness baseline at `3161c60`, accepted
optimizations now save 14,545,243 cycles (0.217563102287%) on the real-block
fixture. Relative to the clean imported baseline, the complete current result
saves 105,456,643 cycles (1.556224891687%).

Both guests exited zero and retained their exact official hashes:

```text
real:                 043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
rounding regression:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
6945db9890336d53b6737dee0a3ddf73822885072589f045aeb30f03232ae05c  artifacts/randomx-real-stride32-candidate     (282168 bytes)
054eb62e7a77020d10579a15b45ec40d101bbe3836c9c7eab89c45542c49badd  artifacts/randomx-cfround-stride32-candidate  (282216 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- compile-time and unit assertions require the decoded instruction to remain
  exactly 32 bytes;
- all five compact verifier tests pass, including 1,024 opcode-boundary
  comparisons and both official fixed hashes;
- the lockstep audit passes 32 hashes, 256 generated programs, 524,288 VM
  iteration states, and every executed instruction state;
- all 20 fixed recent Monero mainnet blocks pass with their official hashes,
  rich state, complete scratchpads, block IDs, chain links, seed epoch,
  difficulty, and ordinary CFROUND coverage;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
27c029eb19b524ab6518232b1d0d62a906fea391328571cc064a85541c60a7bd  compact/src/lib.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
