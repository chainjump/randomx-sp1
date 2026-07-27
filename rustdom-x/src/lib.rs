#[macro_use]
extern crate log;

pub mod byte_string;
pub mod common;
pub mod hash;
pub mod m128;
pub mod memory;
pub mod program;
pub mod superscalar;
pub mod vm;

pub use crate::memory::VmMemory;
pub use crate::vm::new_vm;

#[test]
fn test_hashing() {
    use crate::memory::VmMemory;
    use crate::vm::new_vm;
    use std::sync::Arc;

    let cache = Arc::new(VmMemory::full(b"test key 000"));
    let mut vm = new_vm(cache);

    for i in 0..10usize {
        let _hash = vm.calculate_hash(&vec![10u8; i * 1000]);
    }
}
