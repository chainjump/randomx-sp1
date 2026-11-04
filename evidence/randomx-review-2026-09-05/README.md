# RandomX correctness and production-readiness review

The later [production validation record](../production-2026-09-05/README.md)
completes the corrected standalone guest build, execution, network proof,
and local/EVM verification. The status below describes the earlier source
review and is retained as historical context.

Review date: 2026-09-05 UTC. Corrected source:
[`48e096823fd332076c2b5ab0e272beee27b2b473`](https://github.com/chainjump/randomx-sp1/commit/48e096823fd332076c2b5ab0e272beee27b2b473).
The review began at `a7de1d52c5d2c507f7c8bd6cbaa95f3fa52a8b0a`; the archived
reports describe the same correction while it was still uncommitted.

The superscalar generator had a confirmed correctness defect, now fixed.
After the correction, no additional critical issue or hash-affecting
discrepancy was found within RandomX's specified 0–60-byte key domain.
The evidence is extensive host-side testing and source review. It does not
establish equality for every possible input or validate the final production
SP1 guest. The demonstration EVM code was excluded from the assessment.

## Corrected defect

Canonical `IMULH_R` and `ISMULH_R` retain a random 32-bit operation-group
parameter when their source register is selected. The Rust constructors set
`group_par_is_source` to true, causing source selection to overwrite that
parameter with the source-register number. Subsequent destination-selection
comparisons could therefore differ from canonical RandomX.

The correction sets `group_par_is_source` to false and `can_reuse` to true,
matching canonical C++. The permanent regression test,
`high_multiply_destination_selection_uses_random_group_parameter`, forces
both relevant destination-selection comparisons for both opcodes.

Release `v0.1.0` and the former README dependency pin,
`01d7e7de62b0fa980feb017bde5bc4bb77895c75`, contain the defect. The README now
pins the corrected source commit above. The retained standalone ELF and vkey
still identify the old implementation; they were not regenerated.

Ordinary test keys did not expose the defect. The extended review replaces
the RNG in isolated Rust and C++ generator copies with identical biased
streams, forcing the parameter collisions that ordinary samples rarely
exercise. The corrected code matches; restoring the old flags makes the
harness fail at stream seed `283a6d78569c2e2b`, mode 1, serialized byte 513.
See the [negative-control result](deep/superscalar-old.txt).

This establishes an implementation defect and a controlled divergent program,
not a concrete ordinary RandomX key with a differing full hash. No per-key or
per-hash occurrence rate was established. In particular, “one in 2^32 hashes”
is not a measured or justified failure-rate claim.

The same source defect was confirmed in published RustDom-X 1.1.0 and the
pinned Mithril revision. The [lineage and size record](lineage-and-size.md)
contains their exact revisions and links. It also records 5,148 nonblank,
noncomment implementation lines, including alternate implementations but
excluding tests, tooling, demos and dependencies.

## Canonical comparison evidence

The oracle was the C++ interpreter from
[RandomX v1.2.3, `12f2c2ffe2108d6cf54c391fee33c8bc3646cdab`](https://github.com/tevador/RandomX/tree/12f2c2ffe2108d6cf54c391fee33c8bc3646cdab).
The existing Rust reference interpreter was not the sole oracle.

| Area | Observed coverage | Evidence |
| --- | --- | --- |
| Blake2 stream | 2,097,152 byte/u32 requests across 4,096 cases | [Canonical comparison](canonical-comparison.md) |
| Ordinary superscalar generation | 32,768 complete programs, scheduling metrics and executions | [Canonical comparison](canonical-comparison.md) |
| Portable AES | 216,384 round comparisons; 128 additional AES stage cases | [Canonical comparison](canonical-comparison.md) |
| VM decoder and instruction effects | 2,359,296 comparisons across opcodes, modifiers, aliases, immediate boundaries, branches and rounding modes | [Canonical comparison](canonical-comparison.md) |
| Complete caches, dataset items and hashes | 16 full caches, 2,048 dataset items and 32 fresh complete hashes | [Canonical comparison](canonical-comparison.md) |
| Biased superscalar generation | 24,576 streams, each checked with ordinary, compact checked and production unchecked execution configurations | [Deep comparison](deep-comparison.md) |
| Scheduler boundaries | 393,216 comparisons over selected occupancy windows and dependency/port cases | [Deep comparison](deep-comparison.md) |
| Argon primitives | 49,152 block compressions, 7,864,300 reference indices, 4,097 initial hashes and 30,720 H-prime cases | [Deep comparison](deep-comparison.md) |
| Reciprocal | 2,097,310 comparisons against C++ and an exact integer formula | [Deep comparison](deep-comparison.md) |
| Runtime floating point | 3,996,180 directed-arithmetic comparisons and 1,982,725 further nearest-helper comparisons against Berkeley SoftFloat | [Deep comparison](deep-comparison.md) |
| Complete VM loops | 128 crafted programs, each running all 2,048 iterations with a synthetic dataset; complete register and scratchpad equality | [Deep comparison](deep-comparison.md) |

The reports distinguish real hashes from individual instructions, artificial
RNG streams and synthetic-dataset executions. Counts are case counts, not
estimates of exhaustive input coverage or independent probabilities of safety.
The biased execution configurations reuse the same 24,576 streams.

The [floating-point domain argument](floating-point-domain.md) checks why
branches terminate and why arithmetic cannot reach NaNs or subnormal results.
It addresses the restrictions on which the custom directed-arithmetic code
relies. A conservative interval calculation is retained in
[float-domain-bound.py](float-domain-bound.py). This is a source argument and
finite calculation, not a mechanically verified proof of the implementation.

The runtime probe compiled source from Succinct's public
`succinct-1.94.0-64bit` tag, pinned in [runtime-source.json](runtime-source.json),
on the host. The installed Succinct compiler did not report its source commit,
so the review does not claim byte identity between that public source and the
installed compiler. It also does not claim to have tested RISC-V code generation.

## Remaining differences

| Difference | Scope and consequence |
| --- | --- |
| Key length at least 2^32 bytes | Rust hashes the full key after truncating the encoded length; C++ truncates both. A differing full hash was reproduced for exactly 2^32 zero bytes. This is outside the specified key domain, where longer-key behavior is implementation-defined. |
| AES helper debug assertion | `hash_aes_1rx4` checks a u64 slice length against 64 rather than 8 in one debug assertion, rejecting some valid small inputs. The full RandomX scratchpad satisfies both checks. |
| Superscalar metadata | IROR_C's source flag and constant group parameter, IMUL_RCP's source flag, and eliminated MOV_RR latency differ from C++. Their consumers were traced; these differences do not affect the current generated program or hash. |
| Big-endian portability | Argon initialization writes little-endian bytes into native u64 storage. SP1 and the reviewed host are little-endian. A big-endian port requires additional handling and validation. |

These were documented, not changed by the superscalar correction. Details,
exact large-key outputs, code locations and applicability are in the two
comparison reports.

## Production-readiness judgment

The corrected hashing source is ready for final integration validation. This
review does not recommend putting funds behind a deployment before the
remaining checks below are completed.

| Requirement | Status at documentation commit |
| --- | --- |
| Commit the fix and use an immutable dependency revision containing it | Corrected source committed; README dependency pin updated |
| Host regression and correctness checks on the correction | Passed; release suite recorded 45 passed, 0 failed, 1 intentionally ignored, plus the differential checks above |
| Reproducibly build the final production SP1 guest from the corrected revision and locked dependencies | Not performed for the corrected source |
| Execute that exact guest against canonical RandomX results | Not performed for the corrected source |
| Derive its verification key, generate a proof and verify it | Not performed for the corrected source |
| Review a downstream parent guest's intended statement and input/output binding | Depends on the actual consuming application; not established by this library review |
| Independent second review of custom floating point and unsafe paths | Recommended before funds are put at risk; not claimed as completed |

The expensive guest/proof checks were deliberately excluded at the owner's
request. The July build/execution evidence and the existing demonstration
proof concern a pre-fix guest. They cannot fill the rows above. This
documentation and source push do not publish a new binary release or declare
the remaining production checks complete.

SP1 proves execution of the supplied program; callers must still establish
the intended application claim. Its
[program safety requirements](https://docs.succinct.xyz/docs/sp1/security/security-model#program-safety-requirements)
also require safe guest code and an untampered compiled ELF. This observation
concerns the production SP1 program, independently of the demonstration EVM code.

## Retained records

The canonical and deep comparison reports link to their observed output logs
and source/configuration SHA-256 manifests. The manifests describe the reviewed
source snapshot, not a rebuilt ELF. [Probe-source notes](probe-sources.md)
describe the retained original harness archive, its external dependencies,
absolute paths and reproduction limits.

The fix, regression and host results precede this documentation-only update.
Formatting, whitespace, evidence checksums, archived source identity, and
documentation links were checked before committing the documentation. No
guest or prover build, execution, paid request or release tagging was performed
as part of documenting and pushing these changes.
