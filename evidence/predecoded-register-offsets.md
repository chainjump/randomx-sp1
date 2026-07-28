# Accepted predecoded register byte offsets

Date: 2026-07-28 UTC

Baseline checkpoint: `14dc5d3` (`perf: use scratchpad byte offsets directly`)

## Change and safety argument

Decoded instructions stored integer and floating-point register numbers.
Every dynamic effect shifted those numbers by three or four before loading or
storing a register. The decoder now stores the corresponding byte offsets in
the same one-byte fields: integer registers use multiples of eight and packed
floating registers use multiples of sixteen. Mixed floating-memory forms use
a floating destination offset and an integer address-register offset.

The compact instruction remains exactly 32 bytes. Decode-time register-usage
tracking still operates on ordinary register numbers before construction, so
branch dependency semantics are unchanged.

The private pointer helpers require aligned, bounded offsets in debug builds.
Compile-time assertions establish the register element sizes and that all
offsets fit in one byte. A directed unit test checks every valid integer and
floating offset plus the no-register sentinel. The exhaustive opcode test then
executes every decoded operand class against the rich VM.

## SP1 A/B measurements

Every command used `timeout --signal=INT --kill-after=1s 55s`.

| Fixture | Prior accepted | Candidate | Reduction |
|---|---:|---:|---:|
| Real block | 6,657,658,031 | 6,650,932,295 | 6,725,736 (0.101022551304%) |
| Rounding regression | 6,853,244,607 | 6,846,492,491 | 6,752,116 (0.098524368926%) |

Relative to the post-correctness baseline at `3161c60`, accepted
optimizations now save 34,596,109 cycles (0.517477556139%) on the real-block
fixture and 32,287,200 cycles (0.469373950764%) on the rounding regression.

Both guests exited zero and retained their exact official hashes:

```text
real:                 043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
rounding regression:  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

Artifacts:

```text
61a665217d2f552911157139c158106efd10a6298de26e6c527545fd10d5ce62  artifacts/randomx-real-register-offset-candidate     (279856 bytes)
4b1fa532e44bdb0baf7992391de2b38d9b4fc6cf575c9bac1a0a0112c7cb5a60  artifacts/randomx-cfround-register-offset-candidate  (279904 bytes)
```

These are lightweight executor measurements, not a proof or PGU result.

## Correctness gates

- all seven compact verifier tests pass, including offset invariants, all
  opcode/memory-mask boundaries, invalid public inputs, and both hashes;
- the lockstep audit passes all 32 hashes, 256 programs, 524,288 iteration
  states, and every executed instruction state;
- all 20 fixed recent Monero mainnet blocks pass with official hashes, rich
  state, complete scratchpads, ordinary CFROUND, block IDs, chain links, seed
  epoch, and difficulty;
- both complete SP1 RV64IM guests return the expected official hash.

Source fingerprints:

```text
bb7b4128e82fcb159819682657ffc269e42aa36b64b00aac95ac32cc178943eb  compact/src/lib.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
