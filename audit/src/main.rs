use randomx_sp1::differential_audit;

fn main() {
    for case in 0..32u64 {
        let mut input = [0u8; 96];
        let mut state = case.wrapping_add(1).wrapping_mul(0xd134_2543_de82_ef95);
        for chunk in input.chunks_exact_mut(8) {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
            chunk.copy_from_slice(&state.to_le_bytes());
        }

        let _hash = differential_audit(&input);
    }

    println!(
        "reference/optimized agreement: 32 hashes, 256 generated programs, \
         524288 VM iteration states, and every executed instruction state"
    );
}
