use sp1_verifier::{Groth16Verifier, GROTH16_VK_BYTES};
use std::{env, fs};

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(
        args.len(),
        5,
        "usage: verify-proof <proof.hex> <public-values.hex> <vkey> <historical-vkey>"
    );
    let proof = hex::decode(fs::read_to_string(&args[1]).unwrap().trim()).unwrap();
    let public = hex::decode(fs::read_to_string(&args[2]).unwrap().trim()).unwrap();
    let vkey = fs::read_to_string(&args[3]).unwrap();
    let historical_vkey = fs::read_to_string(&args[4]).unwrap();
    assert_eq!(public.len(), 32);
    let verify = |proof: &[u8], public: &[u8], key: &str| {
        Groth16Verifier::verify(proof, public, key.trim(), &GROTH16_VK_BYTES)
    };
    verify(&proof, &public, &vkey).expect("released proof must verify");
    println!("SP1 6.3.1 portable Groth16 verifier: valid proof accepted");
    assert!(verify(&proof, &public, &historical_vkey).is_err());
    println!("historical program verification key: rejected");
    let mut changed_public = public.clone();
    changed_public[0] ^= 1;
    assert!(verify(&proof, &changed_public, &vkey).is_err());
    println!("changed public RandomX digest: rejected");
    for index in [0, 35, 67, 99, 100, proof.len() - 1] {
        let mut changed_proof = proof.clone();
        changed_proof[index] ^= 1;
        assert!(
            verify(&changed_proof, &public, &vkey).is_err(),
            "proof mutation at {index} accepted"
        );
    }
    println!("six proof mutations (verifier prefix, exit code, recursion root, nonce, curve proof): rejected");
    assert!(verify(&[], &public, &vkey).is_err());
    println!("empty proof: rejected");
}
