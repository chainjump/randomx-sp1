use std::env;
use std::io::{self, Read};
use std::sync::Arc;

use randomx_sp1_audit::monero::{
    blob_object_hash, decode_hex, encode_hex, hashing_blob, meets_difficulty,
    parse_wide_difficulty, MoneroBlockFixture, MoneroBlockFixtures, RpcBlockRecord,
};
use randomx_sp1_audit::network_fixtures::randomx_seed_height;
use randomx_sp1_audit::official::OfficialVm;
use randomx_sp1_core::{new_vm, VmMemory};

fn main() {
    let mut args = env::args().skip(1);
    let seed_height: u64 = args
        .next()
        .expect("usage: network_fixture_builder <seed-height> <seed-hash> <rpc-endpoint>")
        .parse()
        .expect("seed height must be an integer");
    let seed_hash = args
        .next()
        .expect("usage: network_fixture_builder <seed-height> <seed-hash> <rpc-endpoint>");
    let rpc_endpoint = args
        .next()
        .expect("usage: network_fixture_builder <seed-height> <seed-hash> <rpc-endpoint>");
    assert!(args.next().is_none(), "too many arguments");

    let seed = decode_hex::<32>(&seed_hash).expect("seed hash must be 32-byte hex");
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let records: Vec<RpcBlockRecord> =
        serde_json::from_str(&input).expect("stdin must be a JSON array of RPC block records");
    assert_eq!(records.len(), 20, "exactly 20 blocks are required");

    let memory = Arc::new(VmMemory::light(&seed));
    let mut reference = new_vm(memory);
    let mut official = OfficialVm::new(&seed);
    let mut blocks = Vec::with_capacity(records.len());

    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.height, records[0].height + index as u64);
        assert_eq!(randomx_seed_height(record.height), seed_height);
        if index > 0 {
            assert_eq!(record.prev_hash, records[index - 1].block_id);
        }

        let blob = hashing_blob(record).unwrap_or_else(|error| {
            panic!(
                "cannot derive hashing blob for block {}: {error}",
                record.height
            )
        });
        assert_eq!(
            encode_hex(&blob_object_hash(&blob)),
            record.block_id,
            "derived hashing blob does not match block id at height {}",
            record.height
        );

        let expected = official.hash(&blob);
        let difficulty = parse_wide_difficulty(&record.wide_difficulty)
            .expect("RPC difficulty must fit Monero's 64-bit difficulty type");
        assert!(
            meets_difficulty(&expected, difficulty),
            "official RandomX hash does not meet network difficulty at block {}",
            record.height
        );
        let actual = reference.calculate_hash(&blob);
        assert_eq!(
            actual.as_bytes(),
            &expected,
            "reference interpreter disagrees with official RandomX at block {}",
            record.height
        );

        blocks.push(MoneroBlockFixture {
            height: record.height,
            block_id: record.block_id.clone(),
            prev_hash: record.prev_hash.clone(),
            timestamp: record.timestamp,
            wide_difficulty: record.wide_difficulty.clone(),
            hashing_blob: encode_hex(&blob),
            pow_hash: encode_hex(&expected),
        });
    }

    let fixture = MoneroBlockFixtures {
        network: "mainnet".to_owned(),
        rpc_endpoint,
        seed_height,
        seed_hash,
        blocks,
    };
    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}
