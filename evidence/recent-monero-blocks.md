# Twenty recent Monero mainnet block regressions

Date: 2026-07-28 UTC

## Frozen window

The audit fetched 20 contiguous, already-confirmed mainnet blocks from the
public RPC endpoint `https://xmr.cryptostorm.is`:

```text
heights:       3,727,300 through 3,727,319
timestamps:    2026-07-27T21:34:59Z through 2026-07-27T22:43:17Z
seed height:   3,725,312
seed hash:     11c798e5ac6515218bc3efcb5416e5b68c599e42a61b86efe5746bb78eb4be8e
node height:   3,727,376 when the window was selected
```

The endpoint is provenance only; the regression is offline. The JSON fixture
embeds every height, public block ID, previous-block link, timestamp, network
difficulty, canonical RandomX hashing blob, and expected PoW hash.

## Fixture construction checks

For each live RPC response, `network_fixture_builder`:

1. Reconstructed Monero's raw hashing blob from the serialized header, miner
   transaction hash, transaction hashes, CryptoNote tree hash, and transaction
   count.
2. Reproduced the public block ID.
3. Calculated the PoW with the independently built official RandomX v1.2.3
   library.
4. Required the internal reference VM to return the same PoW hash.
5. Required the hash to satisfy the recorded network difficulty.
6. Required contiguous heights, the correct seed epoch, and intact chain
   links.

The restricted RPC returned empty `pow_hash` fields, so expected hashes were
calculated locally rather than trusted from the node.

## Offline regression

The test creates the 256 MiB light cache once for the shared runtime key, then
processes all 20 blobs with both the internal reference and optimized
interpreters. At every height it checks block and chain metadata, network
difficulty, both hashes, final register files, and complete 2 MiB
scratchpads.

The current implementation passed in release mode:

```text
cargo test --release --locked \
  -p randomx-sp1-audit --lib \
  network_fixtures::tests::twenty_recent_mainnet_blocks_match_fixed_pow_hashes \
  -- --exact

test result: ok. 1 passed; 0 failed
```

No proof or paid prover-network request was made as part of this 20-block
differential test.
