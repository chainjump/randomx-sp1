# Line count and upstream bug check, 2026-09-05

The implementation has 5,148 nonblank, noncomment Rust source lines, excluding
unit-test sections, the separate canonical test file, audit/test crates,
executor/prover tooling, profiling guests, demos, and third-party dependencies.
Rust attributes and lines containing code plus inline comments count as code.
Internal reference code and alternate feature/platform implementations remain
included; this is a source-line count, not a count of code reachable in one
compiled configuration. The count includes the existing superscalar fix.

| Component | Code lines |
| --- | ---: |
| randomx-sp1 | 1,088 |
| randomx-core | 3,298 |
| argon2 | 366 |
| softfp library | 387 |
| main SP1 guest | 9 |
| Total | 5,148 |

All tracked Rust files, including tests and supporting tools but excluding
network-prover, contain 8,663 code lines across 31 files. Per-file implementation
counts and test-tail exclusions are in [line-count.json](line-count.json).
Counting used the Rust lexer to remove ordinary/doc comments while preserving
attributes; files without block comments were independently checked by a
nonblank/non-line-comment physical-line count.

The upstream high-multiply metadata defect was verified in:

- Published RustDom-X 1.1.0, whose `.cargo_vcs_info.json` identifies commit
  `af74663e0986f1d36c04c3d775222dc18a7e19e3`. Its superscalar source matches the
  independently fetched current upstream source byte-for-byte.
  [Pinned source](https://github.com/snap-coin/rustdom-x/blob/af74663e0986f1d36c04c3d775222dc18a7e19e3/src/superscalar.rs#L303-L325).
- Mithril at upstream main commit `29160cc8537084c4c9756ab7710f9cbc6b2e8612`.
  [Pinned source](https://github.com/Ragnaroek/mithril/blob/29160cc8537084c4c9756ab7710f9cbc6b2e8612/src/randomx/superscalar.rs#L303-L325).

Both construct IMULH_R and ISMULH_R with `group_par_is_source: true`, so
`select_source` subsequently replaces the random operation-group parameter
with the selected source register. Both also use `can_reuse: false` where
canonical C++ specifies true. The source-selection scheduling behavior is the
same as the version from which this repository's fixed defect arose. This
establishes the shared defect at the source level; this check did not discover
an actual upstream key producing a divergent full hash or measure its rate.

GitHub commit metadata and line counts are retained beside this note. The
pinned source links identify the external implementations inspected.
No tests, guest builds, prover builds, or repository edits were performed for
this line-count and lineage check.
