# Provenance and recovery

The optimized implementation and its development history live entirely in
this Git repository. The public library is `randomx-sp1`; its internal fork
lineage and licenses are recorded in `ATTRIBUTION.md`.

No ELF, vkey, request, or proof is retained for the current source tree.
Release-facing cleanup invalidated the previous generated identities. After
the final source is approved, the locked Docker build must record the exact
source commit, image digest, lockfile hash, ELF hash, vkey, execution result,
and proof identity before a release tag is created.

No Git remote is configured. The `.git` directory must be preserved or backed
up independently.
