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

#[cfg(test)]
mod canonical_v1_tests;
