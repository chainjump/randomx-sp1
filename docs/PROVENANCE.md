# Build and source provenance

The implementation and source history live in
[`chainjump/randomx-sp1`](https://github.com/chainjump/randomx-sp1).
Its upstream lineage and licenses are recorded in `ATTRIBUTION.md`.

The current standalone guest uses SP1 **6.10.0** and the corrected RandomX
v0.1.1 source. `artifacts/MANIFEST.json` records its ELF size and hash, EVM vkey,
native network VK hash, toolchain, and local execution. The two identifiers are
different encodings of the same verifying key; a downstream parent program
must derive its own key from its complete ELF.

Two independent locked builds used
`ghcr.io/succinctlabs/sp1:v6.10.0`, image digest
`sha256:9363a70004271de441e863612c7aefbbc5c36f371f111cd8abb1672fa5ea99f1`,
and produced byte-identical ELFs. Guest compilation used
`succinct-1.96.0-64bit-v3`; host tooling uses Rust 1.97.1. The vkey is derived
with the SP1 6.10.0 light prover, without proof generation or network access.

Check `artifacts/SOURCE_SHA256SUMS` from the repository root and
`artifacts/SHA256SUMS` from the artifact directory. The reproducibility record
and dependency audit are retained beside the current ELF. Older release
ELFs, keys, proofs and proof receipts have been removed. No network proof or
gateway validation has been performed for this guest; those release gates
remain open.

The canonical differential corpus and source correctness review remain
applicable to the unchanged hashing implementation. Dated review records
describe their original source snapshots, not this SP1 release. The current
host dependency audit reports `RUSTSEC-2026-0387` in transitive
`crypto-bigint 0.5.5` and maintenance warnings; see the exact findings in
`artifacts/dependency-audit.json`. The patched major version is incompatible
with the current SP1/elliptic-curve dependency line.

Rebuild commands are in `README.md` and `RELEASING.md`. A rebuilt ELF and key
do not recreate a paid network proof.
