#![no_main]

use std::sync::Arc;

use rustdom_x::{new_vm, VmMemory};
use rustdom_x_compact_vm::calculate_hash;

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let randomx_key = sp1_zkvm::io::read_vec();
    let memory = Arc::new(VmMemory::light(&randomx_key));
    let mut vm = new_vm(memory);
    let hashing_blob = sp1_zkvm::io::read_vec();
    assert!(!hashing_blob.is_empty());
    let hash = calculate_hash(&mut vm, &hashing_blob);
    sp1_zkvm::io::commit_slice(hash.as_bytes());
}
