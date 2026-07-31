# Mainnet network-proof evidence

On 2026-07-30, the approved reproducible SP1 ELF was proven through the
Succinct Prover Network's mainnet auction and verified locally and through the
Ethereum-mainnet verifier gateway. No EVM transaction was signed or broadcast.
This is evidence for one execution, not a substitute for the implementation's
correctness corpus or the reproducible ELF/vkey identity.

## Request

```text
request ID:       0xf0ca00e4ef3e4c2f78d51977d4e0a6e66168a98ffa0f9b3b137df44ea2a95603
requester:        0x19861D4DF321BC5962C15A3f8d7CA33dF3476af4
created:          2026-07-30 23:20:50 UTC
fulfilled:        2026-07-30 23:23:15 UTC
network time:     145 seconds
mode:             Groth16
strategy:         mainnet auction
SDK version:      SP1 6.3.1
explorer version: sp1-v6.1.0
request tx:       0xbeac5722219435416a8f24331f6d6dc45084376c9110c5c98ac3379993062bd5
fulfill tx:       0xfea0950af231b54c0bfba28f97f38e6ac7b1ba9855155c6bdcdb38f0a29a7aa1
```

The explorer's `sp1-v6.1.0` label comes from the `SP1_CIRCUIT_VERSION` embedded
in `sp1-prover` 6.3.1; the crate release and circuit-artifact versions are
separate identifiers. The request and both successful verification paths used
the pinned SP1 6.3.1 SDK. The request is public at the
[Succinct Prover Network Explorer](https://explorer.succinct.xyz/request/0xf0ca00e4ef3e4c2f78d51977d4e0a6e66168a98ffa0f9b3b137df44ea2a95603).

## Program and execution

```text
Monero block:     3,727,837
block ID:         fd20c878bddf0302867fcc5f7ce6b01e6e8d61ee0a4351879232793a8665f6af
program vkey:     0x00ef0352217c1bd40da717b661a67da22554bbddc4589ee54fd836f15cc0a771
ELF SHA-256:      d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940
cycles:           6,447,164,336
PGU:              7,797,620,538
public values:    5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000
```

The network-reported cycles and PGU exactly match the locally recorded values
in `../reproducible-build-2026-07-30.md`.

## Cost

```text
base fee:         0.445103 PROVE
clearing price:   570,000,000 PROVE wei/PGU
request cost:     4.88974670666 PROVE
balance before:   11.06723345065 PROVE
balance after:    6.17748674399 PROVE
```

The request cost is the base fee plus the clearing price multiplied by actual
PGU. It was below the fail-closed maximum cap of 7.245103 PROVE.

## Verification

The client loaded the returned proof with SP1 6.3.1, verified it against the
derived verifying key, and rejected any public values other than the expected
Monero PoW hash. Local SP1 verification passed.

The same proof was then simulated with `eth_call` on Ethereum mainnet (chain
ID 1) against the canonical Groth16 gateway
`0x397a5f7f3dbd538f23de225b51f532c34448da9b`. The call did not revert, so EVM
verification returned true. The client has no EVM signer and broadcast no
transaction.

`proof-evm.hex` retains the exact 356 proof bytes accepted by the Solidity
verifier, and `public-values.hex` retains the exact 32 public-value bytes. They
allow anyone to repeat the `eth_call` with an external EVM tool without
building or running this repository's Rust code. The root README gives the
copy-paste `cast call` command. On 2026-07-31, that command was tested with
Foundry 1.7.1 against Ethereum mainnet and returned `0x`.

## Retained files

```text
request-id SHA-256:   efeb49a1991f01e15bf2bd9d9aa3e5ef2bd157e96aea00ccdc8387e9e5ee4a16
proof.bin SHA-256:    0c81249c035a3ab826f9be9e6a61aee5df54198ec7aecea2ea8d4f380fe93a2d
proof.bin SHA-512:    4eec27e3fd5d8e22528468b26dda35a3540fba9c99d1f6276d6bbbb398a7c3c19fec9ed89f471168650888b4c8caeef0a115a368b8e72e942a43ac819507d8b2
proof-evm.hex SHA-256: 2309bc02f6babbb1d786e7e06c9b620649f0b57fa9c7d8ed0b1c5807f7d7fff6
public-values.hex SHA-256: 1cb618a56fd9aad8e28d2a23a24b0183a5db997f0887b3b7a1a3bddca10580d9
```

`proof.bin` is the SP1 proof-with-public-values serialization consumed by the
local verifier and original EVM simulation client. `proof-evm.hex` and
`public-values.hex` are its verifier-ready data representation, not additional
proofs. `SHA256SUMS` covers the one-block proof evidence. The ELF and vkey have
their separate identity manifest in `../../artifacts/SHA256SUMS`.
