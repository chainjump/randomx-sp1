# Canonical RandomX v1.2.3 test port

Date: 2026-07-28 UTC

## Reference and scope

The reference is upstream RandomX tag `v1.2.3`, commit
`12f2c2ffe2108d6cf54c391fee33c8bc3646cdab`, specifically
`src/tests/tests.cpp`. Its `randomx-tests` program contains 95 numbered
checks. The Rust implementation passes all 84 checks that apply to its
portable interpreter architecture.

| Canonical area | Checks | Rust adaptation |
|---|---:|---|
| Argon2 cache initialization | 1 | Exact cache-word checkpoints |
| SuperscalarHash generator | 1 | Exact hashes of all ten generated programs |
| Portable reciprocal calculation | 1 | All seven exact constants |
| Interpreter dataset initialization | 1 | Exact words from four dataset items |
| AES Generator1R | 1 | Exact output vector |
| Instruction decode and execution | 71 | One-to-one assertions for every numbered check |
| Interpreter full hashes | 6 | Exact hashes through both rich and compact Rust VMs |
| Preserve caller rounding mode | 1 | Exact hash and native control-mode restoration |
| Commitment | 1 | Exact Blake2b-256 commitment |
| **Applicable total** | **84** | **All pass** |

The 71 instruction checks are grouped into one Rust test so that they can
share VM setup. A counter assertion requires exactly 71 successful adapted
checks; this prevents a removed case from silently reducing coverage. The six
full-hash vectors are likewise grouped because each RandomX key requires a
256 MiB cache initialization. These are 84 canonical logical checks rather
than 84 separate Cargo test processes.

## Configuration-specific exclusions

| Upstream check | Count | Reason it is not applicable |
|---|---:|---|
| `randomx_reciprocal_fast` | 1 | Alternate platform-specific implementation; Rust uses the tested portable calculation |
| Dataset initialization (compiler) | 1 | Exercises the canonical native JIT, which the SP1 interpreter does not implement |
| Compiler hash tests 2a-2f | 6 | Duplicate hash vectors routed through that native JIT |
| Cache initialization: SSSE3 | 1 | x86 SIMD-specific Argon2 backend absent from the portable guest |
| Cache initialization: AVX2 | 1 | x86 SIMD-specific Argon2 backend absent from the portable guest |
| Hash batch test | 1 | Pipeline API attached to the canonical native compiler/JIT |
| **Excluded total** | **11** | |

No algorithmic interpreter, cache, dataset, hash, rounding, or commitment
check is excluded. The exclusions test alternate native implementations or an
API built around the JIT rather than behavior missing from the Rust RandomX
hash path.

## Locations

- `rustdom-x/src/canonical_v1_tests.rs`: cache, dataset, superscalar,
  reciprocal, AES, and all 71 instruction checks.
- `compact/src/lib.rs`: six full hashes through both Rust VMs and preservation
  of the caller's native floating-point control mode.
- `audit/src/lib.rs`: commitment vector.

The rounding-preservation case initially exposed a native-host bug: a public
hash call left the final program rounding mode active. The native public entry
points now capture and restore the complete x86-64 MXCSR or AArch64 FPCR. This
code is target-gated out of the SP1 RISC-V guest, which uses software floating
point.

## Results

```text
cargo test --workspace --release --locked -- --test-threads=1
173 passed; 0 failed

cargo check --release --locked -p rustdom-x --no-default-features
passed

cargo check --release --locked -p rustdom-x \
  --no-default-features --features compact-superscalar
passed
```

The workspace total includes 138 tests from the in-tree `rust-argon2` fork:
136 inherited generic unit, integration, and documentation tests plus two
RandomX-specialization invariants. Those validate the modified primitive but
are separate from the 84 canonical RandomX checks above.

Rebuilding the SP1 guest after the port produced the retained artifact
byte-for-byte:

```text
size:    295352 bytes
sha256:  ac3eff37cbae4583f57cdbc193cca776a80672c77a63c09eb507dc35d154c317
```

Consequently the retained measurements remain 6,388,938,325 SP1 cycles and
7,744,392,823 PGU for Monero block 3,727,315.
