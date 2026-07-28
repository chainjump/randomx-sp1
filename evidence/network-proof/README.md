# Prover-network proof evidence

Status: complete. The prover-network proof, local SP1 verification, and
Ethereum mainnet verifier simulation all succeeded on 2026-07-28 UTC.

## Fixed statement

The host requester in `network-prover/` is hardcoded to Monero mainnet block
`3,727,837`:

```text
block id:    fd20c878bddf0302867fcc5f7ce6b01e6e8d61ee0a4351879232793a8665f6af
timestamp:   1785253434
previous:    df66d34b58d9c65ee20ca8e7c307608db0f7c4e7c6b450bc38e3348d2778f51b
difficulty:  0x9e0ea93a72
seed height: 3727360
seed hash:   0e3b4521acd1982c62a99b6b76ad8504eaa80e164d8e9df3f047b1cf6607f2bd
hashing blob: 1010ba9ca3d306df66d34b58d9c65ee20ca8e7c307608db0f7c4e7c6b450bc38e3348d2778f51b4940173c7c0f26941324afc7aa4e30ffa1b2cd80a84ebbfc464833d6222bee72886d3a9d8a01
PoW hash:    5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000
```

On 2026-07-28 UTC, the block ID, timestamp, previous hash, difficulty, seed
height, and seed hash agreed across these public RPCs:

- `https://xmr.cryptostorm.is/json_rpc`
- `https://xmr-node.cakewallet.com:18081/json_rpc`
- `https://monero.stackwallet.com:18081/json_rpc`

The repository's native reconstruction tool independently rebuilt the
canonical 77-byte hashing blob, checked that it produces the block ID,
calculated the stated RandomX hash, and checked the Monero difficulty.

## SP1 measurements and identity

```text
ELF:          artifacts/randomx-program
ELF SHA-256:  ac3eff37cbae4583f57cdbc193cca776a80672c77a63c09eb507dc35d154c317
SP1 SDK:      6.3.1
SP1 circuit:  v6.1.0
SP1 cycles:   6445471022
SP1 PGU:      7796263443
cycle limit:  6500000000
gas limit:    8000000000
program vkey: 0x0046e62a80cbde7273e58b8e54b6715ebb3cfbf7905e78f985c3e74f6933de4a
```

`cargo prove vkey --elf artifacts/randomx-program` derived the retained vkey
in 2:41.26 wall time with a 5,693,136 KiB maximum resident set. Vkey setup did
not execute the RandomX input or contact the prover network.

The fixed-block client passed all five release tests. Its live zero-balance
test validated the block and ELF, refreshed the quote, and refused before SP1
setup or request submission. The root guest `Cargo.lock` remained unchanged.

`cargo audit` found no blocking vulnerability in the isolated 706-package
host lockfile. It reported nine unmaintained dependencies and the
`RUSTSEC-2026-0002` informational soundness warning in `lru 0.12.5`, pulled in
by `sp1-prover 6.3.1`. The advisory applies specifically to `LruCache::iter_mut`;
the SP1 6.3.1 cache call sites use `new`, `get`, and `push`, not the affected
iterator. This dependency is host-only and is absent from the guest ELF.

## Completed network proof

```text
request ID:    0x24b125a758ee65aeb0556581b83bb69523143514eb58387ecb111e20d0393768
proof file:    evidence/network-proof/proof.bin
proof SHA-256: 1cb71a8414de9ea97570606f3eb8be9281db015006287a9ac388acac5fb22de8
proof bytes:   1726 (serialized SP1ProofWithPublicValues)
route prefix:  0x4388a21c
public value:  5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000
```

The request ID was persisted at `22:24:29` UTC and the locally verified proof
was saved at `22:26:54` UTC, an elapsed time of approximately 2 minutes 25
seconds. `NetworkProver::verify` accepted the downloaded Groth16 proof, and
its sole 32-byte public value exactly matched the selected block's RandomX
PoW hash.

The Explorer reports circuit version `v6.1.0`. This is expected: the installed
`sp1-sdk` and `sp1-prover` crate version is `6.3.1`, while that package's
`SP1_CIRCUIT_VERSION` is `v6.1.0`.

Explorer:
`https://explorer.succinct.xyz/request/0x24b125a758ee65aeb0556581b83bb69523143514eb58387ecb111e20d0393768`

## Network account and cost

```text
requester: 0x19861D4DF321BC5962C15A3f8d7CA33dF3476af4
starting balance: 15.000000000000000000 PROVE
ending balance:   11.067233450650000000 PROVE
proof cost:        3.932766549350000000 PROVE
```

The submission-time maximum cost cap for 8 billion PGU was `6.824448 PROVE`.
Only one request was submitted, so the `3.93276654935 PROVE` balance decrease
is attributable to this proof. The cap was not the actual charge.

## EVM simulation target

The client called the canonical Groth16 gateway using `eth_call`; it has no EVM
signer or transaction-broadcast code.

```text
chain:   Ethereum mainnet (1 / 0x1)
gateway: 0x397A5f7f3dBd538f23DE225B51f532c34448dA9B
method:  verifyProof(bytes32,bytes,bytes)
selector: 0x41493c60
```

On 2026-07-28 UTC, an Ethereum mainnet RPC returned chain ID `0x1`, and
`eth_getCode` returned 1,975 bytes of deployed code for that gateway. The
proof-specific `verifyProof` simulation completed without reverting and
returned the exact empty data (`0x`) required for a successful Solidity `void`
call. The client reported `EVM verification simulation: true` and
`EVM transaction broadcast: no`.
