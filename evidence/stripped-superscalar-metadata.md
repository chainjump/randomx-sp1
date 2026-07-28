# Accepted release-only superscalar metadata stripping

Date: 2026-07-28 UTC

Baseline checkpoint: `a110a61` (`perf: elide superscalar address bounds check`)

## Change

RandomX superscalar generation retained the rich instruction vector, CPU/ASIC
latency vectors, display strings, and nine diagnostic scheduling metrics in
every `ScProgram`. Compact verifier execution reads only the decoded executable
instructions and selected address register.

Release builds with `compact-superscalar` now discard those diagnostic fields
after generation. Non-compact builds and test builds retain the full rich
representation, so the existing differential remains available. Release
generation also omits metric-only counters and shrinks static macro-op records
by dropping their diagnostic name and encoded-size fields.

The hot release `ScProgram` stride falls from 160 bytes to 32 bytes. No
instruction, scheduling decision, executable immediate, or address-register
selection changes.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Cache/program construction | 5,160,080,785 | 5,159,976,799 | 103,986 (0.002015201008%) |
| Real block | 6,684,639,305 | 6,684,502,428 | 136,877 (0.002047634790%) |
| CFROUND-heavy | 6,877,890,592 | 6,877,753,715 | 136,877 (0.001990101444%) |

Relative to the post-correctness baseline at `3161c60`, the three accepted
lookup/metadata changes save 1,025,976 cycles (0.015346221540% real and
0.014915087357% CFROUND).

All outputs and guest exit codes remain exact:

```text
cache:    dd0da1aa1eee52ee4b3ebfe834f2904c57e62bd91515f2fe0800000000000000
real:     043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
CFROUND:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
exit:     0 for all guests
```

Artifacts:

```text
b6bb2795433503213f0eaceb31128436da29fd84e236d427228f50ed9d157294  artifacts/randomx-cache-current                         (213400 bytes)
d0a239644d041a047a79f6a17af3b08a4bf2e74eed3a198f61dbc300bc646a70  artifacts/randomx-cache-stripped-metadata-candidate     (210520 bytes)
f4f2776c52ccd3c3352efe2597cf66207a890064b847c33ddf82aa4ee174cf01  artifacts/randomx-real-unchecked-address-candidate      (291336 bytes)
710b98f858574b42c74fafe811fcf3b8e06e00960ff47d5afa0d753ab7400e75  artifacts/randomx-real-stripped-metadata-candidate      (289304 bytes)
8535db5a07f7cad97c691d19d16560f5fcf16c4851f27c23a4a302743edfb636  artifacts/randomx-cfround-unchecked-address-candidate   (291384 bytes)
3a2d2488da9739916a4551033aaeff40a65debdee27f80ea0d34844615b185ae  artifacts/randomx-cfround-stripped-metadata-candidate   (289352 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- the 65,536-case rich/compact/unchecked superscalar differential passes and
  retains the full diagnostic representation under `cfg(test)`;
- all four compact-VM release tests pass, including opcode boundaries and both
  official fixed hashes;
- the 20-block recent-Monero regression passes official hashes, network
  difficulty, CFROUND counts, final registers, and complete scratchpads;
- twelve official RandomX v1.2.3 comparisons pass for empty and 257-byte keys
  across all six blob shapes; and
- the cache-only, real, and CFROUND-heavy SP1 guests all commit exact expected
  bytes.

Source fingerprints:

```text
fecebeb2b283d6cdd3f8b61616a9d6ee02cbe78af18c997310fd8a55e09e8a88  rustdom-x/src/superscalar.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
