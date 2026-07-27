# RandomX CFROUND for SP1

This workspace implements RandomX binary64 arithmetic on SP1's RV64IM target,
which has no hardware floating-point rounding-mode register. It supports the
four consensus modes selected by `CFROUND`: nearest ties-to-even, toward
negative infinity, toward positive infinity, and toward zero. The implemented
two-lane operations are `FADD`, `FSUB`, `FMUL`, `FDIV`, and `FSQRT`.

Nearest-even uses the target's existing compiler helpers. A directed operation
computes that nearest result, compares it with the exact result using bounded
integer significands, and moves by one representable value only when required.
This avoids a general software-float package in the VM hot loop. Signed zero,
including exact cancellation under round-toward-negative, is handled
explicitly. Finite-input overflow and subsequent infinity propagation are
also handled: the generic VM state machine can reach them even though the
initial RandomX floating-point operands are finite. Subnormal and underflow
results remain excluded by the RandomX operand construction.

## Validation

- Berkeley SoftFloat is the independent host oracle for 20,000 deterministic
  randomized finite-normal cases, every operation, both lanes' scalar
  primitive, and all four modes.
- Explicit oracle tests cover `+0`, `-0`, zero operands, exact cancellation,
  multiply/divide zero signs, `sqrt(-0)`, directed overflow, and infinity
  propagation.
- A packed-x86 audit checks 20,000 deterministic cases per operation and mode
  after setting the corresponding MXCSR rounding control.
- Official RandomX floating-point vectors pass in all four modes.
- An actual SP1 RV64IM microguest embeds the official and signed-zero checks
  and exits successfully.
- The compact full VM matches official RandomX v1.2.3 for an input with 18,442
  executed `CFROUND`s spanning all four modes. The SP1 guest commits
  `c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95`.
- A lockstep audit compares the rich and compact register state after every
  executed instruction across 32 hashes and 256 generated programs.

The current correctness findings and exact post-fix artifacts are frozen in
`evidence/cfround-correctness-fix.md`.

## Measured cost

With the subsequent fixed-pass Argon2 specialization, the optimized full
CFROUND hash takes 6,983,128,950 SP1 cycles. A deliberately
incorrect nearest-only build of the same source and same input takes
6,742,561,417 cycles and returns a different hash. Exact rounding therefore
adds 240,567,533 cycles, or 3.567895% over that control. The CFROUND code is
unchanged by the Argon2 optimization. The first correct
prototype took 7,362,212,643 cycles; integer exact-result comparisons removed
366,238,628 cycles (4.974573%).

See [microbenchmark.txt](artifacts/microbenchmark.txt) for per-operation costs
and [argon-pass-specialization.txt](../optimization-vm-compact/artifacts/argon-pass-specialization.txt)
for the current frozen full-hash evidence. The historical pre-Argon result is
in [cfround-execution.txt](../optimization-vm-compact/artifacts/cfround-execution.txt).
Every build, test, and execution command was bounded with `timeout 55s` after
the user's runtime requirement was set.
