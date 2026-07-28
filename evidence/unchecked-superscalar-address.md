# Accepted superscalar address-register lookup

Date: 2026-07-28 UTC

Baseline checkpoint: `e258492` (`perf: flatten RandomX cache word lookup`)

## Change

After each of the eight superscalar programs used to derive a dataset item,
RandomX selects the next cache-line address from one of the eight dataset
registers. `ScProgram::generate` chooses that register in a fixed `0..8` loop,
but the hot path performed a dynamic slice bounds check for every selection.

The selected register is now stored as a private `u8` and read through an
always-inlined method. The existing `unchecked-superscalar` feature uses a
guarded unchecked read; builds without that feature retain safe indexing.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,684,823,892 | 6,684,639,305 | 184,587 (0.002761284411%) |
| CFROUND-heavy | 6,878,075,179 | 6,877,890,592 | 184,587 (0.002683701402%) |

Relative to the post-correctness baseline at `3161c60`, the two dataset
lookup changes together save 889,099 cycles (0.013298859062% real and
0.012925243138% CFROUND).

Both guests returned their exact expected hashes and exit code zero:

```text
real:     043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
CFROUND:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
7f812307984e322321707ab045b8f648faec7303210baa6651a8b151f039c847  artifacts/randomx-real-flat-cache-candidate            (291504 bytes)
f4f2776c52ccd3c3352efe2597cf66207a890064b847c33ddf82aa4ee174cf01  artifacts/randomx-real-unchecked-address-candidate      (291336 bytes)
0cbb7fd4ffc5f20ec65cd2dfa486ef375ab9c14fdec103c74afc4967201d6124  artifacts/randomx-cfround-flat-cache-candidate         (291552 bytes)
8535db5a07f7cad97c691d19d16560f5fcf16c4851f27c23a4a302743edfb636  artifacts/randomx-cfround-unchecked-address-candidate   (291384 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Safety and correctness

- `address_reg` is private and can only be created by `ScProgram::generate`.
- Generation selects it from an explicit `for i in 0..8` loop.
- A debug assertion guards the range at the unchecked-read boundary.
- The superscalar differential checks the method against safe indexing for
  65,536 deterministic program/input combinations and confirms that the
  corpus exercises all eight possible address registers.
- The complete 20-block recent-Monero regression passes after the change,
  including official hashes, difficulty, all four CFROUND modes, final
  registers, and full scratchpads.
- Twelve additional official RandomX v1.2.3 comparisons pass for empty and
  257-byte keys across all six blob shapes. The immediately preceding
  flattened-cache checkpoint passed the full 42-case official corpus; this
  candidate changes only the sealed address-register access described above.
- Both actual SP1 RV64IM guests produce exact outputs.

Source fingerprints:

```text
10f6c8a2c8fd491c53478c481870d5adb5b16bc5b29383e009ab1a85dce0d7ef  rustdom-x/src/superscalar.rs
2af92161c7021f993e7b2ed444e787f09b2bf7fd64eb0028e65e650bedc26217  rustdom-x/src/memory.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
