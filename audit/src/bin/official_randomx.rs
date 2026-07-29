use std::env;
use std::sync::Arc;
use std::time::Instant;

use randomx_sp1::hash_with_vm_for_audit;
use randomx_sp1_audit::monero::encode_hex;
use randomx_sp1_audit::official::OfficialVm;
use randomx_sp1_core::{new_vm, VmMemory};

fn pattern(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    let mut bytes = vec![0; length];
    for byte in &mut bytes {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        *byte = (state >> 56) as u8;
    }
    bytes
}

fn main() {
    let requested_key = env::args().nth(1).expect(
        "usage: official_randomx <empty|one-byte|test-key|zero-32|monero|pattern-64|pattern-257>",
    );
    let monero_seed = vec![
        0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54, 0x16, 0xe5,
        0xb6, 0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74, 0x6b, 0xb7, 0x8e, 0xb4,
        0xbe, 0x8e,
    ];
    let keys = [
        ("empty", Vec::new()),
        ("one-byte", vec![0xa5]),
        ("test-key", b"test key 000".to_vec()),
        ("zero-32", vec![0; 32]),
        ("monero", monero_seed),
        ("pattern-64", pattern(64, 0x243f_6a88_85a3_08d3)),
        ("pattern-257", pattern(257, 0x1319_8a2e_0370_7344)),
    ];
    let inputs = [
        ("empty", Vec::new()),
        ("one-byte", vec![0]),
        ("text", b"RandomX differential audit".to_vec()),
        ("blob-76", pattern(76, 0xa409_3822_299f_31d0)),
        ("blob-257", pattern(257, 0x082e_fa98_ec4e_6c89)),
        ("blob-4096", pattern(4096, 0x4528_21e6_38d0_1377)),
    ];

    let (key_name, key) = keys
        .into_iter()
        .find(|(name, _)| *name == requested_key)
        .unwrap_or_else(|| panic!("unknown audit key: {requested_key}"));

    let started = Instant::now();
    let mut comparisons = 0usize;
    let memory = Arc::new(VmMemory::light(&key));
    let mut reference = new_vm(Arc::clone(&memory));
    let mut compact = new_vm(memory);
    let mut official = OfficialVm::new(&key);

    for (input_name, input) in &inputs {
        let expected = official.hash(input);
        let reference_hash = reference.calculate_hash(input);
        let compact_hash = hash_with_vm_for_audit(&mut compact, input);

        assert_eq!(
            reference_hash.as_bytes(),
            &expected,
            "reference-interpreter mismatch for key {key_name}, input {input_name}"
        );
        assert_eq!(
            compact_hash.as_bytes(),
            &expected,
            "compact mismatch for key {key_name}, input {input_name}"
        );
        assert_eq!(
            reference.reg.to_bytes(),
            compact.reg.to_bytes(),
            "register mismatch for key {key_name}, input {input_name}"
        );
        assert_eq!(
            reference.scratchpad, compact.scratchpad,
            "scratchpad mismatch for key {key_name}, input {input_name}"
        );
        compact.reset_rounding_mode();
        comparisons += 1;

        println!("{key_name}/{input_name}: {}", encode_hex(&expected));
    }

    println!(
        "official/reference/optimized agreement: {comparisons} complete light-mode hashes for {key_name} in {:.3?}",
        started.elapsed()
    );
}
