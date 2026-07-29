# Reproducible SP1 build

## Source and toolchain

```text
source commit:      a9853a1437bf8cb1e92e2a123b88afc463fde095
Cargo.lock SHA-256: 33ebda2e2ec74582258af9a260f3ad3a3c0662874ac87b3c8287f914700b405c
SP1 release:        6.3.1
cargo-prove commit: 8252c29
Docker image:       ghcr.io/succinctlabs/sp1@sha256:0942a27dbe8e38f4b14f3732e779df4027b17bde93e9fbc9e8c773c15eb63400
```

The source commit was clean before the build. From `program/`:

```bash
cargo prove build --docker --tag v6.3.1 --locked \
  --elf-name randomx-program \
  --output-directory ../artifacts
```

The command completed successfully in 1 minute 23 seconds. It produced:

```text
path:        artifacts/randomx-program
format:      ELF64 little-endian RISC-V executable, statically linked
size:        296808 bytes
SHA-256:     54c38936058ea869d31b5e31977174e38f2a9ae14b4b28728ee2f3587132aefc
```

## Verification key

The verification-key hash was derived locally from that exact ELF:

```bash
cargo prove vkey --elf artifacts/randomx-program
```

The command completed successfully in 2 minutes 41 seconds and returned:

```text
0x0033de0ef4a6536badf767961a5ced95181db5b94f48346e0b2f9021c45dffe6
```

The vkey is retained at `evidence/network-proof/program-vkey`. Derivation did
not execute the guest, use a private key, contact the prover network, or create
a proof. No proof is claimed for this ELF yet.

## ELF inspection

`readelf` and the SP1 RISC-V `objdump` report `_start` as the ELF entry point at
`0x78027e30`; no loadable section maps address zero. All 20 `ecall`
instructions are confined to the linked SP1 6.3.1 runtime functions
`syscall_halt`, `syscall_hint_len`, `syscall_hint_read`, and `syscall_write`.
No custom RandomX function contains a direct `ecall`.
