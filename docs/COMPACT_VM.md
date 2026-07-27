# Compact RandomX VM experiment

This isolated experiment flattens Rustdom's boxed `Instr` / `Store` / `Mode`
representation without changing the proved RandomX statement. The SP1 guest
still derives the Argon2 cache from the epoch seed, derives every requested
dataset item, generates all eight AES instruction streams, runs all 2,048 VM
iterations per stream, checks the selected Monero block hash, and commits the
same 32 bytes.

## Generic CFROUND update

The compact VM now implements all four RandomX rounding modes on SP1 RV64IM
for `FADD`, `FSUB`, `FMUL`, `FDIV`, and `FSQRT`. The current same-input result
also includes compile-time specialization of RandomX's fixed Argon2 passes and
slices, raw cache access, a hoisted cache pointer, and omission of the lane
mask only in specialized cases whose reference position cannot wrap:

| Same CFROUND-heavy input | SP1 cycles | Hash |
|---|---:|---|
| Nearest-only negative control | 6,729,191,786 | `fbd2e95d…8e698b` (wrong) |
| Exact four-mode implementation | 6,969,759,319 | `c19ae2f2…0fda95` |

The exact incremental cost is 240,567,533 cycles (3.574984%). Official
RandomX v1.2.3 executes 18,442 `CFROUND`s for this input, distributed across
nearest/down/up/toward-zero as 4738/4852/4325/4527. The exact host VM and the
actual SP1 guest both return the official hash. The latest frozen outputs,
complete-cache differential, rejected-candidate result, source hash, and ELF
hashes are in `artifacts/argon-nowrap-specialization.txt`; earlier results
remain frozen in `artifacts/argon-pass-specialization.txt` and
`artifacts/cfround-execution.txt`.

Phase probes against that prior baseline attributed 5,276,459,999 cycles
(75.421378%) to cache construction and 597,809,633 cycles (8.545052%) to a
full VM hash with deterministic no-memory dataset values. The residual
1,121,704,383 cycles (16.033570%) is a useful estimate of on-demand dataset
work, but is not an exact subtraction because the no-memory execution follows
a different state trajectory. Pass specialization first reduced the cache
probe to 5,263,614,934 cycles and the exact full hash to 6,983,128,950. Slice
specialization, pointer hoisting, and the statically non-wrapping reference
optimization reduce the current cache probe to 5,250,245,303 cycles and the
exact full hash to 6,969,759,319. The final non-wrapping change alone saves
327,687 cycles in both the cache probe and full guest. It preserves the
complete 256 MiB derivation; three full-cache digests match the generic
implementation.

The experiment depends on the current concrete-memory `vendor/rustdom-x` but
does not edit `program-full` or the vendor directory.

## Result

| Variant | SP1 cycles | Change |
|---|---:|---:|
| Markerless concrete-memory rich VM control | 8,331,891,851 | — |
| Markerless 24-byte compact VM instruction | 8,272,019,127 | -59,872,724 (-0.718597%) |

The cycle speedup is 1.007238x. The compact ELF returned the exact expected
hash `043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000`
with guest exit code zero. Its wall time was 15.33 seconds on this host.

## Representation

Each executable instruction is exactly 24 bytes:

- an efficient function pointer, retained because the exhaustive direct-opcode
  match was already measured and rejected;
- one preprocessed `u64` immediate (also used for reciprocals and branch
  increments);
- a precomputed branch target;
- byte-sized destination, source, and shift/condition/scratchpad modes.

Decode precomputes register indices, sign extension, immediate-vs-register
effects, `IMUL_RCP` reciprocals, memory levels, branch increments and targets,
and floating-register banks. The hot loop therefore has no boxed stores,
`Option` unwraps, or rich-enum register matches.

## Validation

- A native differential audit compared rich and compact execution for 32 full
  hashes with `VmMemory::no_memory()`'s deterministic dataset initialization:
  256 generated programs and
  524,288 evolving VM iteration states. Final hashes, all register bytes, and
  the complete 2 MiB scratchpads matched exactly.
- The exact full SP1 guest derived the real cache and dataset, produced the
  expected Monero PoW hash, and exited successfully.
- Locked offline workspace checking and focused unit tests pass.
- Every command was hard-capped below 60 seconds.

## Safety and semantic risks

The executor uses validated unchecked array access. Its private decoder reduces
integer register indices modulo 8 and floating indices modulo 4; programs are
always 256 instructions; decoded branch targets are in `-1..255` and become a
valid next fetch after the loop increment; scratchpad masks confine indexes to
the fixed 262,144-word allocation. Violating any of those construction
invariants would make the unchecked accesses unsafe, so an integrated version
should keep the decoder and executable fields private and preserve assertions
at the decode boundary.

The compact decoder currently duplicates RandomX opcode thresholds and several
VM constants from Rustdom. That is reviewable but creates protocol-drift risk;
production integration should place it beside the rich decoder and share
constants/tests rather than retain this wrapper architecture.

The 32-hash differential covers many generated programs and dynamic states but
is not a mathematical proof of decoder equivalence for every possible 64-bit
instruction word. The exact selected-block SP1 execution is an additional
end-to-end check. A production merge should also add exhaustive opcode-boundary
tests and property-based one-instruction comparisons.

The original selected fixed block has zero dynamic `CFROUND` executions, as
established by the separate audit. Generic inputs use the new integer-assisted
software rounding path; the nearest-only audit feature is a negative control
and must never be enabled in a verifier.

## Frozen evidence

- Markerless concrete-memory compact ELF:
  `artifacts/randomx-compact-vm-markerless-program`
  (`11189022f0e4872cf173ce572b86869071a0023bcf422a887463f8d02345eb43`)
- Compact executor source: `compact/src/lib.rs`
  (`230fc50bb1e18023c34886e9994bb68992688f1b1c8af812358d04c0471b4e95`)

## Minimal integration patch

The measured wrapper architecture can be integrated without changing the
vendor source. Add this dependency to `program-full/Cargo.toml`:

```toml
rustdom-x-compact-vm = { path = "../optimization-vm-compact/compact" }
```

Then import `rustdom_x_compact_vm::calculate_hash` in
`program-full/src/main.rs` and replace:

```rust
let hash = vm.calculate_hash(&HASHING_BLOB);
```

with:

```rust
let hash = calculate_hash(&mut vm, &HASHING_BLOB);
```

No root-workspace member is needed for a path dependency. Regenerate
`Cargo.lock`, then require `--locked` for all subsequent builds. The isolated
`program` package already uses exactly this dependency/import/call layout and
passes locked checking plus the end-to-end SP1 execution.

For a cleaner long-term merge, move `compact/src/lib.rs` into Rustdom as a
`compact_vm` module, change its `rustdom_x::...` imports to `super::...`, export
the module from `vendor/rustdom-x/src/lib.rs`, and make the same one-line call
change in `program-full`. That avoids a second package while sharing Rustdom's
existing `blake2b_simd` dependency.
