# Additional RandomX canonical review, 2026-09-05

Archived review record: the working-tree fix described below is now committed
as `48e096823fd332076c2b5ab0e272beee27b2b473`. Source findings and observed results
are unchanged. See [release status](README.md) and [probe-source notes](probe-sources.md).

No additional critical issue or hash-affecting discrepancy was found for the
specified RandomX v1 key domain (0-60 bytes). This pass found one additional
inert superscalar metadata difference, described below. It does not establish
universal equivalence for every possible input or certify a guest binary.

Reviewed repository: `/root/experiment/randomx-sp1`, HEAD
`a7de1d52c5d2c507f7c8bd6cbaa95f3fa52a8b0a`, with the earlier uncommitted
high-multiply metadata fix. This pass made no implementation changes.
[Source/configuration digests](deep/reviewed-source-sha256.json) identify the
reviewed files. The existing changes to `CHANGELOG.md` and
`randomx-core/src/superscalar.rs` were preserved.

The oracle remains canonical
[RandomX v1.2.3, 12f2c2ffe2108d6cf54c391fee33c8bc3646cdab](https://github.com/tevador/RandomX/tree/12f2c2ffe2108d6cf54c391fee33c8bc3646cdab).
All compilation and execution in this pass was on the host. No SP1 guest,
executor or prover was built or run. The demonstration EVM prover was excluded.
This supplements the [previous canonical review](canonical-comparison.md).

One additional source difference

At [superscalar.rs:364](https://github.com/chainjump/randomx-sp1/blob/48e096823fd332076c2b5ab0e272beee27b2b473/randomx-core/src/superscalar.rs#L364),
IMUL_RCP sets `group_par_is_source = true`. Canonical C++ resets this flag to
false and does not set it for IMUL_RCP. Both set the operation-group parameter
to -1.

This flag cannot affect the generated program: IMUL_RCP's
[source-selection macro-op index is -1](https://github.com/chainjump/randomx-sp1/blob/48e096823fd332076c2b5ab0e272beee27b2b473/randomx-core/src/superscalar.rs#L543),
whereas its executed macro-op indices are 0 and 1. The flag's only consumer
is `select_source`, which is never called for this instruction. This is the
same kind of inactive metadata difference as the previously reported IROR_C
flag, and is distinct from the fixed high-multiply problem, where source
selection does run.

Additional host checks

| Area | Completed coverage | Result / evidence |
| --- | --- | --- |
| Biased superscalar generation | 24,576 streams; complete serialized programs, RNG consumption, address register, all integer scheduling metrics and execution | Match; [superscalar.log](deep/superscalar.txt) |
| Compact checked execution | Same 24,576 biased streams with compact instructions and precomputed reciprocals | Match; [superscalar-compact.log](deep/superscalar-compact.txt) |
| Production unchecked execution | Same 24,576 biased streams with paired dispatch and precomputed reciprocals, compiled without cfg(test) | Match; [superscalar-unchecked.log](deep/superscalar-unchecked.txt) |
| Scheduler | 393,216 comparisons covering every occupancy pattern in selected three-cycle windows, including the end of the 174-cycle map, paired ports, dependencies and commit/no-commit | Match; [superscalar.log](deep/superscalar.txt) |
| Argon block compression | 49,152 comparisons using sparse bits, complements, random blocks, identical read operands, with and without XOR into the old destination | Match; [argon.log](deep/argon.txt) |
| Argon reference indexing | 7,864,300 comparisons: every block position in every one of three passes, ten boundary/random pseudo-random values per position | Match; [argon.log](deep/argon.txt) |
| Argon initialization hash | Every key length 0 through 4,096, fresh deterministic key contents | 4,097 matches; [argon.log](deep/argon.txt) |
| Argon H-prime | All output lengths 1 through 2,048 for 15 input lengths around block and seed boundaries | 30,720 matches; [argon.log](deep/argon.txt) |
| Reciprocal | 2,097,310 divisors, including every divisor 1 through 2^20, random u32 divisors and power-of-two boundaries | Match to C++ and an independent exact-division expression; [integer.log](deep/integer.txt) |
| Branch termination | Algebraic carry analysis plus 1,073,741,824 reduced transition combinations | No three consecutive taken branches; [integer.log](deep/integer.txt), [domain argument](floating-point-domain.md) |
| Directed floating point using pinned runtime helpers | 3,996,180 comparisons, all four modes, targeted exponent/fraction boundaries and 100,000 random pairs | Match to Berkeley SoftFloat; [runtime.log](deep/runtime.txt) |
| Nearest helper edge cases | 1,982,725 further comparisons including subnormals, signed zero, underflow and infinity | Match to Berkeley SoftFloat; [runtime.log](deep/runtime.txt) |
| Whole VM execution | 128 crafted programs/configurations, each executing all 2,048 iterations, using a synthetic dataset to isolate the loop | Registers, full 2 MiB scratchpad, rounding mode, memory registers, dataset offset and configuration all match; [vm-loop.log](deep/vm-loop.txt) |

How the rare superscalar paths were tested

Both isolated generator copies consume the same deterministic replacement
RNG. Some modes heavily bias u32 values toward zero, small register indices,
powers of two and signed boundaries. Others bias bytes to chosen values,
forcing unusual slot/operand-selection patterns. Unbiased values remain
possible so rejection loops can terminate. Generator selection/scheduling
logic is unchanged; this is a controlled stream experiment, not a set of real
Blake2-derived RandomX keys.

The run retired 269,302 high multiplies with group parameters in 0..7, and
exercised 54,031 instruction discards after selection stalls and 93 failures
to map a macro-op into the remaining cycle map. All generated output and RNG
consumption matched. The tests do not claim exhaustive coverage of every
possible full port map or the 256-discard abort guard.

As a negative control, restoring the old high-multiply flags in an isolated
Rust copy makes this harness fail in biased mode 1 at stream seed
`283a6d78569c2e2b`, first differing serialized byte 513. The corrected copy
passes. The [expected failure log](deep/superscalar-old.txt) demonstrates that
these tests can detect the previously missed problem. This artificial stream
is not a real key and does not measure per-key or per-hash occurrence rates.

Argon initialization and memory safety

The C oracle calls the actual canonical compression routine, index function,
initial-hash function and H-prime implementation. The Rust probe contains the
exact production source followed by small visibility wrappers. Index code is
extracted from the production segment loop, not from its unit-test reference.
For the first-pass compression checks, the Rust output buffer really begins
as MaybeUninit storage; later-pass checks begin with initialized old values.

The indexing formulas are also equivalent algebraically for one lane.
On pass zero the reference area is current_offset - 1, so every referenced
block precedes the destination. Later passes use the canonical rotated
reference interval, which excludes the current destination. Both products in
the pseudo-random indexing formula fit in u64. The column and row permutation
indices each cover all 128 scratch words exactly once. Together these facts
support the raw-pointer initialization and non-overlap invariants; the tests
alone are not a proof that uninitialized reads never occur.

Floating-point assumptions and runtime differences

The [domain argument](floating-point-domain.md) checks branch bounds, E underflow,
positive-infinity propagation, F cancellation, FSCAL applied to zero, and F
exponent growth. A conservative interval analysis excludes F infinity/NaNs
through 768 arbitrary instruction effects. These checks address the domain
restrictions on which `randomx-softfp` relies, rather than simply discarding
mismatching subnormal outputs from a general-purpose arithmetic test.

Ordinary host tests use a different nearest-rounding backend from riscv64.
This pass therefore downloaded source from Succinct's public
[succinct-1.94.0-64bit tag, c7149403db5f6f72f410d6dffcee90378235f23b](https://github.com/succinctlabs/rust/tree/c7149403db5f6f72f410d6dffcee90378235f23b/library/compiler-builtins).
Its compiler_builtins version is 0.1.160 and bundled libm reports 0.2.15.
The actual pinned sources were compiled on the host with mangled symbols and
assembly disabled; portable sqrt was forced. The directed-arithmetic source
was copied unchanged except for selecting these nearest helpers. The
additional direct nearest checks also include arithmetic underflow, outside
the narrower RandomX directed-arithmetic contract. For invalid operations
outside the RandomX domain, NaNs were compared by classification, not payload.

The installed Succinct rustc reports 1.94.0-dev but does not report its source
commit. The reviewed public tag is therefore explicitly pinned evidence,
not proof that its files are byte-identical to the installed toolchain's
build. Inspecting symbols in the old guest ELF confirmed the expected
compiler-builtins/libm helper families, but the ELF was neither executed nor
rebuilt. These host checks do not validate riscv64 code generation or SP1
execution/proof semantics.

Whole-loop adversarial cases

Crafted programs include repeated multiplication/overflow, division, square
root, FSCAL including zero, scale/add sequences, add/subtract, all-tier stores,
consecutive branches, a long branch-repeated division block, frequent rounding
changes, immediate L3 loads, lane swaps, integer wraparound and mixed random
instructions. Entropy and initial scratchpad patterns include zero, all ones,
signed-i32 boundaries and random words, as well as matching scratchpad lines,
the last scratchpad line, and dataset-offset endpoints.

The C++ VM uses its canonical initializer and complete interpreter loop.
Only dataset reads/prefetches are overridden with the same deterministic
synthetic data as Rust's existing VmMemory::no_memory. Rust's production loop
is copied verbatim with its AES-generated input replaced by raw program
injection. This checks cumulative effects and iteration mixing; it is not a
complete RandomX hash comparison. The earlier 16 full-cache, 2,048 dataset-item
and 32 fresh complete-hash comparisons remain separate evidence.

Cumulative source coverage and remaining differences

The earlier pass covered Blake2 stream boundaries, AES generation/fingerprinting,
portable AES rounds, the complete VM opcode/modifier map, sign extension,
register aliases, memory tiers, branch targets, dataset derivation, complete
cache initialization and hash chaining/final serialization. This pass adds
biased generator streams, the checked/unchecked compact execution paths,
individual Argon primitives, runtime helper source, floating-point-domain
reasoning and cumulative crafted VM loops. Raw memory accesses were traced
to their register-offset, scratchpad-mask, initialized-cache and program-counter
invariants. No new violation of those invariants was found.

Previously reported differences remain unchanged: the >=2^32-byte key
extension differs from C++ (outside the specified key range); the small-input
AES helper has an incorrect debug-only length assertion; IROR_C metadata and
eliminated MOV_RR latency differ without changing hashes. IMUL_RCP's unused
flag above is now added to that metadata list. Big-endian portability remains
unvalidated; the native-u64 Argon storage path agrees with little-endian SP1
and the host reviewed here.

Probe source and the original host build/run commands are retained in the
[source archive](probe-sources.tar.gz); see [preparation notes](probe-sources.md)
for external pinned sources and the original absolute paths. No attempt is
made to turn finite differential checks into a claim of exhaustive equality
for all keys and execution states.
