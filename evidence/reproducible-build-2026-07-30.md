# Reproducible SP1 build and execution

Date: 2026-07-30 UTC

## Build identity

The universal guest was built twice with:

```text
cargo prove build --docker --tag v6.3.1 --locked \
  --binaries randomx-sp1-program \
  --elf-name randomx-sp1-program \
  --output-directory <independent-output-directory>
```

Both outputs were byte-for-byte identical. Before the second build, the entire
217 MiB Docker target directory was moved aside; the second command therefore
performed a clean 86.66-second compiler pass rather than reusing compiled
guest artifacts.

```text
guest source commit: 9eeaf6349e4f2cdd2576dc79b5629f05e197e6bb
Cargo.lock SHA-256: 62a2b592a03dcb500262cbc0df78a1f2f38410f0777e4a471248793dcdc4bac6
SP1 version:        6.3.1
cargo-prove:        8252c29 (2026-06-25)
guest rustc:        1.94.0-dev
Docker image:       ghcr.io/succinctlabs/sp1:v6.3.1
Docker digest:      sha256:0942a27dbe8e38f4b14f3732e779df4027b17bde93e9fbc9e8c773c15eb63400
ELF size:           289512 bytes
ELF SHA-256:        d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940
ELF SHA-512:        0f51779ccf0732040cc4c80271a19835a57285e81b0dd76a075dd1c3da6f140ff9e27c6e0447dfe102c27b84d60429fb9c87fd3d56734a2d8c73716bf3ed8bf4
program vkey:       0x00ef0352217c1bd40da717b661a67da22554bbddc4589ee54fd836f15cc0a771
```

The generated ELF is retained at `artifacts/randomx-sp1-program`. The
follow-up evidence and host-side prover-client approval do not enter the guest
dependency graph; the guest source that produced the ELF is exactly the commit
recorded above.

## ELF review

The artifact is a statically linked, little-endian, 64-bit RISC-V executable
using the soft-float ABI. Its entry point is `0x78027778`, not zero. Its three
loadable segments begin at `0x78000000`, `0x780068b4`, and `0x7802c318`; no
loadable segment maps address zero, and the GNU stack is writable but not
executable.

The complete disassembly contains 20 `ecall` instructions. Every one lies in a
linked SP1 runtime symbol: `syscall_halt`, `syscall_hint_len`,
`syscall_hint_read`, or `syscall_write`. No application function issues a
direct syscall. The SHA-256 of the demangled disassembly text is
`293f54e1c8fed08a83149a50ca543001232b5c951fee1f4eb40b005b4234cfbf`.

## Real Monero execution

The exact ELF was executed against Monero mainnet block 3,727,837:

```text
block ID:           fd20c878bddf0302867fcc5f7ce6b01e6e8d61ee0a4351879232793a8665f6af
RandomX seed hash:  0e3b4521acd1982c62a99b6b76ad8504eaa80e164d8e9df3f047b1cf6607f2bd
expected PoW hash:  5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000
actual public data: 5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000
guest exit code:    0
SP1 cycles:         6447164336
cycle limit:        6500000000
cycle headroom:     52835664
host wall time:     14.45 seconds
host peak RSS:      536632 KiB
```

The calibrated gas estimator covered all ten trace chunks with its production
threshold and one worker:

```text
SP1 PGU:           7797620538
PGU limit:         8000000000
PGU headroom:      202379462
trace threshold:   134217728
host wall time:    437.68 seconds
host peak RSS:     2634656 KiB
```

Both cycle and PGU measurements are deterministic for this exact ELF and
input. The proof requester is pinned to the ELF SHA-256 above and rejects any
other artifact.

## Dependency audit

`cargo audit -D unsound` found no known vulnerabilities or unsoundness warnings
in the 370-package guest lockfile; five unmaintained-package warnings remain.
The 706-package prover-client lockfile likewise has no known vulnerabilities
and nine unmaintained-package warnings after applying one exact reviewed
exception:

```text
RUSTSEC-2026-0002: lru 0.12.5 IterMut violates Stacked Borrows
```

The affected methods are `IterMut::next` and `next_back`. `lru` is transitive
through `sp1-prover 6.3.1`; inspection of that exact crate found only `get`,
`push`, and `put` calls on its two `LruCache` values. This client does not use
`lru` directly, so the unsound API is unreachable. CI ignores only this
advisory and runs with `-D unsound`, ensuring any other unsoundness finding
fails the build.

## Offline release gates

All source-tree and host-client gates that do not require a prover key or a
returned proof completed successfully:

```text
workspace format checks:             passed
workspace/default-feature clippy:    passed with -D warnings
executor profiling-feature clippy:   passed with -D warnings
soft-float guest-feature clippy:     passed with -D warnings
profile-probe guest-feature clippy:  passed with -D warnings
differential-audit feature clippy:   passed with -D warnings
public-library rustdoc:              passed with -D warnings
workspace release tests:             44 passed; 1 intentionally ignored
prover-client release tests:         7 passed
prover-client release build:         passed offline with --locked
guest and client dependency audits:  passed as described above
```

The ignored test is the deliberately opt-in 32-hash reference/optimized
lockstep audit. The ordinary suite still runs the canonical vectors, Monero
fixtures, complete-cache differential tests, and focused interpreter checks.

## Proof status

No prover-network request was submitted and no funds or requester key were
used. Consequently no proof-dependent local verification or Ethereum
`eth_call` simulation was possible in this build pass.
