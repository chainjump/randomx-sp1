#![no_main]

use randomx_sp1::hash;

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let randomx_key = sp1_zkvm::io::read_vec();
    let hashing_blob = sp1_zkvm::io::read_vec();
    let output = hash(&randomx_key, &hashing_blob);
    sp1_zkvm::io::commit_slice(&output);
}
