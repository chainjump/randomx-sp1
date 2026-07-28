# Rejected Argon block-pointer recurrence

Date: 2026-07-28 UTC

After accepting the numeric previous-offset recurrence, the sequential current
and previous `Block` pointers were explicitly carried across the specialized
Argon loop. Only the pseudo-random reference pointer remained indexed from the
allocation base.

The cache bytes remained exact, but the SP1 cache probe regressed:

```text
accepted offset recurrence:   5,157,814,142 cycles
explicit pointer recurrence:  5,158,862,723 cycles
regression:                       1,048,581 cycles (0.020330%)
output:                        dd0da1aa1eee52ee4b3ebfe834f2904c57e62bd91515f2fe0800000000000000
exit:                          0
```

The accepted numeric recurrence already lets LLVM optimize sequential address
formation. Keeping two additional pointers live across the fully inlined
1 KiB compressor appears to increase register pressure. The candidate was
reverted.

```text
c688c21796596a2d92fbd3eee15989b642cf8ca93a07774718651f5976418b9f  artifacts/randomx-cache-prev-recurrence-candidate     (210488 bytes)
6ecad38c21905e241e42f5b26d5559854b4de018806859036c58b1ea1f547224  artifacts/randomx-cache-pointer-recurrence-candidate  (210600 bytes)
```

Every command used a 55-second hard timeout. No proof or paid proving-network
request was made.
