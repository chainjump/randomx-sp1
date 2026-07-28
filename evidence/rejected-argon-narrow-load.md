# Rejected Argon pseudo-random narrowing hint

Date: 2026-07-28 UTC

The one-lane specialized Argon loop only consumes the low 32 bits of the
previous block's first word. Rewriting the existing `u64` mask as an explicit
`u64 -> u32 -> u64` cast was tested in an attempt to induce an RV64IM `lwu`.

LLVM canonicalized both forms identically. The candidate ELF is byte-for-byte
the same as the control, and both execute in 5,157,814,142 cycles with the
same cache output:

```text
c688c21796596a2d92fbd3eee15989b642cf8ca93a07774718651f5976418b9f  artifacts/randomx-cache-prev-recurrence-candidate
c688c21796596a2d92fbd3eee15989b642cf8ca93a07774718651f5976418b9f  artifacts/randomx-cache-narrow-pseudorandom-candidate
```

Both artifacts are 210,488 bytes. The source-only hint was reverted.

Every command used a 55-second hard timeout. No proof or paid proving-network
request was made.
