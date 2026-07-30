# Fixed Monero proof request

This host-side client submits exactly one proof target: Monero mainnet block
`3,727,837`. The reusable SP1 guest remains input-general; only this network
requester hardcodes the demonstration input.

Hardcoded identities and limits:

```text
block id:    fd20c878bddf0302867fcc5f7ce6b01e6e8d61ee0a4351879232793a8665f6af
seed height: 3727360
seed hash:   0e3b4521acd1982c62a99b6b76ad8504eaa80e164d8e9df3f047b1cf6607f2bd
PoW hash:    5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000
cycle limit: 6500000000
gas limit:   8000000000
deadline:    3600 seconds
```

The pinned reproducible ELF measured 6,447,164,336 SP1 cycles and
7,797,620,538 calibrated PGU for this block. The configured limits therefore
leave 52,835,664 cycles and 202,379,462 PGU of deterministic headroom.

The client validates the canonical 77-byte hashing blob against the block ID,
checks the RandomX epoch height and Monero difficulty, checks the approved ELF
digest, and refuses to submit if a request-ID or proof file already exists.
The only approved identity is the reviewed reproducible ELF with SHA-256
`d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940`;
every other ELF fails closed.

## Build and test

From the repository root:

```bash
CARGO_TARGET_DIR=target cargo test \
  --manifest-path network-prover/Cargo.toml \
  --release --locked --offline

CARGO_TARGET_DIR=target cargo build \
  --manifest-path network-prover/Cargo.toml \
  --release --locked --offline

cargo audit --file network-prover/Cargo.lock \
  --ignore RUSTSEC-2026-0002 -D unsound
```

The nested workspace intentionally uses the root workspace's ThinLTO release
profile so Cargo can reuse the existing SP1 host dependency cache without
changing the guest workspace or its lockfile.

The narrowly ignored `RUSTSEC-2026-0002` advisory affects only
`lru 0.12.5`'s `IterMut::next` and `next_back` methods. The dependency is
transitive through `sp1-prover 6.3.1`; that version's two `LruCache` users call
only `get`, `push`, and `put`, and this client does not use `lru` directly.
Consequently the unsound API is unreachable here. The audit command denies
every other current or future unsoundness warning while suppressing this exact
reviewed exception until SP1 upgrades to a patched `lru` release.

## Quote, submit, and resume

The private-key file must contain the funded Succinct Network requester key
and must not be accessible to group or other users.

```bash
target/release/randomx-sp1-network-prover account \
  .secrets/succinct-network-requester.key 8000000000

target/release/randomx-sp1-network-prover prove \
  .secrets/succinct-network-requester.key \
  artifacts/randomx-sp1-program \
  evidence/network-proof/request-id \
  evidence/network-proof/proof.bin \
  evidence/network-proof/program-vkey
```

The request ID is persisted before waiting. If the waiting process stops,
resume without submitting or paying for a second request:

```bash
target/release/randomx-sp1-network-prover resume \
  .secrets/succinct-network-requester.key \
  artifacts/randomx-sp1-program \
  evidence/network-proof/request-id \
  evidence/network-proof/proof.bin \
  evidence/network-proof/program-vkey
```

The returned proof is accepted only after SP1 verification succeeds and its
public values equal the hardcoded Monero PoW hash.

## EVM simulation only

After the Groth16 proof is saved, simulate the canonical gateway call with an
Ethereum mainnet JSON-RPC endpoint:

```bash
EVM_RPC_URL=https://example.invalid \
target/release/randomx-sp1-network-prover evm-verify \
  artifacts/randomx-sp1-program \
  evidence/network-proof/program-vkey \
  evidence/network-proof/proof.bin
```

This command only issues `eth_chainId` and `eth_call`, and rejects any chain ID
other than Ethereum mainnet (`1`). It contains no EVM signer and cannot
broadcast a transaction. The canonical `verifyProof` function has no Boolean
return value: a non-reverting `eth_call` with empty return data is recorded as
a successful (`true`) simulation.

The standalone guest commits only the 32-byte RandomX hash. The seed and blob
are prover inputs, so an application that needs those values publicly bound
must expose them in its containing SP1 statement.
