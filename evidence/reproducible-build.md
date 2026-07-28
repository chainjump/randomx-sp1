# Reproducible SP1 build evidence

Status: complete. On 2026-07-28 UTC, SP1's Docker build reproduced the ELF
already retained and proved by the Succinct prover network byte for byte.

## Source and builder

```text
source commit:          4aca304adecdad1e2c61bd18c31ed005c4662bac
Docker client/server:   29.1.3 / 29.1.3
SP1 image tag:          ghcr.io/succinctlabs/sp1:v6.3.1
OCI index digest:       sha256:0942a27dbe8e38f4b14f3732e779df4027b17bde93e9fbc9e8c773c15eb63400
Linux/amd64 manifest:   sha256:7c1c8201de6f63e3f1fb9075bd9a67a4c5fc8c2d546d11a5ff71587bb51e6eb3
image creation time:    2026-06-25T11:54:10.081043797Z
container rustc:        rustc 1.94.0-dev
```

The source worktree was clean at the recorded commit. The documentation added
afterward does not alter the guest source, lockfile, or retained ELF.

## Commands

The first build selected the documented release tag:

```bash
cd program
cargo prove build --docker --tag v6.3.1 --locked \
  --elf-name randomx-program \
  --output-directory ../target/reproducible-build
```

The cold build took 1:55.26 wall time; the container's Cargo compilation took
1 minute 26 seconds. A second build selected the immutable OCI index digest:

```bash
cd program
SP1_DOCKER_IMAGE=ghcr.io/succinctlabs/sp1@sha256:0942a27dbe8e38f4b14f3732e779df4027b17bde93e9fbc9e8c773c15eb63400 \
  cargo prove build --docker --tag v6.3.1 --locked \
  --elf-name randomx-program \
  --output-directory ../target/reproducible-build-digest
```

With the image and compilation layers cached, the second command took 7.04
seconds. `--docker` supplies the controlled SP1 build environment. `--locked`
is a Cargo integrity option: it requires the checked-in `Cargo.lock` to be
usable without dependency resolution changes. It is not a substitute for the
container, and the documented `cargo prove build --docker` command remains the
core reproducible-build mechanism.

## Binary identity

Both outputs and the retained artifact had the same identity:

```text
file:    artifacts/randomx-program
size:    295352 bytes
SHA-256: ac3eff37cbae4583f57cdbc193cca776a80672c77a63c09eb507dc35d154c317
cmp tagged output against retained artifact:  success (exit 0)
cmp digest output against retained artifact:  success (exit 0)
format:  ELF64, little-endian, RISC-V
entry:   0x78027d70
```

This establishes that the reproducible build did not merely produce an
equivalent program: it produced the exact binary used for the network proof.

## Program identity and proof verification

The verification key was independently derived from the Docker-built output:

```bash
cargo prove vkey --elf target/reproducible-build/randomx-program
```

```text
program vkey: 0x0046e62a80cbde7273e58b8e54b6715ebb3cfbf7905e78f985c3e74f6933de4a
wall time:    2:41.59
maximum RSS:  6005112 KiB
```

This exactly matches the vkey saved with the completed proof. Vkey derivation
does not run RandomX, submit a prover-network request, or incur PROVE cost.

Finally, the saved proof was checked using the Docker-built ELF as the client
input and an Ethereum-mainnet RPC:

```bash
EVM_RPC_URL=https://ethereum-rpc.publicnode.com \
  target/release/randomx-network-prover evm-verify \
  target/reproducible-build/randomx-program \
  evidence/network-proof/program-vkey \
  evidence/network-proof/proof.bin
```

```text
EVM verification simulation: true
EVM transaction broadcast:   no
chain:                       Ethereum mainnet (1)
gateway:                     0x397a5f7f3dbd538f23de225b51f532c34448da9b
```

No second paid proof was requested. Because the Docker output is byte
identical and has the same vkey, the existing locally verified network proof
is already a proof of this reproducibly built ELF. Re-proving it would only
duplicate the same statement and spend additional PROVE.

The two comparison outputs were temporary ignored files under `target/`; they
were removed after recording this evidence. The sole retained generated guest
ELF remains `artifacts/randomx-program`.
