# Accepted uninitialized-cache allocation

Date: 2026-07-27 UTC

## Change

RandomX's fixed one-lane Argon2d first pass initializes blocks in increasing
order and can reference only earlier blocks. The new builder therefore:

1. allocates `Box<[MaybeUninit<Block>]>` without eagerly writing 256 MiB of
   zeroes;
2. initializes blocks 0 and 1 from Argon2's initial hash;
3. grows the initialized prefix one block at a time with the existing raw
   compressor;
4. converts the allocation to `Box<[Block]>` only after the first pass has
   initialized every block; and
5. runs passes 1 and 2 on the fully initialized allocation.

This is materially different from the previously rejected safe no-zero
candidate. That version constructed and moved a separate 1 KiB `Block` for
every first-pass iteration and regressed. This version writes each destination
in place through the already validated compressor.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Baseline | Candidate | Reduction |
|---|---:|---:|---:|
| Cache-only | 5,250,245,303 | 5,160,085,148 | 90,160,155 (1.717256048%) |
| Real block | 6,776,439,804 | 6,686,279,649 | 90,160,155 (1.330494443%) |
| CFROUND-heavy | 6,969,759,319 | 6,879,599,164 | 90,160,155 (1.293590652%) |

The identical delta in all three executions confirms that the change is
confined to cache construction.

```text
b5e24d1579dc702243df31cac6d5cf1d5f09d394253e09c35cd35a6da721ba7c  artifacts/randomx-cache-nozero-candidate
8628eed59a470499c3eaf53d6518be568d0ea0784a4fa918f07890cbb33a0cfc  artifacts/randomx-real-nozero-candidate
0e3afd519eb91718967b8725092f91bfe69b8f60555fc2af89c1ca0421c9ff22  artifacts/randomx-cfround-nozero-candidate
```

Candidate artifact sizes are 213,592, 291,392, and 291,440 bytes,
respectively.

Source fingerprints:

```text
a658e0503fc6246e5de0cc9a6dd6f748ea0faad230928b7f4d8ecdccf36f7e5a  argon2/src/core.rs
bd9ea6f1db1d55be3e4380979a9aa5a770ae045d8cab1cb66562012722b32eed  argon2/src/memory.rs
36892c71297e401fd8f83e7c86825f4bbd1be5546cc08e7f114a2d35b8ca652d  rustdom-x/src/memory.rs
133a760654a93015ac7ea8b52e871c6043ae8a79bb88e5b728cd55f4df4f27d2  Cargo.lock
```

The real-block output remains:

```text
043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
```

The CFROUND-heavy output remains:

```text
c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Both guests exited with code zero.

## Initialization and aliasing invariants

- The allocation has exactly 262,144 aligned `Block` slots.
- Blocks 0 and 1 are written before the specialized first pass begins.
- First-pass destinations proceed monotonically from block 2 through block
  262,143 with no gaps or duplicates.
- In one-lane Argon2d's first pass, both the previous and reference offsets are
  strictly less than the current offset. Existing boundary tests compare the
  specialized formula with generic Argon2 and assert this property; debug
  assertions also guard it at the raw access site.
- `fill_block_raw::<false>` never reads the destination and writes all 128
  words during its column loop before its row loop reads them. The existing
  partition test verifies that the eight 16-word column ranges cover exactly
  `0..128`.
- The `assume_init` conversion occurs only after the complete first pass.
- Later passes operate on initialized blocks, and their reference formula
  excludes the current mutable block. Previous and reference inputs may alias
  each other, but both are shared reads.
- If initialization panics before conversion, dropping
  `Box<[MaybeUninit<Block>]>` does not inspect uninitialized elements.

## Correctness evidence

- 61 Argon2 unit tests, 21 integration tests, and 12 documentation tests pass.
- All three complete 256 MiB cache digests match both frozen expected values
  and the crates.io generic Argon2 implementation:

```text
selected Monero seed  152add6ff4fd241ba703f004dcea77fea6c2d55d8b20100aae1578e7bca88a5c
32 zero bytes          f303edc0c3dc803869f25bb11178193805d767427e11f519bb2ac123ea1ef63e
empty key              faf16925e389d546a2ebf79d1329ed4f8f217902ba00a5641447773725306d15
```

- All three compact-VM release tests pass, including the official
  CFROUND-heavy and real-block vectors.
- Both actual SP1 RV64IM full guests return their exact expected hashes.

No cache, cache commitment, or transcript was added as input. The complete
cache is still derived inside the guest from the key.
