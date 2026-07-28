# Accepted masked iteration scratchpad access

Date: 2026-07-28 UTC

Baseline checkpoint: `a4c67e4` (`perf: specialize light dataset reads`)

## Change and safety argument

Each of RandomX's 16,384 VM iterations reads and writes two aligned
eight-word scratchpad groups. The addresses are reduced by
`SCRATCHPAD_L3_MASK_U32` (`0x1fffc0`) and divided by eight before use. The
largest possible starting word is therefore 262,136; adding the largest
in-group index, seven, reaches word 262,143 in the 262,144-word scratchpad.

The compact loop now uses the existing checked-in-debug, unchecked-in-release
scratchpad helpers for those accesses. Compile-time assertions bind both the
iteration mask and the instruction-memory mask to the exact scratchpad size.
Because `Vm::scratchpad` is publicly mutable, `calculate_hash` also performs
one release-mode length assertion before any unchecked operation. A directed
test shortens the vector and verifies that the public API rejects it rather
than entering the unsafe path. No instruction, address calculation, VM state,
or scratchpad value changes.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,681,946,555 | 6,679,275,975 | 2,670,580 (0.039967096085%) |
| CFROUND-heavy | 6,875,197,842 | 6,872,527,262 | 2,670,580 (0.038843682195%) |

The public-boundary length assertion costs four cycles; it is included in both
candidate counts. The identical full-hash reduction confirms that the change
is independent of program contents and rounding modes. Relative to the
post-correctness baseline at `3161c60`, accepted optimizations now save
6,252,429 cycles (0.093521837350%) on the real-block fixture.

Both guests exited zero and retained their exact official hashes:

```text
real:     043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
CFROUND:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
0592acfaeeef7dc315d713624a08a5f0f113933afdb6b3ef4d567aa94464c6c6  artifacts/randomx-real-unchecked-iteration-candidate     (281448 bytes)
8e1689b4a73c68ab7ded315228feb32b1f569e39affc8e55d8d5627b409d2403  artifacts/randomx-cfround-unchecked-iteration-candidate  (281496 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all five compact verifier tests pass, including malformed-scratchpad
  rejection, 1,024 opcode-boundary comparisons, and both fixed hashes;
- the fixed 20-block recent Monero mainnet test passes through the optimized
  `calculate_hash` path, with official hashes, rich state, complete
  scratchpads, block IDs, chain links, seed epoch, and difficulty checked;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
79bc8b1a37bc428ead31b3b8b760bbcd13f75651a2842dc1f3415229d209262e  compact/src/lib.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
