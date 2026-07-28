# Accepted validated rounding-mode enum conversion

Date: 2026-07-28 UTC

Baseline checkpoint: `738e9e1` (`perf: store RandomX rounding mode directly`)

## Change and safety boundary

After storing the logical rounding mode directly, each compact FP effect still
called the general `RoundingMode::from_fprc`. That public conversion masks an
arbitrary integer and emits a branch tree to construct the enum, even though
`Vm` can contain only values zero through three.

`RoundingMode::from_valid_fprc` now provides an explicitly unsafe conversion
for a prevalidated value. The compact verifier uses it only after reading the
VM's private rounding field. The constructor and reset path write zero, and
the public setter now uses a release-mode assertion to reject values above
three before storing them or changing the host environment. Directed tests
verify all four checked/validated conversions and rejection of an invalid
setter input. Thus no safe caller can create an invalid enum discriminant.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,669,574,950 | 6,664,941,704 | 4,633,246 (0.069468384938%) |
| Rounding regression | 6,862,777,494 | 6,860,314,853 | 2,462,641 (0.035884028036%) |

The differing reductions reflect the fixtures' different instruction and
rounding-mode trajectories. Relative to the post-correctness baseline at
`3161c60`, accepted optimizations now save 20,586,700 cycles
(0.307929287798%) on the real-block fixture and 18,464,838 cycles
(0.268431885152%) on the rounding regression.

Both guests exited zero and retained their exact official hashes:

```text
real:                 043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
rounding regression:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
17b039c5d4db5a96304c8c6c1f1f6c47562030f962abdd926716d3fc1dbd1854  artifacts/randomx-real-direct-rounding-enum-candidate     (281896 bytes)
8b12977828280db6e68064d97d8e533c02901eebf340cb4616261052ee07f248  artifacts/randomx-cfround-direct-rounding-enum-candidate  (281944 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all seven software-FP tests pass, including every validated enum conversion
  and the Berkeley SoftFloat comparisons;
- all six compact verifier tests pass, including invalid-mode rejection,
  opcode boundaries, and both complete fixed hashes;
- hardware and software arithmetic agree for 20,000 cases per operation and
  rounding mode, including directed boundaries;
- the lockstep audit passes 32 hashes, 256 programs, 524,288 VM iteration
  states, and every executed instruction state and rounding mode;
- all 20 fixed recent Monero mainnet blocks pass with 381,809 ordinary
  CFROUND executions, official hashes, rich state, complete scratchpads,
  block IDs, chain links, seed epoch, and difficulty;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
e207a11200adb363d0f8606cecc1d1e59da410a0501c784b49563dc6e2bb0f9e  compact/src/lib.rs
04a90ed18167e652a0a04cce44d9d7f78ae834f353e4ed775231d300a708bf94  rustdom-x/src/vm.rs
d1ada78d39c515f2cde662b3687ad6100759bc0d04f22155b025d50810c1154c  softfp/src/lib.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
