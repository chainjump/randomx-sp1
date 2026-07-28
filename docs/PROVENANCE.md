# Provenance and recovery

The optimized implementation and its development history live entirely in
this Git repository. Commit `4f1f4a76a2329c677ef3e4743146c6b4d23796a3`
is the original source import; later commits contain the correctness fixes,
optimizations, runtime-input guest, audits, and gas-estimation support.

Generated ELFs are intentionally ignored by Git. Rebuild the current artifact
from `program/` using the locked command in the repository README. To reproduce
an earlier result, check out its historical commit and run that commit's locked
build command.

The current retained ELF was rebuilt from the current source and compared
byte-for-byte before legacy workspaces were removed. Historical recovery is
local: no remote is configured, so the `.git` directory must be preserved or
backed up independently.
