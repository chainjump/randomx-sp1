# Accepted byte-sized rounding mode

Date: 2026-07-28 UTC

Baseline checkpoint: `7abc258` (`perf: trust validated RandomX rounding
modes`)

## Change

The private VM rounding field could contain only four values but still used a
`u32`. Converting it to the `repr(u8)` `RoundingMode` required an RV64IM
word-load followed by byte zero-extension in every floating-point effect.

The field now uses `u8`. The checked public setter still accepts `u32`, rejects
values outside `0..=3`, and narrows only after that assertion. The public
getter still returns `u32`, so this is an internal representation change with
no API or RandomX-state semantic change.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,664,941,704 | 6,664,284,743 | 656,961 (0.009856965435%) |
| Rounding regression | 6,860,314,853 | 6,859,655,768 | 659,085 (0.009607212120%) |

Relative to the post-correctness baseline at `3161c60`, accepted
optimizations now save 21,243,661 cycles (0.317755900750%) on the real-block
fixture and 19,123,923 cycles (0.278013308451%) on the rounding regression.

Both guests exited zero and retained their exact official hashes:

```text
real:                 043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
rounding regression:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
e8b562b1c004ced13c158ccd9f281076a9c2894c08305174911fc4e12ca259dc  artifacts/randomx-real-u8-rounding-candidate     (281880 bytes)
758ec68f94d67bea6b50a183ee4a5f2bb29630124b845f1cfa809a423c0a27f3  artifacts/randomx-cfround-u8-rounding-candidate  (281928 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all six compact verifier tests pass, including invalid-mode rejection,
  opcode boundaries, and both complete fixed hashes;
- hardware and software arithmetic agree for 20,000 cases per operation and
  mode;
- the lockstep audit passes all 32 hashes, 256 programs, 524,288 iteration
  states, and every executed instruction state;
- all 20 fixed recent Monero mainnet blocks pass with ordinary CFROUND across
  all modes, official hashes, rich state, complete scratchpads, block IDs,
  chain links, seed epoch, and difficulty;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
35ab9ebc8a34145f0234e226b262cb13e9f3fe4384e9230cdd2ee50480f1c841  rustdom-x/src/vm.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
