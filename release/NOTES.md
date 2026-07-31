This rerelease corrects the provenance record and makes the retained proof
bundle independently checkable from the release page. The reviewed library
source, lockfile, SP1 ELF, vkey, and live proof are unchanged from the original
release.

## Correctness and identity

- The implementation matched 20 consecutive Monero blocks in the test suite.
- SP1 version: `6.3.1`
- ELF SHA-256: `d3a15025cf7619615b1be5d35c7d8e3910aac8a399f009319a44235910518940`
- Program vkey: `0x00ef0352217c1bd40da717b661a67da22554bbddc4589ee54fd836f15cc0a771`
- Live prover-network request:
  [`0xf0ca00e4ef3e4c2f78d51977d4e0a6e66168a98ffa0f9b3b137df44ea2a95603`](https://explorer.succinct.xyz/request/0xf0ca00e4ef3e4c2f78d51977d4e0a6e66168a98ffa0f9b3b137df44ea2a95603)
- Ethereum mainnet verification was reproduced with a read-only `eth_call`.

## Verify the release bundle

Download all release assets into one empty directory, then run:

```console
sha256sum --check SHA256SUMS
```

The bundle includes the SP1 guest ELF and vkey, the serialized Groth16 proof,
verifier-ready EVM proof bytes, public values, request ID, and the flat checksum
manifest. See the repository README for source-to-ELF reproduction and the
standalone Ethereum verification command.
