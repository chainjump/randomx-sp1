# Rejected local compact-VM program counter

Date: 2026-07-28 UTC

Control checkpoint: `d092719` (`perf: elide masked scratchpad bounds checks`)

## Candidate

The compact instruction effect ABI was changed from a two-argument function
returning unit to a three-argument function returning the program counter.
Ordinary effects returned the input counter unchanged and `CBRANCH` returned
its decoded target when taken. This allowed the 256-instruction execution loop
to keep the counter local and write `Vm::pc` only after the loop, instead of
loading and storing the public field around every indirect effect call.

The full rich/compact lockstep audit passed all 32 hashes, 256 programs,
524,288 VM iterations, and every executed instruction state. All five compact
tests also passed, including opcode boundaries and both fixed hashes.

## Rejection

Every command used `timeout --signal=INT --kill-after=1s 55s`.

```text
control:    6,679,275,975 cycles
candidate:  6,679,294,549 cycles
regression:        18,574 cycles (0.000278084% of control)
```

Passing and returning the counter through all indirect calls costs slightly
more on the SP1 RV64IM executor than the eliminated VM-field traffic. The
candidate also enlarged the ELF by 216 bytes. The source change was reverted;
the measured artifact remains for reproducibility.

```text
0592acfaeeef7dc315d713624a08a5f0f113933afdb6b3ef4d567aa94464c6c6  artifacts/randomx-real-unchecked-iteration-candidate  (281448 bytes)
a3ff454204bb5f88356aab2b5be6712c9e0d15e7d7a2b000c84648b85102bcbc  artifacts/randomx-real-local-pc-candidate             (281664 bytes)
```

These are lightweight executor measurements. No proof or paid
proving-network request was made.
