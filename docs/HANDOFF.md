# Prompt for the next Codex instance

You are continuing an existing performance-engineering task in
`/root/experiment`. Work autonomously: inspect the existing evidence, modify or
rewrite the Rust implementation as necessary, run bounded A/B benchmarks, and
keep optimizing. Do not stop at a list of suggestions.

## Objective

Reduce the SP1 cycle count of a **complete, generic RandomX light-mode
computation** without weakening its claim. The primary metric is the cycle
count for one hash, including:

```text
arbitrary randomx_key
    -> complete 256 MiB RandomX Argon2d cache
    -> all requested dataset items derived from that cache
    -> scratchpad initialization
    -> eight generated RandomX programs
    -> 2,048 VM iterations per program
    -> final RandomX hash
```

The implementation must work for arbitrary keys and hashing blobs under one
fixed SP1 program. It must implement every RandomX opcode and all four
`CFROUND` modes. Do not optimize by relying on the selected benchmark block's
instruction schedule.

The eventual application is the ETH-to-XMR protocol specified in
`ETH_TO_XMR_INTENT_SWAP_PROTOCOL.md`. Its RandomX predicate is deliberately
narrow:

- `randomx_key` is an arbitrary runtime witness. The proof does **not** prove
  that it came from Monero's canonical seed block.
- Nevertheless, the guest must derive the complete cache and every used
  dataset item from that key. An uploaded cache, dataset, item transcript,
  scratchpad, program, or VM trace cannot be accepted as an unchecked fact.
- The hashing blob is tied to the selected work block elsewhere in the
  combined guest.
- The eventual guest checks the exact Monero predicate
  `uint256_le(randomx_output) * randomx_difficulty <= 2^256 - 1` without
  truncating the multiplication.
- It does not need to prove Monero chain membership, canonical seed selection,
  or cumulative work. Do not redesign the swap protocol in this task.

Fixed constants are fine for controlled benchmark fixtures, but an accepted
optimization must not depend semantically on a fixed key, blob, output, absence
of `CFROUND`, or particular generated programs. Protocol-wide RandomX constants
such as Argon2d v1.3, one lane, 262,144 one-KiB blocks, three passes, program
lengths, and iteration counts may be specialized after asserting them.

## Non-negotiable execution constraint

Every build, test, audit, executor, or profiling command must have a hard wall
clock limit below 60 seconds. Use this pattern:

```bash
timeout --signal=INT --kill-after=1s 55s <command>
```

Do not start a heavyweight local SP1 prover, an unbounded background process,
or a paid prover-network request. The lightweight SP1 executor is the routine
measurement path. No paid request is authorized.

## Repository state: read this carefully

There is no Git metadata in `/root/experiment`. Preserve all frozen artifacts,
logs, source archives, and rejected-candidate evidence. Create a new isolated
optimization workspace before making substantial changes; do not overwrite or
relabel old artifacts.

The top-level README's “authoritative” snapshot is historically valid but is
not the latest source:

- `artifacts/randomx-full-program.elf`
- SHA-256:
  `86ebb544e43837bb492ab741af366fb11feeacb868dda4407fd5d62a71034ea0`
- 8,270,833,431 SP1 cycles
- 14.82 seconds in the lightweight executor
- fixed block 3,726,485, whose generated execution contains no `CFROUND`
- frozen source archive:
  `artifacts/randomx-proof-source.tar.gz`

That ELF predates the generic software-`CFROUND` implementation and the latest
Argon2 changes. The root `Cargo.lock` also predates the `randomx-softfp`
dependency. A current root rebuild with `--locked` is not expected to reproduce
the frozen ELF. Do not optimize against or overwrite that stale snapshot.

The current-best generic line is the isolated
`optimization-vm-compact` workspace:

- guest fixture:
  `optimization-vm-compact/program-cfround/src/main.rs`
- compact VM:
  `optimization-vm-compact/compact/src/lib.rs`
- four-mode software floating point:
  `optimization-cfround-soft/softfp/src/lib.rs`
- cache/dataset implementation:
  `vendor/rustdom-x/src/memory.rs`
- latest specialized Argon2 core:
  `experiments/argon2-randomx-specialized/rustdom-x-argon2/src/core.rs`
- latest evidence:
  `optimization-vm-compact/artifacts/argon-nowrap-specialization.txt`

Current source fingerprints are:

```text
f2d2d794cb5ee74bfad47168d2bd6de9b69d9c463a1753ce0fde53effc627d0a  Argon2 core.rs
c62186b3a05d99b26879cce2a904c29e970771f9ce0b2b95535a38d635020798  compact VM lib.rs
4f7ea367799bd77258439dec4a15698067eefb8fc76a6b5d8247b18be631a42a  softfp lib.rs
```

The latest generic-CFROUND measurement is:

```text
SP1 cycles: 6,969,759,319
hash: c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
guest exit code: 0
ELF: optimization-vm-compact/artifacts/randomx-cfround-argon-nowrap-exact
ELF SHA-256: 61f69707391e3b0f2e5ea4823c423ba539f42ee4c2f8c03f94a5cbc03894b025
```

This deliberately difficult fixture executes 18,442 dynamic `CFROUND`s across
nearest/down/up/toward-zero with counts `4738/4852/4325/4527`. The same-source
nearest-only negative control costs 6,729,191,786 cycles and produces the wrong
hash. Exact rounding therefore costs 240,567,533 cycles for this input.
`nearest-only-audit` must never be enabled in a verifier.

The latest phase evidence attributes approximately:

```text
cache construction                 5,250,245,303 cycles (~75%)
full no-memory VM probe              597,809,633 cycles (~8.5%)
on-demand dataset work             ~1,121,704,383 cycles (~16%, inferred)
```

The dataset number is approximate because the no-memory probe changes the VM
trajectory. Re-profile after establishing the new baseline.

`optimization-argon-deep` contains an older fixed-block result of
6,777,323,550 cycles after fusing Argon2 column/row work and using validated raw
access. It predates the generic-CFROUND line, and the inputs differ, so do not
compare those two numbers as an A/B result. The latest shared Argon source
appears to compose the deep compressor with later pass/slice/non-wrapping
specializations; verify this from source rather than assuming it.

## Correctness status

The implementation is a heavily modified fork of crates.io `rustdom-x` 1.1.0,
not Cuprate. Cuprate's RandomX package is Rust FFI around native C++ RandomX and
cannot simply run in SP1's RV64IM guest. Official RandomX v1.2.3 is the external
consensus oracle.

Positive evidence already present includes:

- complete in-guest cache construction and on-demand dataset derivation;
- three complete 256 MiB cache-digest comparisons;
- 65,536 rich/safe/unchecked superscalar state comparisons;
- 32 rich/compact VM differential hashes covering 256 programs and 524,288
  evolving states;
- AES helper differentials;
- one official transcript comparing all 16,384 dataset reads/items for the
  selected block;
- Berkeley SoftFloat comparisons for 20,000 deterministic randomized
  finite-normal cases per operation/mode plus signed-zero and cancellation
  edges; and
- an official-C++-matched CFROUND-heavy full hash on the actual SP1 RV64IM
  executor.

This is strong differential evidence, but it is **not yet justified to claim
that every possible RandomX hash works**. Gaps include a small official
end-to-end corpus, duplicated opcode thresholds/constants in the compact
decoder, unchecked indexing invariants, non-exhaustive floating-point boundary
coverage, and no generated and independently verified SP1 proof.

Any accepted optimization must preserve or improve correctness evidence.
Expand official-C++ differential coverage across many keys/blobs and dataset
indices; add opcode-boundary and one-instruction property tests; add adversarial
floating-point boundary cases; and retain complete cache comparisons. Unsafe
code is acceptable only with explicit, reviewable construction and aliasing
invariants plus differential tests.

## Work plan

1. Read these files completely before editing:

   - `README.md` (treat its root artifact as a frozen historical snapshot)
   - `optimization-vm-compact/README.md`
   - `optimization-vm-compact/artifacts/argon-nowrap-specialization.txt`
   - `optimization-cfround-soft/README.md`
   - `optimization-argon-deep/README.md`
   - `resources/cfround-audit/README.md`
   - `resources/transcript/README.md`

2. Record checksums of the actual current dependency closure. Copy the latest
   generic sources into a clean isolated workspace, regenerate its lockfile,
   and establish source control or an equivalent immutable checksum trail
   there. Leave the frozen root snapshot untouched.

3. Reproduce a clean baseline with both fixtures:

   - real-block fixture, expected hash
     `043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000`;
   - CFROUND-heavy fixture, expected hash
     `c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95`.

   Use `/root/experiment/target/release/execute-fast <ELF> <expected-hash>`.
   Do not infer current cycles from an old artifact.

4. Profile the latest code by phase and pursue claim-preserving changes. The
   cache is still the dominant single-hash cost, followed by dataset-item
   derivation and VM work. Measure each candidate against an otherwise
   identical control. Revert regressions and retain concise evidence for both
   accepted and rejected ideas.

5. Prioritize single-hash improvement. Same-key batching can be explored and
   reported separately, but do not present amortized cycles as a reduction in
   the one-hash predicate. A shared cache must still be derived inside the same
   proved computation or be connected by a sound recursive/authenticated-memory
   construction.

6. Once the best generic core is stable, add or update a fixed-program runtime
   input fixture for arbitrary `randomx_key` and hashing blob and measure its
   overhead. Do not let constant fixtures conceal specialization. Keep journal
   design minimal; this optimization task does not require implementing the
   entire payment/inclusion guest.

7. For every accepted change, preserve:

   - exact source and ELF SHA-256 values;
   - exact SP1 cycles and executor exit/public output;
   - wall time and command;
   - native/reference differential results;
   - safety justification for every unchecked operation; and
   - a clear statement of what is and is not proved.

## Known rejected directions

Do not repeat these without a materially different reason:

- nearest-only or `CFROUND` exclusion: incorrect for generic RandomX;
- externally supplied cache, dataset, or transcript: weakens the proof;
- direct exhaustive opcode dispatch: measured regression;
- explicit Argon round unrolling: +256.6 million cycles;
- fat LTO: regression;
- safe progressive no-zero allocation: regression;
- raw-pointer lookup microchange on the older cache loop: regression;
- removing one temporary zero: only 786,426 cycles saved with added risk;
- giant paired-byte AES table: tiny cycle gain with about 1.05 MiB extra
  VK-bound program data.

Reconsidering one is allowed only if composition or generated code has changed
enough to invalidate the old measurement, and then use a strict A/B test.

## Deliverable

Continue until no credible bounded optimization remains. Deliver:

- the lowest reproducible **generic** single-hash SP1 cycle count;
- the exact isolated source and artifact paths/hashes;
- correctness and differential-test evidence, including CFROUND-heavy output;
- the measured delta for each accepted change and concise rejected-candidate
  notes;
- an honest assessment of whether arbitrary-input support is now justified;
- remaining bottlenecks and the next highest-value direction.

Do not claim proof cost from cycle count. No exact PGU measurement or full proof
exists. A dated proxy put the 6.97-billion-cycle candidate near $1.09, but that
is neither a current quote nor an exact estimate and is not part of this task.
