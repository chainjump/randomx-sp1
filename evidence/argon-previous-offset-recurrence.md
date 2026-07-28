# Accepted Argon previous-offset recurrence

Date: 2026-07-28 UTC

Baseline checkpoint: `d0784fe` (`perf: strip verifier superscalar metadata`)

## Change

The specialized one-lane Argon2 loop recomputed the previous-block offset for
every block as:

```text
(current_offset - 1) & (lane_length - 1)
```

Within each sequential segment, the previous offset after the first iteration
is exactly the prior loop offset. The accepted loop computes the masked value
once before entering the segment and then assigns `previous = current` at the
end of each iteration. This preserves the one required wrap: later passes'
first segment begins at offset zero and starts with previous offset 262,143.

No block, pass, reference formula, compressor operation, or cache byte changes.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Cache/program construction | 5,159,976,799 | 5,157,814,142 | 2,162,657 (0.041912145815%) |
| Real block | 6,684,502,428 | 6,682,339,771 | 2,162,657 (0.032353298144%) |
| CFROUND-heavy | 6,877,753,715 | 6,875,591,058 | 2,162,657 (0.031444234406%) |

The identical full-hash delta confirms that the change is confined to cache
construction. Relative to the post-correctness baseline at `3161c60`, all
four subsequently accepted optimizations save 3,188,633 cycles
(0.047694554676% real and 0.046354631828% CFROUND).

Exact outputs and exit code zero were retained:

```text
cache:    dd0da1aa1eee52ee4b3ebfe834f2904c57e62bd91515f2fe0800000000000000
real:     043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
CFROUND:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
d0a239644d041a047a79f6a17af3b08a4bf2e74eed3a198f61dbc300bc646a70  artifacts/randomx-cache-stripped-metadata-candidate    (210520 bytes)
c688c21796596a2d92fbd3eee15989b642cf8ca93a07774718651f5976418b9f  artifacts/randomx-cache-prev-recurrence-candidate      (210488 bytes)
710b98f858574b42c74fafe811fcf3b8e06e00960ff47d5afa0d753ab7400e75  artifacts/randomx-real-stripped-metadata-candidate     (289304 bytes)
db6b719278f29480aa8c2071930b1a02406cf5a0e5b47911365b9185324e38f9  artifacts/randomx-real-prev-recurrence-candidate       (289280 bytes)
3a2d2488da9739916a4551033aaeff40a65debdee27f80ea0d34844615b185ae  artifacts/randomx-cfround-stripped-metadata-candidate  (289352 bytes)
f7d41308ee665ea94e26ca79e9448d7069b4ea66f165ec1f8d059150761b1f3b  artifacts/randomx-cfround-prev-recurrence-candidate    (289328 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all 61 Argon2 unit tests pass;
- the specialized reference formula and fused scratch-index partition tests
  pass;
- five complete 256 MiB caches match the independent generic crates.io
  implementation byte-for-byte by digest, covering the selected Monero key,
  zero and empty keys, plus 64- and 257-byte keys;
- the cache-only SP1 guest commits the unchanged cache sample;
- both complete actual SP1 RV64IM guests return exact official hashes.

The recurrence does not depend on the key, cache contents, or hashing blob.

Source fingerprints:

```text
5e6be7491ab4f98475bbf8fa6c5ffe3483071a3e1770f2e1c95901e6765a4f0f  argon2/src/core.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
