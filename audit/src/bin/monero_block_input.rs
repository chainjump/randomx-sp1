use std::{
    env,
    io::{self, Read},
    sync::Arc,
};

use randomx_compact_vm_audit::{
    monero::{
        blob_object_hash, decode_hex, encode_hex, hashing_blob, meets_difficulty,
        parse_wide_difficulty, MoneroBlockFixture, RpcBlockRecord,
    },
    network_fixtures::randomx_seed_height,
};
use rustdom_x::{new_vm, VmMemory};
use rustdom_x_compact_vm::calculate_hash;

const USAGE: &str = "usage: monero_block_input <seed-height> <seed-hash> < record.json";

fn main() {
    let mut args = env::args().skip(1);
    let seed_height: u64 = args
        .next()
        .expect(USAGE)
        .parse()
        .expect("seed height must be an integer");
    let seed_hash = args.next().expect(USAGE);
    assert!(args.next().is_none(), "{USAGE}");

    let seed = decode_hex::<32>(&seed_hash).expect("seed hash must be 32-byte hex");
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("reading block record from stdin");
    let record: RpcBlockRecord =
        serde_json::from_str(&input).expect("stdin must be one RPC block record");
    assert_eq!(
        randomx_seed_height(record.height),
        seed_height,
        "wrong RandomX seed height"
    );

    let blob = hashing_blob(&record).expect("deriving canonical Monero hashing blob");
    assert_eq!(
        encode_hex(&blob_object_hash(&blob)),
        record.block_id,
        "canonical hashing blob does not produce the RPC block ID"
    );

    let memory = Arc::new(VmMemory::light(&seed));
    let mut vm = new_vm(memory);
    let calculated = calculate_hash(&mut vm, &blob);
    let pow_hash: [u8; 32] = calculated
        .as_bytes()
        .try_into()
        .expect("RandomX hash must contain 32 bytes");
    let difficulty = parse_wide_difficulty(&record.wide_difficulty)
        .expect("RPC difficulty must fit Monero's 64-bit difficulty type");
    assert!(
        meets_difficulty(&pow_hash, difficulty),
        "calculated PoW hash does not meet the RPC block difficulty"
    );

    let fixture = MoneroBlockFixture {
        height: record.height,
        block_id: record.block_id,
        prev_hash: record.prev_hash,
        timestamp: record.timestamp,
        wide_difficulty: record.wide_difficulty,
        hashing_blob: encode_hex(&blob),
        pow_hash: encode_hex(&pow_hash),
    };
    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}
