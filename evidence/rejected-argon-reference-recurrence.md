# Rejected Argon reference-area recurrence

Date: 2026-07-28 UTC

After the accepted previous-block recurrence, the monotonically increasing
Argon reference-area size was also carried as an explicit loop variable. The
cache bytes remained exact, but this version was slower:

```text
accepted expression:  5,157,814,142 cycles
explicit recurrence:  5,158,731,626 cycles
regression:              917,484 cycles (0.017788%)
output:                 dd0da1aa1eee52ee4b3ebfe834f2904c57e62bd91515f2fe0800000000000000
exit:                   0
```

The previous offset is a 32-bit pointer index with one wrap boundary, so
carrying it helped. The reference-area candidate instead introduced a live
64-bit induction value; LLVM's original compile-time pass/slice expression was
cheaper. The candidate was reverted.

```text
c688c21796596a2d92fbd3eee15989b642cf8ca93a07774718651f5976418b9f  artifacts/randomx-cache-prev-recurrence-candidate       (210488 bytes)
665a1ef1eae22e630c8c7d0415edd6dbe36e9304ccd9f2c121509ee259d84ffb  artifacts/randomx-cache-reference-recurrence-candidate  (210488 bytes)
```

Every command used a 55-second hard timeout. No proof or paid proving-network
request was made.
