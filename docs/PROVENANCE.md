# Source import provenance

The initial source import is Git commit
`4f1f4a76a2329c677ef3e4743146c6b4d23796a3`. It predates all path rewrites
needed by this unified workspace and is the immutable rollback point for the
copied implementation.

The import copied, rather than deleted, the legacy paths because the handoff
requires preserving historical source archives, artifacts, controls, and
rejected-candidate evidence. Source origins are:

| New path | Legacy source |
|---|---|
| `program/` | `optimization-vm-compact/program/` |
| `program-cfround/` | `optimization-vm-compact/program-cfround/` |
| `compact/` | `optimization-vm-compact/compact/` |
| `audit/` | `optimization-vm-compact/audit/` |
| `profile-probes/` | `optimization-vm-compact/profile-probes/` |
| `softfp/` | `optimization-cfround-soft/softfp/` |
| `softfp-guest/` | `optimization-cfround-soft/guest/` |
| `softfp-runner/` | `optimization-cfround-soft/runner/` |
| `rustdom-x/` | `vendor/rustdom-x/` |
| `argon2/` | `experiments/argon2-randomx-specialized/rustdom-x-argon2/` |
| `argon2-native-compare/` | `experiments/argon2-randomx-specialized/native-compare/` |

The sorted SHA-256 listing for every imported manifest and Rust source is
`checksums/imported-source.sha256`. The digest of those listing lines at the
import commit is:

```text
50f00f4325d5bef6e8186e35037245012fe46b6593874f68062632f753dd204a
```

Key handoff fingerprints match exactly:

```text
f2d2d794cb5ee74bfad47168d2bd6de9b69d9c463a1753ce0fde53effc627d0a  argon2/src/core.rs
c62186b3a05d99b26879cce2a904c29e970771f9ce0b2b95535a38d635020798  compact/src/lib.rs
4f7ea367799bd77258439dec4a15698067eefb8fc76a6b5d8247b18be631a42a  softfp/src/lib.rs
a075a90bf8e748a6145b7b5bb7dcb8a891ff38a6e0a0bf2aca9718d9a2bab8ec  rustdom-x/src/memory.rs
```

After the import commit, only manifests and repository scaffolding were
changed to make every path dependency local to this repository. The consensus
implementation source files remained byte-identical.
