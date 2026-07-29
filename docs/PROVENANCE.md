# Provenance and recovery

The optimized implementation and its development history live entirely in
this Git repository. The public library is `randomx-sp1`; its internal fork
lineage and licenses are recorded in `ATTRIBUTION.md`.

The current ELF was built reproducibly from source commit
`a9853a1437bf8cb1e92e2a123b88afc463fde095` with the locked Docker command in
the repository README. Its container digest, ELF hash, and vkey are recorded
in `evidence/reproducible-build.md`. No proof has been generated for this ELF;
execution and proof evidence must be recorded separately after approval.

No Git remote is configured. The `.git` directory must be preserved or backed
up independently.
