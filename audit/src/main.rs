use std::sync::Arc;

use rustdom_x::{new_vm, VmMemory};
use rustdom_x_compact_vm::calculate_hash;

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

        let memory = Arc::new(VmMemory::no_memory());
        let mut rich = new_vm(Arc::clone(&memory));
        let mut compact = new_vm(memory);
        let rich_hash = rich.calculate_hash(&input);
        let compact_hash = calculate_hash(&mut compact, &input);
        assert_eq!(rich_hash.as_bytes(), compact_hash.as_bytes(), "case {case}");
        assert_eq!(rich.reg.to_bytes(), compact.reg.to_bytes(), "case {case}");
        assert_eq!(rich.scratchpad, compact.scratchpad, "case {case}");
    }

    println!("rich/compact agreement: 32 hashes, 256 generated programs, 524288 VM states");
}
