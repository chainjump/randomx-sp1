# Provenance and recovery

The optimized implementation and its development history live entirely in
this Git repository. Commit `4f1f4a76a2329c677ef3e4743146c6b4d23796a3`
is the original source import; later commits contain the correctness fixes,
optimizations, runtime-input guest, audits, and gas-estimation support.

The single retained ELF is tracked as `artifacts/randomx-program`. Rebuild it
from `program/` using the locked Docker command in the repository README. To
reproduce an earlier result, check out its historical commit and run that
commit's locked build command.

On 2026-07-28, source commit
`4aca304adecdad1e2c61bd18c31ed005c4662bac` was built with SP1's Docker image
`ghcr.io/succinctlabs/sp1:v6.3.1`. The tag's OCI index digest was
`sha256:0942a27dbe8e38f4b14f3732e779df4027b17bde93e9fbc9e8c773c15eb63400`,
and its Linux/amd64 manifest digest was
`sha256:7c1c8201de6f63e3f1fb9075bd9a67a4c5fc8c2d546d11a5ff71587bb51e6eb3`.
Builds through both the tag and immutable index digest produced the exact
tracked 295,352-byte ELF with SHA-256
`ac3eff37cbae4583f57cdbc193cca776a80672c77a63c09eb507dc35d154c317`.
The full record is in `evidence/reproducible-build.md`.

The current retained ELF was rebuilt from the current source and measured with
runtime keys. Historical recovery is local: no remote is configured, so the
`.git` directory must be preserved or backed up independently.
