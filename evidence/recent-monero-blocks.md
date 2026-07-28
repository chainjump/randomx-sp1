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

The endpoint is recorded as provenance, not used by the fixed regression.
`audit/fixtures/recent_monero_blocks.json` embeds every height, public block
ID, previous-block link, timestamp, network difficulty, canonical RandomX
hashing blob, expected PoW hash, and per-rounding-mode CFROUND counts.

## Fixture construction checks

For each live RPC response, `network_fixture_builder` performed these checks
before emitting the fixed JSON:

1. Reconstructed Monero's raw hashing blob from the serialized header, miner
   transaction hash, transaction hashes, CryptoNote tree hash, and transaction
   count.
2. Reproduced the public block ID. Monero's block-ID path serializes the blob
   string with a varint length prefix; RandomX itself receives the raw blob.
3. Calculated the PoW with the independently built, pinned official RandomX
   v1.2.3 library.
4. Required Rustdom's rich VM to return that same PoW hash.
5. Required the official PoW hash to satisfy the block's recorded 64-bit
   network difficulty.
6. Required contiguous heights, a single correct RandomX seed epoch, and
   intact previous-block links.

The independent library is the same clean v1.2.3 build documented in
`official-v1.2.3-corpus.md` (commit
`12f2c2ffe2108d6cf54c391fee33c8bc3646cdab`). The node returned an empty
`pow_hash` because it exposes a restricted RPC, so expected PoW hashes were
calculated locally rather than trusted from that node.

## Offline fixed regression

The fixed test creates the 256 MiB light cache once for the shared seed, then
processes all 20 embedded hashing blobs with both the rich and compact VMs. At
every height it checks:

- the block ID and chain/seed metadata;
- the expected PoW against network difficulty;
- rich and compact hashes against the frozen official hash;
- identical final register files and complete 2 MiB scratchpads;
- exact CFROUND execution counts.

It passed in release mode:

```text
timeout --signal=INT --kill-after=1s 55s \
  cargo test --release -p randomx-compact-vm-audit --lib \
  network_fixtures::tests::twenty_recent_mainnet_blocks_match_fixed_pow_hashes \
  -- --exact --nocapture

test result: ok. 1 passed; 0 failed; finished in 39.39s
```

CFROUND was not merely present in one block: every block exercised it (the
per-block totals range from 12,326 to 28,741). Across the window, dynamic
executions by resulting rounding mode were:

```text
nearest:       96,999
down:          95,606
up:            94,513
toward zero:   94,691
total:        381,809
```

## Source fingerprints

```text
88f1d7992129fb5b8a2f63b4d275c854c309d8ac1763c12796a506854064f657  audit/fixtures/recent_monero_blocks.json
e3bc50bf8d7ecd5e0e8856cf9994bcc74300ee11b75f916431a69b0fdaa3501a  audit/src/monero.rs
2b707e0b04e939f396d63d4a1288c123b566f1387c841c25f866fe9c4a203e57  audit/src/network_fixtures.rs
f88c6ef9aa37921d40a45d52b70943c7bd178bfe862962b3795b275212b20a39  audit/src/bin/network_fixture_builder.rs
9f5217cd1a0f45beb00ec428f946ff018b53f247f2f644d25c67fc659df6ce44  audit/src/official.rs
99addb5541be2b56a9f5fd82700b54f4267244d1af00febe5fa5953b693dc435  audit/src/bin/official_randomx.rs
9cd6e76f5d36b2c0af4bc8d08d97ef301a09d40d75f44fee3949244ec13894a4  Cargo.lock
```

No proof or paid proving-network request was made.
