# Accepted direct logical rounding mode

Date: 2026-07-28 UTC

Baseline checkpoint: `ac07524` (`perf: align compact instructions to
power-of-two stride`)

## Change

`Vm` stored its logical RandomX rounding mode as an MXCSR-shaped control word.
Every floating-point instruction then loaded that word, shifted by 13, and
masked it back to a value in `0..=3`. On non-x86 guests this encoding had no
purpose; even on x86 it was separate from the actual `stmxcsr`/`ldmxcsr`
environment update.

The private VM field now stores the logical mode directly. Reset writes zero,
`CFROUND` writes its already-masked two-bit result, and `get_rounding_mode`
returns the field without extraction. The existing x86 and aarch64 hardware
environment update is unchanged. A debug assertion constrains all setter
inputs to the four valid modes.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,670,983,161 | 6,669,574,950 | 1,408,211 (0.021109497146%) |
| Rounding regression | 6,864,234,897 | 6,862,777,494 | 1,457,403 (0.021231834602%) |

The primary fixture remains in nearest mode and saves the two extraction
instructions on each FP operation. The rounding regression additionally
saves work on 18,442 ordinary `CFROUND` updates. Relative to the
post-correctness baseline at `3161c60`, accepted optimizations now save
15,953,454 cycles (0.238626672956%) on the real-block fixture and 16,002,197
cycles (0.232631334609%) on the rounding regression.

Both guests exited zero and retained their exact official hashes:

```text
real:                 043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
rounding regression:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
3f49cc15d9813fd9abbd80272403bed2b5482d8bb8ce1cf5afb70575d69ebb43  artifacts/randomx-real-direct-rounding-candidate     (282104 bytes)
54350735c4b3b3427fb764bf1c43e315841fc336581aac771b89d9e626e7f79a  artifacts/randomx-cfround-direct-rounding-candidate  (282152 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all five compact verifier tests pass, including all opcode boundaries and
  both complete fixed hashes;
- hardware and software arithmetic agree for 20,000 cases per operation and
  rounding mode, including directed boundaries;
- the lockstep audit passes 32 hashes, 256 programs, 524,288 VM iteration
  states, and every executed instruction state and rounding mode;
- all 20 fixed recent Monero mainnet blocks pass with 381,809 ordinary
  CFROUND executions across all modes, official hashes, rich state, complete
  scratchpads, block IDs, chain links, seed epoch, and difficulty;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
7a063f4b338caddee7e252f6b1d220c22e0e75e709eefc06e048fe023ca86aa1  rustdom-x/src/vm.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
