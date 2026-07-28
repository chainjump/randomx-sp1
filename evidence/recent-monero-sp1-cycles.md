# Recent Monero block SP1 cycle distribution

Date: 2026-07-28 UTC

This measurement executes the 20 fixed sequential Monero mainnet blocks at
heights 3,727,300 through 3,727,319 in the SP1 v6.3.1 lightweight executor.
It is an executor-cycle measurement, not a proof or a prover-cost estimate.

The measured compact VM checkpoint is `1544701` (`perf: keep immutable VM
iteration config local`). The guest fixes the network fixture's RandomX seed
`11c798e5ac6515218bc3efcb5416e5b68c599e42a61b86efe5746bb78eb4be8e`,
reads one canonical hashing blob from the SP1 hint stream, performs one complete
light-mode RandomX hash including construction of the 256 MiB cache, and commits
the 32-byte result. Every run returned exit code zero and the executor compared
the committed result with that block's fixed network PoW hash.

## Results

| Height | SP1 cycles | Dynamic CFROUNDs |
|---:|---:|---:|
| 3,727,300 | 6,831,730,329 | 22,560 |
| 3,727,301 | 6,871,796,664 | 24,610 |
| 3,727,302 | 6,922,656,603 | 16,413 |
| 3,727,303 | 6,898,327,806 | 12,326 |
| 3,727,304 | 6,820,350,509 | 16,448 |
| 3,727,305 | 6,810,551,529 | 16,419 |
| 3,727,306 | 6,835,123,007 | 18,486 |
| 3,727,307 | 6,797,474,537 | 16,404 |
| 3,727,308 | 6,810,990,077 | 18,464 |
| 3,727,309 | 6,828,038,987 | 14,390 |
| 3,727,310 | 6,879,791,411 | 22,563 |
| 3,727,311 | 6,804,307,220 | 16,415 |
| 3,727,312 | 6,847,666,349 | 18,469 |
| 3,727,313 | 6,854,917,409 | 28,741 |
| 3,727,314 | 6,884,420,531 | 20,532 |
| 3,727,315 | 6,766,247,664 | 20,532 |
| 3,727,316 | 6,872,975,568 | 22,578 |
| 3,727,317 | 6,811,274,150 | 20,526 |
| 3,727,318 | 6,886,302,273 | 20,557 |
| 3,727,319 | 6,796,121,961 | 14,376 |

The exact sum is **136,831,064,584 cycles**, giving an arithmetic mean of
**6,841,553,229.2 cycles per block**. The median is 6,833,426,668 cycles. The
minimum is 6,766,247,664 cycles at height 3,727,315; the maximum is
6,922,656,603 cycles at height 3,727,302; and the range is 156,408,939 cycles.
These blocks contain 381,809 dynamic `CFROUND` executions in total, and every
block exercises all four rounding modes.

The same retained ELF executes the original benchmark blob in 6,650,702,843
cycles. Cycle count is input-dependent; no correction has been applied to the
20-block measurements.

## Reproduction

The ELF was built once, then reused for every block:

```text
timeout --signal=INT --kill-after=1s 55s \
  cargo prove build --locked \
  --elf-name randomx-program --output-directory ../artifacts

timeout --signal=INT --kill-after=1s 55s \
  target/release/randomx-executor artifacts/randomx-program \
  <fixed-pow-hash-hex> <fixed-hashing-blob-hex>
```

Each executor invocation was independently capped at 55 seconds. Runs were
issued in small parallel batches, but SP1's reported global clock is a
deterministic guest instruction count and is independent of host wall time.

Artifact identity:

```text
a55aa6f4a1b6535bf7771cfa5dc53d38f85e4795eb6dacd339f5fd3581d1c308  artifacts/randomx-program
size: 273688 bytes
```
