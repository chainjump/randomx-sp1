use std::sync::Arc;
use std::time::{Duration, Instant};

use randomx_sp1::hash_with_vm_for_audit;
use randomx_sp1_core::{new_vm, VmMemory};

use crate::monero::{
    blob_object_hash, decode_hex, decode_hex_vec, encode_hex, meets_difficulty,
    parse_wide_difficulty, MoneroBlockFixtures,
};

pub const RECENT_MAINNET_BLOCKS_JSON: &str = include_str!("../fixtures/recent_monero_blocks.json");

#[derive(Debug)]
pub struct ValidationSummary {
    pub blocks: usize,
    pub first_height: u64,
    pub last_height: u64,
    pub elapsed: Duration,
}

pub fn randomx_seed_height(height: u64) -> u64 {
    if height <= 2_048 + 64 {
        0
    } else {
        (height - 64 - 1) & !(2_048 - 1)
    }
}

pub fn validate_recent_mainnet_blocks() -> ValidationSummary {
    let fixtures: MoneroBlockFixtures = serde_json::from_str(RECENT_MAINNET_BLOCKS_JSON)
        .expect("the embedded Monero fixture must be valid JSON");
    assert_eq!(fixtures.network, "mainnet");
    assert_eq!(fixtures.blocks.len(), 20, "fixture must contain 20 blocks");

    let seed = decode_hex::<32>(&fixtures.seed_hash).expect("fixture seed hash must be hex");
    let memory = Arc::new(VmMemory::light(&seed));
    let mut reference = new_vm(Arc::clone(&memory));
    let mut compact = new_vm(memory);
    let started = Instant::now();

    for (index, block) in fixtures.blocks.iter().enumerate() {
        assert_eq!(
            block.height,
            fixtures.blocks[0].height + index as u64,
            "fixture heights must be sequential"
        );
        assert_eq!(
            randomx_seed_height(block.height),
            fixtures.seed_height,
            "block {} has the wrong RandomX seed height",
            block.height
        );
        if index > 0 {
            assert_eq!(
                block.prev_hash,
                fixtures.blocks[index - 1].block_id,
                "broken chain link at block {}",
                block.height
            );
        }

        let blob = decode_hex_vec(&block.hashing_blob).expect("hashing blob must be hex");
        assert_eq!(
            encode_hex(&blob_object_hash(&blob)),
            block.block_id,
            "canonical hashing blob does not produce block id at height {}",
            block.height
        );
        let expected = decode_hex::<32>(&block.pow_hash).expect("PoW hash must be hex");
        let difficulty = parse_wide_difficulty(&block.wide_difficulty)
            .expect("fixture difficulty must fit Monero's 64-bit difficulty type");
        assert!(
            meets_difficulty(&expected, difficulty),
            "PoW hash does not meet difficulty at block {}",
            block.height
        );

        let reference_hash = reference.calculate_hash(&blob);
        let compact_hash = hash_with_vm_for_audit(&mut compact, &blob);
        assert_eq!(
            reference_hash.as_bytes(),
            &expected,
            "reference RandomX PoW mismatch at block {}",
            block.height
        );
        assert_eq!(
            compact_hash.as_bytes(),
            &expected,
            "compact RandomX PoW mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.reg.to_bytes(),
            compact.reg.to_bytes(),
            "final register mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.scratchpad, compact.scratchpad,
            "final scratchpad mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.mem_reg.mx, compact.mem_reg.mx,
            "final mx mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.mem_reg.ma, compact.mem_reg.ma,
            "final ma mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.pc, compact.pc,
            "final PC mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.config.read_reg, compact.config.read_reg,
            "final read-register mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.config.e_mask, compact.config.e_mask,
            "final exponent-mask mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.dataset_offset, compact.dataset_offset,
            "final dataset-offset mismatch at block {}",
            block.height
        );
        assert_eq!(
            reference.get_rounding_mode(),
            compact.get_rounding_mode(),
            "final rounding-mode mismatch at block {}",
            block.height
        );
        compact.reset_rounding_mode();
    }

    ValidationSummary {
        blocks: fixtures.blocks.len(),
        first_height: fixtures.blocks.first().unwrap().height,
        last_height: fixtures.blocks.last().unwrap().height,
        elapsed: started.elapsed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_recent_mainnet_blocks_match_fixed_pow_hashes() {
        let summary = validate_recent_mainnet_blocks();
        assert_eq!(summary.blocks, 20);
        assert_eq!(summary.last_height - summary.first_height, 19);
    }
}
