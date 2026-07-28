# Official RandomX v1.2.3 corpus and long-key correction

Date: 2026-07-28 UTC

## Correctness bug found

The native official-RandomX differential found a pre-existing arbitrary-key
bug. `Blake2Generator::new` allocated a 60-byte staging array and copied
`seed.len()` bytes into it. Keys longer than 60 bytes therefore panicked (or
hit the preceding debug assertion) while official RandomX accepts them.

The official implementation has two deliberately different key rules:

- Argon2 cache initialization consumes the complete key.
- Superscalar-program generation copies `min(key_size, 60)` bytes into its
  64-byte Blake2 generator state and appends a four-byte nonce.

Rustdom now implements that same truncation only in the superscalar generator;
its Argon2 context still receives the complete key. A unit test compares 256
generated `u32`s from a 256-byte seed with its first 60 bytes. The sequences
match exactly.

## Monero-network applicability

The selected real-network regression block uses Monero's normal 32-byte epoch
key and executes no dynamic `CFROUND`; it matched before and after these fixes.
The long-key panic therefore cannot occur in ordinary Monero protocol inputs.
It is a defect in the explicitly required generic arbitrary-key API.

The earlier x86 MXCSR mapping defect can affect other real Monero inputs whose
generated programs execute `CFROUND`; the selected block simply did not cover
that opcode. The directed-overflow defect was observed on the synthetic
no-memory trajectory, not on a real-cache Monero trajectory, so no claim of an
observed network mismatch is made for that edge case.

## Independent reference

The comparison used a clean local checkout of upstream RandomX:

```text
tag:                 v1.2.3
commit:              12f2c2ffe2108d6cf54c391fee33c8bc3646cdab
librandomx.a SHA-256: 6fa9e2ba6c8cf51a440da2da219445ca9f705403db9151bf10a97fea421e4443
```

All 95 upstream `randomx-tests` checks passed before the library was used as
the oracle. The opt-in audit binary links that library only when the
`official-randomx` feature is enabled, so ordinary workspace builds remain
self-contained and offline.

## Complete light-mode corpus

The corpus crosses seven key shapes with six blob shapes, for 42 complete
light-mode hashes:

- keys: empty, one byte, `test key 000`, 32 zero bytes, the selected Monero
  seed, a deterministic 64-byte key, and a deterministic 257-byte key;
- blobs: empty, one byte, 26-byte text, and deterministic 76-, 257-, and
  4,096-byte values.

For every pair, official RandomX v1.2.3, Rustdom's rich VM, and the compact VM
returned the same 32-byte hash. The rich and compact implementations also had
identical final register bytes and complete 2 MiB scratchpads. The seven
one-key shards each finished in about 17 seconds and used this bounded form:

```text
timeout --signal=INT --kill-after=1s 55s \
  env RANDOMX_LIB_DIR=/tmp/randomx-v1.2.3-research/build \
  cargo run --release --locked --offline \
  -p randomx-compact-vm-audit --features official-randomx \
  --bin official_randomx -- <key-name>
```

Sharding is intentional: all 42 comparisons in one command exceed the
mandatory 55-second limit.

## Complete-cache differential

The specialized no-zero Argon2 path was also compared with the generic
crates.io Argon2 implementation over all 262,144 blocks (256 MiB) for five
keys. All digests matched:

```text
selected Monero seed  152add6ff4fd241ba703f004dcea77fea6c2d55d8b20100aae1578e7bca88a5c
32 zero bytes          f303edc0c3dc803869f25bb11178193805d767427e11f519bb2ac123ea1ef63e
empty key              faf16925e389d546a2ebf79d1329ed4f8f217902ba00a5641447773725306d15
64-byte pattern        d5faea3e30c30e04a8d7ef7f997931b58e24bdbc2aeb4a8d898bfed612614392
257-byte pattern       6361c02873ca5b04e939b6bd3b2e0cba81122fd152a8c6f2794f96cea5849948
```

## Decoder and arithmetic coverage

- A new boundary test executes every raw opcode byte (`0x00..0xff`) using
  four operand patterns covering equal/different registers, immediate
  extremes, all rounding modes, memory levels, and branch/store conditions:
  1,024 rich/compact instruction-state comparisons.
- The existing lockstep audit still passes 32 hashes, 256 generated programs,
  524,288 VM iterations, and compares registers after every executed
  instruction plus scratchpads after every program.
- The x86/software rounding audit still passes 20,000 randomized inputs per
  operation and mode, including directed-overflow boundaries.
- Six sharded groups cover all 54 Argon2d/i/id version 1.0/1.3 integration
  vectors; the remaining 11 integration checks, 61 unit tests, and 12
  documentation tests pass.

## Limits

This evidence removes a known arbitrary-key failure and provides an
independent end-to-end oracle across substantially more inputs. It is not a
formal proof over every possible key, blob, generated program, or machine
architecture. The rich and compact implementations also share some constants
and helpers, which is why the independent official hashes and generic Argon2
cache comparison remain essential.

Source fingerprints for this checkpoint:

```text
bfb661071a9359de0bf5aa2805256bc1ebf4187074339af07e5a3a5ad85888b0  rustdom-x/src/superscalar.rs
398ee384db24fda3bf82b410f9dcee36bb75d1636e5ac75fa069309bf5c04fb0  compact/src/lib.rs
d944481ccdfec9b4e92215efec2c120d6eaac50c86369f8f3b4d218ea725f640  audit/src/bin/official_randomx.rs
e2fedf07872048f8972754f6dcaa54336dabb8a6c594015c060d4a025b35133e  audit/build.rs
7da706ea021d31e08521d896d22fcbee473dd29eaff91f234295dab5b6ef31f4  argon2-native-compare/src/main.rs
472fda39b80e7abb47a5f4115ae542d5e7c69a30c5866a454267fe453d44e5f8  Cargo.lock
```

No proof or paid proving-network request was made.
