# CFROUND host-oracle and directed-overflow correction

Date: 2026-07-27 UTC

## Correctness findings

Two pre-existing defects were exposed while strengthening the rich/compact VM
differential audit. Neither was introduced by the uninitialized-cache
optimization in commit `68938af`.

1. The x86-64 rich VM passed x87 control-word encodings
   (`0x400/0x800/0xc00`) through an MXCSR `0x6000` mask. The masked values were
   all zero, so the host reference silently stayed in nearest-even mode for
   every `CFROUND`. MXCSR rounding control actually uses
   `0x2000/0x4000/0x6000` for down/up/toward-zero. This defect affected the
   native audit oracle, not the RV64IM guest's software-rounding path.
2. The compact guest's directed binary64 implementation assumed finite-input
   overflow was unreachable. In the deterministic `VmMemory::no_memory()`
   audit trajectory, program 0, iteration 59, PC 199 (`FDIV_M`) diverged in
   toward-zero mode: the corrected hardware reference produced maximum finite
   while the software path retained infinity. After that was corrected, an
   infinity propagated to program 1, iteration 940, PC 254 (`FSQRT_R`) in
   round-up mode; attempting to move one ULP above positive infinity produced
   a NaN in the old software path.

The software implementation now handles finite overflow before attempting its
ordinary exact-neighbor comparison and propagates infinities created by prior
RandomX operations. The synthetic no-memory trajectory proves the code path is
reachable in the generic VM state machine. It does not, by itself, prove that
the same trajectory occurs for an official cache/key pair, so that narrower
reachability claim is intentionally not made.

## Independent arithmetic checks

Every command was bounded by
`timeout --signal=INT --kill-after=1s 55s`.

- Six release-mode `randomx-softfp` tests pass. Berkeley SoftFloat checks
  20,000 deterministic cases per operation and mode, explicit positive and
  negative overflow for add/subtract/multiply/divide, infinity propagation,
  signed zero, exact cancellation, and the official RandomX vectors.
- `hardware_rounding` sets each corrected x86 MXCSR mode and compares packed
  x86 arithmetic with the software implementation for 20,000 randomized
  inputs per operation and mode, plus directed-overflow boundaries. It exits
  successfully with:

```text
hardware/software agreement: 20000 cases per operation and mode
```

## Full VM audit

The audit feature now runs the rich and compact decoders in lockstep and
compares register state after every executed instruction, not only at VM
iteration boundaries. It also compares complete scratchpads after every
generated program. The corrected build reports:

```text
rich/compact agreement: 32 hashes, 256 generated programs, 524288 VM iteration states, and every executed instruction state
```

All three compact-VM release tests pass. In particular, the x86 rich VM and
the compact VM now both match the official directed-CFROUND hash; the stale
test that treated the broken rich VM as a nearest-only control was replaced by
this positive three-way check.

## SP1 end-to-end A/B

Both RV64IM guests retain the expected public hash and exit code zero. The
extra overflow guards also avoid unnecessary exact-arithmetic work once an
infinity occurs, producing a small cycle reduction relative to commit
`68938af`.

| Fixture | Before fix | Corrected | Reduction |
|---|---:|---:|---:|
| Real block | 6,686,279,649 | 6,685,528,404 | 751,245 (0.011235620%) |
| CFROUND-heavy | 6,879,599,164 | 6,878,779,691 | 819,473 (0.011911639%) |

The old artifacts were re-executed with the same runner and reproduced their
frozen cycle counts exactly.

```text
87b996c6bf96b5d94977dc53356bf89876d6f53f7c53dde7494e93bf8f007c46  artifacts/randomx-real-overflow-fixed
f068d4b32f4d107a5eeb3e006d852b5d4d7704d910dc64ab58904b0460585ca1  artifacts/randomx-cfround-overflow-fixed
```

Artifact sizes are 291,928 and 291,976 bytes, respectively. The committed
outputs remain:

```text
real block:      043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
CFROUND-heavy:   c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Source and lockfile fingerprints:

```text
a19b507931a0fe76f78d2db75930dc5d6daa50a7b2ce2289960edee4dc5a4fca  rustdom-x/src/vm.rs
92d54589e5b9579f093b686f8651edf88507aac754f307119263d89a4812fc2d  softfp/src/lib.rs
6a73e5602da0d7491edb0fb7855808395c28038338e892ca908cffce57ebae6f  compact/src/lib.rs
36497a524e81e85712f19f3ec68277ab68f9135aeb2f3edc77ee4ed5fb73dfa2  audit/src/main.rs
428984d6fc2967119bd366f9bbfdd813372f3612399d3562087a903d9d4de55c  audit/src/bin/hardware_rounding.rs
472fda39b80e7abb47a5f4115ae542d5e7c69a30c5866a454267fe453d44e5f8  Cargo.lock
```

No proof or paid proving-network request was made.
