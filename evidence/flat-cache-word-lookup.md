# Accepted flattened cache-word lookup

Date: 2026-07-28 UTC

Baseline checkpoint: `3161c60` (`test: freeze recent Monero RandomX blocks`)

## Change

Each dataset item performs 64 reads from RandomX's 256 MiB Argon2 cache. The
old helper converted every selected cache-line byte offset into a block index
and then a word-within-block index, with two Rust slice-indexing operations.
The accepted helper treats the contiguous cache allocation as its underlying
flat array of `u64` words and computes the equivalent index directly:

```text
word = ((register & (cache_line_count - 1)) * 8) + lane
```

This removes the repeated two-dimensional addressing and bounds checks. It
does not alter cache generation, superscalar execution, dataset-item order, or
any RandomX input or output.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Baseline | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,685,528,404 | 6,684,823,892 | 704,512 (0.010537865632%) |
| CFROUND-heavy | 6,878,779,691 | 6,878,075,179 | 704,512 (0.010241816596%) |

The identical delta is consistent with both hashes executing the
protocol-fixed number of dataset reads. Outputs remained exact:

```text
real:     043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
CFROUND:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
exit:     0 for both guests
```

Artifacts:

```text
fbbeb1fad25378c7a06ff0972fbd6cfd7fa986f65bae8e7322fd602c595fdbe3  artifacts/randomx-real-post-audit-baseline       (291712 bytes)
7f812307984e322321707ab045b8f648faec7303210baa6651a8b151f039c847  artifacts/randomx-real-flat-cache-candidate       (291504 bytes)
0e537f9c82e35307f515e150eccaddf2d0e8582683171c4cb54236590f6344de  artifacts/randomx-cfround-post-audit-baseline    (291760 bytes)
0cbb7fd4ffc5f20ec65cd2dfa486ef375ab9c14fdec103c74afc4967201d6124  artifacts/randomx-cfround-flat-cache-candidate    (291552 bytes)
```

These are lightweight executor cycle counts, not a proof or PGU
measurement.

## Safety invariants

- `SeedMemory`'s cache and superscalar-program fields are now private, so
  outside code cannot construct a nonempty program list over a short cache.
- `new_initialised` installs exactly 262,144 blocks. `no_memory` installs zero
  blocks and zero programs, so the read helper is unreachable there.
- Argon2 `Block` is `repr(transparent)` over `[u64; 128]`. Compile-time checks
  require its size to be 1,024 bytes and its alignment to equal `u64`.
- A boxed slice stores its blocks contiguously. The allocation is therefore
  exactly 33,554,432 initialized `u64` words with no inter-block padding.
- The cache has 4,194,304 power-of-two lines. Masking selects a valid line;
  the enumerated lane is `0..8`, so the largest index is one below the word
  count.
- Debug assertions guard cache length, lane, and final word bounds.
- A new mapping test compares boundary and 10,000 deterministic randomized
  register values against the former block/word calculation for all lanes.

Read-only accessors preserve the profiling probes without exposing mutation
of either invariant-bearing field.

## Correctness gates

- all 9 `rustdom-x` release tests pass, including 65,536 rich/compact/unchecked
  superscalar comparisons and the flattened-index test;
- all 4 compact-VM release tests pass, including 1,024 opcode-boundary cases
  and both fixed official hashes;
- all three complete 256 MiB cache digests remain unchanged;
- all 42 official RandomX v1.2.3 light-mode hashes pass across seven key and
  six blob shapes, including 64- and 257-byte arbitrary keys;
- all 20 fixed recent Monero blocks pass their official PoW, difficulty,
  CFROUND-count, final-register, and full-scratchpad checks; and
- the 32-hash lockstep audit passes all 256 programs, 524,288 iterations, and
  every executed instruction state.

Source fingerprints:

```text
b07189c957d1b33834f8c57ef8078579f8569d31f2b73f267d5adce1ccd2fe74  rustdom-x/src/memory.rs
46905aabc81d17fa71b32a3e3d11f3c9b5919e71b8a7c8a3e0c7430be63a91a2  profile-probes/src/main.rs
fa671953f97d812fad475a46ab01ab0eb97cdd8b06139c954db44808031f2a91  profile-probes/tests/cache_digest.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
