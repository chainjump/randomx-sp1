use std::time::Instant;

use blake2b_simd::Params;

const MONERO_SEED: [u8; 32] = [
    0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54, 0x16, 0xe5, 0xb6,
    0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74, 0x6b, 0xb7, 0x8e, 0xb4, 0xbe, 0x8e,
];
const ZERO_SEED: [u8; 32] = [0; 32];
const SALT: &[u8; 8] = b"RandomX\x03";
const MEMORY_BLOCKS: u32 = 262_144;
const ITERATIONS: u32 = 3;

fn baseline_digest(key: &[u8]) -> [u8; 32] {
    let config = baseline::config::Config {
        ad: &[],
        hash_length: 0,
        lanes: 1,
        mem_cost: MEMORY_BLOCKS,
        secret: &[],
        time_cost: ITERATIONS,
        variant: baseline::Variant::Argon2d,
        version: baseline::Version::Version13,
    };
    let context = baseline::context::Context {
        config,
        memory_blocks: MEMORY_BLOCKS,
        pwd: key,
        salt: SALT,
        lane_length: MEMORY_BLOCKS,
        segment_length: MEMORY_BLOCKS / 4,
    };
    let mut memory = baseline::memory::Memory::new(1, MEMORY_BLOCKS);
    baseline::core::initialize(&context, &mut memory);
    baseline::core::fill_memory_blocks(&context, &mut memory);

    let mut digest = Params::new().hash_length(32).to_state();
    for block in memory.blocks.iter() {
        digest.update(block.as_u8());
    }
    digest.finalize().as_bytes().try_into().unwrap()
}

fn optimized_digest(key: &[u8]) -> [u8; 32] {
    let config = optimized::config::Config {
        ad: &[],
        hash_length: 0,
        lanes: 1,
        mem_cost: MEMORY_BLOCKS,
        secret: &[],
        time_cost: ITERATIONS,
        variant: optimized::Variant::Argon2d,
        version: optimized::Version::Version13,
    };
    let context = optimized::context::Context {
        config,
        memory_blocks: MEMORY_BLOCKS,
        pwd: key,
        salt: SALT,
        lane_length: MEMORY_BLOCKS,
        segment_length: MEMORY_BLOCKS / 4,
    };
    let mut memory = optimized::memory::Memory::new(1, MEMORY_BLOCKS);
    optimized::core::initialize(&context, &mut memory);
    optimized::core::fill_memory_blocks_randomx(&context, &mut memory);

    let mut digest = Params::new().hash_length(32).to_state();
    for block in memory.blocks.iter() {
        digest.update(block.as_u8());
    }
    digest.finalize().as_bytes().try_into().unwrap()
}

fn main() {
    let pattern_64: Vec<u8> = (0..64)
        .map(|index| (index as u8).wrapping_mul(0x9d).wrapping_add(0x37))
        .collect();
    let pattern_257: Vec<u8> = (0..257)
        .map(|index| (index as u8).wrapping_mul(0x6d).wrapping_add(0xa5))
        .collect();
    let cases: [(&str, &[u8]); 5] = [
        ("selected-monero-seed", &MONERO_SEED),
        ("zero-32-byte-seed", &ZERO_SEED),
        ("empty-seed", &[]),
        ("pattern-64-byte-seed", &pattern_64),
        ("pattern-257-byte-seed", &pattern_257),
    ];

    for (name, key) in cases {
        let baseline_start = Instant::now();
        let baseline = baseline_digest(key);
        let baseline_elapsed = baseline_start.elapsed();

        let optimized_start = Instant::now();
        let optimized = optimized_digest(key);
        let optimized_elapsed = optimized_start.elapsed();

        println!(
            "{name}: {} (baseline {baseline_elapsed:.3?}, specialized {optimized_elapsed:.3?})",
            hex::encode(baseline)
        );
        assert_eq!(optimized, baseline, "specialized cache differs for {name}");
    }

    println!("all five complete 256 MiB RandomX cache digests match");
}
