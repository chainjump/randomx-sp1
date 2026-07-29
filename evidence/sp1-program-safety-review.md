# SP1 program safety review

Review date: 2026-07-29 UTC

## Result

No known SP1-unsupported or proof-unsound behavior is reachable from the
RandomX guest entry point. The guest uses ordinary deterministic Rust plus the
documented SP1 input and public-output APIs. It does not directly invoke an
SP1 syscall, precompile, unconstrained block, host service, thread, clock, or
random-number source.

One unnecessary source-level undefined-behavior hazard was found during this
review. The statically dispatched superscalar interpreter used
`unreachable_unchecked` for an opcode value that its private constructor
restricts to `0..=9`. It now uses a defined trapping `unreachable!` fallback,
so a future bad table entry cannot turn that invariant violation into Rust
undefined behavior. The complete release suite exercises the table and its
construction invariants.

This is a code review and test record, not a formal verification claim.

## Build status

This review covers the source and dependency graph. Release-facing cleanup
invalidated the previous ELF and vkey, so reproducible build, disassembly,
execution, and proof review must be repeated after the final source is
approved.

## SP1 security guidance applied

The review used the following current Succinct material:

- [SP1 security model](https://docs.succinct.xyz/docs/sp1/security/security-model),
  particularly its requirements that the ELF be untampered, the guest be
  nonmalicious and memory-safe, unsafe Rust not invoke undefined behavior, and
  program counter zero not be a valid execution state;
- [safe precompile usage](https://docs.succinct.xyz/docs/sp1/security/safe-precompile-usage),
  including the warnings against direct `ecall` use and the pointer,
  canonical-field, curve-point, and `U256` preconditions;
- [compiling programs](https://docs.succinct.xyz/docs/sp1/writing-programs/compiling),
  including the production recommendation for a reproducible Docker build;
  and
- [SP1 security advisories](https://github.com/succinctlabs/sp1/security/advisories).

All locked SP1 crates are version 6.3.1. In particular, this is newer than the
6.1.0 fix for
[GHSA-63x8-x938-vx33](https://github.com/succinctlabs/sp1/security/advisories/GHSA-63x8-x938-vx33),
which affected SP1 6.0.0 through 6.0.2.

## Reachable guest behavior

`program/src/main.rs` has one straight-line entry point:

1. read a byte-vector RandomX key with `sp1_zkvm::io::read_vec`;
2. construct the fixed 256 MiB RandomX Argon2d cache and eight superscalar
   programs from that runtime key;
3. read a byte-vector hashing blob with `sp1_zkvm::io::read_vec`;
4. execute the eight RandomX VM programs in light mode; and
5. commit the 32-byte hash with `sp1_zkvm::io::commit_slice`.

The key and blob are prover inputs and are not public values in this standalone
guest. Consequently, its standalone statement exposes only the resulting
hash. A containing application must separately bind or expose the key and blob
when its intended statement is specifically about a public block. This is a
statement-design property, not an SP1 soundness violation.

For finite inputs, all RandomX and cache loops are bounded. Very large input
vectors can exhaust memory or exceed a network execution limit; that produces
a failed execution rather than an accepting proof. A caller accepting
untrusted input should impose application-level length limits for availability
and cost control.

### Syscalls and precompiles

The guest and its RandomX crates contain no direct `ecall`, custom SP1 syscall,
or unconstrained-block call. The application uses only the documented
`read_vec`, `commit_slice`, and normal entry-point return path supplied by the
SP1 6.3.1 runtime. The final release ELF must be inspected to confirm that
direct `ecall` instructions remain confined to linked SP1 runtime functions,
that `_start` is nonzero, and that no loadable section maps address zero.

No elliptic-curve, field, `U256`, hashing, or other accelerated SP1 precompile
is called by the custom guest. The precompile-specific canonical-value,
on-curve, edge-case, and pointer requirements therefore do not apply. Native
x86-64/AArch64 AES and floating-point implementations are target-gated out of
the RISC-V build; the guest uses software AES and the reviewed integer-based
binary64 implementation.

### Determinism and host interaction

The reachable path has no filesystem, network, environment, subprocess,
thread, random-number, or wall-clock access. An `Instant`-based performance
helper and full-dataset allocator remain in non-guest-reachable library code
and are target-gated out of the guest path. The guest creates `VmMemory` in light
mode; the optimized execution path never takes the optional `RwLock`-backed
full-dataset cache path.

## Unsafe Rust review

The performance-sensitive implementation intentionally contains unsafe Rust.
The following reachable categories and their invariants were reviewed.

### Argon2 cache construction

- The uninitialized allocation has exactly 262,144 `Block`s (256 MiB), with
  RandomX-fixed one-lane Argon2d v1.3 parameters checked at entry.
- Initial blocks 0 and 1 are written first. The first pass visits every
  remaining block in increasing order and refers only to initialized earlier
  blocks. Two later XOR passes operate only on initialized memory.
- `fill_block_raw` writes all 128 words before any destination block or local
  `MaybeUninit` array is assumed initialized.
- Segment, previous-block, and reference-block indices are derived from the
  fixed RandomX parameters and stay within the allocation.

### Cache and block views

- `Block` is transparent over `[u64; 128]`; compile-time size and alignment
  assertions guard byte and flat-`u64` views.
- `SeedMemory` fields are private. The only state with superscalar programs
  also has the exact fixed cache allocation, so masked cache-line indices and
  their eight word offsets remain in bounds.

### Optimized RandomX interpreter

- `CompactProgram` and decoded instruction fields are private. Decoding masks
  or reduces register indices, validates byte offsets, and installs exactly
  256 instructions.
- Branch targets refer to an earlier instruction or `-1`; incrementing the
  program counter before the next fetch keeps every unchecked fetch in
  `0..256`.
- Scratchpad masks enforce eight-byte alignment and keep every unchecked word
  access within the fixed scratchpad. Entry checks reject malformed lengths.
- The rounding-mode value is private and limited to `0..=3` before the
  validated enum conversion. All four modes, including program-generated
  rounding-mode changes, are exercised by canonical vectors.

### Superscalar dispatcher

- Private decoding restricts source and destination register offsets to the
  eight-word register array.
- Ten opcode classes generate a fixed 100-entry pair-handler table. The table
  index is formed only from those validated classes.
- The formerly unchecked impossible fallback now traps with defined behavior.

Panics, failed assertions, allocation failure, or an execution-limit breach do
not create an accepting execution; they are liveness failures. No reviewed
unsafe operation relies on such a failure being ignored.

## Validation performed

The source hardening was followed by:

```text
cargo test --workspace --release --locked -- --test-threads=1
result: 174 passed; 0 failed
```

This includes the canonical RandomX cache/dataset, decoder, reciprocal,
superscalar, interpreter, rounding-mode, full-hash, and Argon2 conformance
tests. It also includes 20 recent Monero block fixtures and all 100 paired
superscalar handler combinations.

`cargo audit` loaded 1,172 current RustSec advisories and found zero known
vulnerabilities among 374 locked dependencies. It reported five allowed
informational unmaintained warnings (`ansi_term`, `bincode`, `number_prefix`,
`paste`, and `proc-macro-error2`). Of these, only `bincode` is in the guest
dependency tree, through SP1 6.3.1; this guest reads raw vectors rather than
deserializing attacker-chosen bincode values.

## Remaining limitations

- A MemorySanitizer attempt was not valid because the instrumented crates
  would have linked against an uninstrumented Rust standard library. It failed
  before the tests and is not counted as evidence.
- Reproducible Docker build, disassembly review, guest execution, local vkey
  derivation, and live proof validation remain pending for the final source.
- Tests, sanitizer runs, dependency scanning, and manual invariant review
  substantially reduce risk but do not prove the absence of every memory
  safety or logic defect.
