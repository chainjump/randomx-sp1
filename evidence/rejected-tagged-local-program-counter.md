# Rejected tagged local program counter

Date: 2026-07-28 UTC

Control checkpoint: `ac07524` (`perf: align compact instructions to
power-of-two stride`)

## Candidate

One reserved byte in the 32-byte decoded instruction marked `CBRANCH`. The VM
loop kept the program counter local for ordinary instructions and synchronized
through `Vm::pc` only for tagged branches. This retained the original
two-argument effect ABI, avoiding the return-value overhead of the earlier
local-counter candidate. An all-opcode test verified that exactly opcodes
`0xd6..0xee` received the tag.

All five compact tests passed, including both complete fixed hashes.

## Rejection

Every command used `timeout --signal=INT --kill-after=1s 55s`.

```text
32-byte-stride control:  6,670,983,161 cycles
tagged local PC:          6,671,718,794 cycles
regression:                     735,633 cycles (0.011027355073%)
```

The tag branch executed for every dynamic instruction and outweighed the
avoided `Vm::pc` traffic on ordinary instructions. ELF size also moved in the
opposite direction from cycle cost, confirming that size alone is not the
acceptance metric. The source was reverted and the artifact retained.

```text
6945db9890336d53b6737dee0a3ddf73822885072589f045aeb30f03232ae05c  artifacts/randomx-real-stride32-candidate   (282168 bytes)
2c201b7effe0e3517edb91ad0886d4d6c8d5556111c50b014f9c8f4733505ba3  artifacts/randomx-real-tagged-pc-candidate  (281408 bytes)
```

These are lightweight executor measurements. No proof or paid
proving-network request was made.
