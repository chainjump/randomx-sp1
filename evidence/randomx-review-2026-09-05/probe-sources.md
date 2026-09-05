# Original host review harnesses

[probe-sources.tar.gz](probe-sources.tar.gz) retains 51 original harness and
source-pin files from the review. [SHA256SUMS](SHA256SUMS) checks the archive's
integrity against this evidence record, and
[probe-source-sha256.json](probe-source-sha256.json) lists the SHA-256 of every
archived file. These are research harness snapshots, outside the maintained
Cargo workspace and CI test suite.

```bash
cd evidence/randomx-review-2026-09-05
sha256sum --check SHA256SUMS
tar -tzf probe-sources.tar.gz
```

The archive contains Rust probe packages and lockfiles, C/C++ adapters, private
access wrappers, the deliberately buggy negative control, generator scripts,
and the original host build/run commands. It includes copies of source used by
the probes so their tested implementation can be inspected independently of
future changes to the repository. The negative-control copy is intentionally
incorrect and must never be used as an implementation dependency.

It contains no executables, target directories, credentials, full canonical
dependency tree, or downloaded Rust toolchain implementation. Repository-derived
files retain this repository's licensing and attribution; the copied canonical
superscalar source retains its upstream BSD notice. External dependencies have
their own licenses.

## Preparing a reproduction

The archived source has the original absolute paths. Extract it into a separate
directory and adapt these prefixes, including embedded rpaths and manifest
paths, to that directory and the source checkout:

| Archive directory | Original location |
| --- | --- |
| `canonical/` | `/tmp/randomx-canonical-review-cgtbetcx` |
| `deep/` | `/tmp/randomx-deep-review-_ic3s9ua` |
| `floating-point/` | `/tmp/randomx-sp1-fp-review-qix7n9vq` |
| Source dependency | `/root/experiment/randomx-sp1` |

Use repository revision `48e096823fd332076c2b5ab0e272beee27b2b473`. Obtain a
clean source tree of canonical RandomX commit
`12f2c2ffe2108d6cf54c391fee33c8bc3646cdab` under `canonical/RandomX`.
`canonical/reproduce.sh` builds its shared library and the ordinary comparison
probes. `deep/reproduce.sh` builds the additional adapters and probes against
that same library. The expected negative-control failure is checked explicitly;
a compile failure is not a successful negative control.

The runtime probe additionally requires `library/compiler-builtins` from
Succinct's Rust commit `c7149403db5f6f72f410d6dffcee90378235f23b` under
`deep/toolchain/library/compiler-builtins`. The archive retains its Git tree
metadata and pin. Use the `compiler-builtins`, `builtins-shim` and `libm`
source directories with their `Cargo.toml`, `build.rs` and `configure.rs` files.
The archived `make_runtime_probe.py` narrows that standalone workspace to
`builtins-shim` and `libm`, preserves the original workspace manifest, and
creates the probe that selects these helpers. It uses `RUSTC_BOOTSTRAP=1` for
the compiler-builtins internal features on the host compiler. No SP1 guest is
built by these commands.

The original runs used host Rust 1.97.1. The `--offline` commands assume the
locked Cargo dependencies have already been fetched. The full-cache and
complete-hash probes remain expensive host checks; the explicit `--large-key`
case uses a read-only anonymous 4 GiB zero mapping to inspect the key-length
boundary. It is outside the specified RandomX key domain.

The source-prefix checks in [deep/source-identity.txt](deep/source-identity.txt)
refer to the original workspace before archival. They verify that the copied
Argon and VM sources are unchanged prefixes, that only the RNG was replaced
in the biased superscalar generator, and that directed arithmetic changed only
its nearest-helper selection. The passing historical logs are retained
separately. Archiving these files does not claim a fresh full reproduction of
all probes during the documentation commit.
