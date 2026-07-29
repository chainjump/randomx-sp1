# Official RandomX v1.2.3 differential corpus

Date: 2026-07-28 UTC

## Independent reference

The comparison used a clean local checkout of upstream RandomX:

```text
tag:                 v1.2.3
commit:              12f2c2ffe2108d6cf54c391fee33c8bc3646cdab
librandomx.a SHA-256: 6fa9e2ba6c8cf51a440da2da219445ca9f705403db9151bf10a97fea421e4443
```

All 95 upstream `randomx-tests` checks passed before the library was used as
the oracle. The opt-in audit binary links that library only when the
`official-randomx` feature is enabled; ordinary builds remain self-contained.

## Complete light-mode corpus

The corpus crosses seven key shapes with six blob shapes, for 42 complete
light-mode hashes:

- keys: empty, one byte, `test key 000`, 32 zero bytes, a Monero epoch key, a
  deterministic 64-byte key, and a deterministic 257-byte key;
- blobs: empty, one byte, 26-byte text, and deterministic 76-, 257-, and
  4,096-byte values.

For every pair, official RandomX v1.2.3, the internal reference interpreter,
and the optimized interpreter returned the same 32-byte hash. Both internal
implementations also had identical final register bytes and complete 2 MiB
scratchpads.

RandomX uses the entire key for Argon2 cache initialization and the first 60
bytes for superscalar-program generation. The implementation and directed
tests enforce both rules for arbitrary key lengths.

Run one key shard with:

```text
env RANDOMX_LIB_DIR=/path/to/RandomX/build \
  cargo run --release --locked --offline \
  -p randomx-sp1-audit --features official-randomx \
  --bin official_randomx -- <key-name>
```

## Complete-cache differential

The optimized Argon2 path was compared with the generic crates.io Argon2
implementation over all 262,144 blocks (256 MiB) for five keys. All digests
matched:

```text
selected Monero seed  152add6ff4fd241ba703f004dcea77fea6c2d55d8b20100aae1578e7bca88a5c
32 zero bytes          f303edc0c3dc803869f25bb11178193805d767427e11f519bb2ac123ea1ef63e
empty key              faf16925e389d546a2ebf79d1329ed4f8f217902ba00a5641447773725306d15
64-byte pattern        d5faea3e30c30e04a8d7ef7f997931b58e24bdbc2aeb4a8d898bfed612614392
257-byte pattern       6361c02873ca5b04e939b6bd3b2e0cba81122fd152a8c6f2794f96cea5849948
```

## Decoder and arithmetic coverage

- Every raw opcode byte (`0x00..0xff`) is executed with four operand patterns:
  1,024 reference/optimized instruction-state comparisons.
- The lockstep audit covers 32 hashes, 256 generated programs, and 524,288 VM
  iterations, comparing registers after every instruction and scratchpads
  after every program.
- The software-rounding audit covers 20,000 randomized inputs per operation
  and mode, including directed-overflow boundaries.
- Twenty consecutive real Monero blocks match official RandomX and exercise
  all supported floating-point rounding modes.
- All Argon2d/i/id version 1.0/1.3 integration vectors, unit tests, and
  documentation tests pass.

This is broad differential evidence, not a formal proof over every possible
key, blob, program, or architecture. Independent official hashes and generic
Argon2 cache comparisons remain essential because the reference and optimized Rust
implementations share some constants and helpers.
