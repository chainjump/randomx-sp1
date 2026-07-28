#![no_main]

use std::sync::Arc;

use rustdom_x::{new_vm, VmMemory};
use rustdom_x_compact_vm::calculate_hash;

sp1_zkvm::entrypoint!(main);

mod epoch;
mod static_superscalar {
    include!(concat!(env!("OUT_DIR"), "/static_superscalar.rs"));
}

// The retained benchmark fixes one RandomX epoch key and accepts the hashing
// blob at runtime. Every generated RandomX program and opcode, including
// CFROUND and its four resulting rounding modes, uses the normal verifier path.
pub fn main() {
    let memory = Arc::new(VmMemory::light_with_dataset_item_initializer(
        &epoch::RANDOMX_SEED,
        static_superscalar::init_dataset_item,
    ));
    let mut vm = new_vm(memory);
    let hashing_blob = sp1_zkvm::io::read_vec();
    assert!(!hashing_blob.is_empty());
    let hash = calculate_hash(&mut vm, &hashing_blob);
    sp1_zkvm::io::commit_slice(hash.as_bytes());
}
