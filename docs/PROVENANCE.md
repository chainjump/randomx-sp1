# Provenance and recovery

The optimized implementation and its development history live entirely in
this Git repository. The public library is `randomx-sp1`; its internal fork
lineage and licenses are recorded in `ATTRIBUTION.md`.

The canonical public remote is
[`https://github.com/chainjump/randomx-sp1`](https://github.com/chainjump/randomx-sp1).
Release `v0.1.0` is identified by its signed Git tag. The guest source and
`Cargo.lock` did not change between reviewed source commit
`9eeaf6349e4f2cdd2576dc79b5629f05e197e6bb`, the reproducible build, the live
proof, and the release.

## Retained identities

- `artifacts/randomx-sp1-program` is the reproducible SP1 ELF with SHA-256
  `d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940`.
- `artifacts/randomx-sp1-program.vkey` is the vkey derived from that ELF.
- `evidence/network-proof/` retains the fulfilled request ID, serialized
  Groth16 proof, public values, verifier-ready EVM encoding, and checksums.
- `release/SHA256SUMS` covers the flat files attached to the GitHub release.

The exact build inputs, Docker image digest, lockfile hash, ELF hashes,
disassembly review, execution measurements, and vkey are recorded in
`evidence/reproducible-build-2026-07-30.md`. The request, proof, public output,
cost, local verification, and Ethereum-mainnet verification are recorded in
`evidence/network-proof/README.md`.

## Recovery

The source and retained binary data can be recovered from the signed release
or this Git history. The ELF can also be rebuilt from the pinned source and
Docker image and checked byte-for-byte; its vkey can then be derived locally.
Rebuilding source does not recreate the paid network proof, so the proof data
is retained both in Git and as release assets. A downstream parent program has
a different ELF and must establish and retain its own identity and proof.
