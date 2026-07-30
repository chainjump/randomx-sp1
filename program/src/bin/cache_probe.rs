#![no_main]

use randomx_sp1_core::VmMemory;

sp1_zkvm::entrypoint!(main);

const RANDOMX_SEED: [u8; 32] = [
    0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54, 0x16, 0xe5, 0xb6,
    0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74, 0x6b, 0xb7, 0x8e, 0xb4, 0xbe, 0x8e,
];

pub fn main() {
    let memory = VmMemory::light(&RANDOMX_SEED);
    let blocks = memory.seed_memory.blocks();
    let words = [
        blocks[0][0],
        blocks[blocks.len() / 2][64],
        blocks[blocks.len() - 1][127],
        memory.seed_memory.program_count() as u64,
    ];
    let mut output = [0u8; 32];
    for (chunk, word) in output.chunks_exact_mut(8).zip(words) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    sp1_zkvm::io::commit_slice(&output);
}
