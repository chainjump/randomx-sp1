#![no_main]

use std::sync::Arc;

use rustdom_x::{new_vm, VmMemory};
use rustdom_x_compact_vm::calculate_hash;

sp1_zkvm::entrypoint!(main);

// The retained benchmark fixes one RandomX epoch key and accepts the hashing
// blob at runtime. Every generated RandomX program and opcode, including
// CFROUND and its four resulting rounding modes, uses the normal verifier path.
const RANDOMX_SEED: [u8; 32] = [
    0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54, 0x16, 0xe5, 0xb6,
    0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74, 0x6b, 0xb7, 0x8e, 0xb4, 0xbe, 0x8e,
];

pub fn main() {
    let memory = Arc::new(VmMemory::light(&RANDOMX_SEED));
    let mut vm = new_vm(memory);
    let hashing_blob = sp1_zkvm::io::read_vec();
    assert!(!hashing_blob.is_empty());
    let hash = calculate_hash(&mut vm, &hashing_blob);
    sp1_zkvm::io::commit_slice(hash.as_bytes());
}
