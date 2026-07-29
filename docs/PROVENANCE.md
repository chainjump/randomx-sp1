# Provenance and recovery

The optimized implementation and its development history live entirely in
this Git repository. The public library is `randomx-sp1`; its internal fork
lineage and licenses are recorded in `ATTRIBUTION.md`.

No generated ELF, vkey, or proof is retained for the current source tree. When
approved, the guest should be built from `program/` using the locked Docker
command in the repository README, followed by recording the exact source
commit, image digest, ELF hash, vkey, execution result, and proof identity.

No Git remote is configured. The `.git` directory must be preserved or backed
up independently.
