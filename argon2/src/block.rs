// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::common;
use std::ops::Index;

/// Structure for the (1KB) memory block implemented as 128 64-bit words.
#[repr(transparent)]
pub struct Block([u64; common::QWORDS_IN_BLOCK]);

impl Block {
    /// Gets the byte slice representation of the block.
    pub fn as_u8(&self) -> &[u8] {
        let bytes: &[u8; common::BLOCK_SIZE] = unsafe {
            &*(&self.0 as *const [u64; common::QWORDS_IN_BLOCK] as *const [u8; common::BLOCK_SIZE])
        };
        bytes
    }

    pub(crate) fn as_u8_mut(&mut self) -> &mut [u8] {
        let bytes: &mut [u8; common::BLOCK_SIZE] = unsafe {
            &mut *(&mut self.0 as *mut [u64; common::QWORDS_IN_BLOCK]
                as *mut [u8; common::BLOCK_SIZE])
        };
        bytes
    }

    /// Creates a new block filled with zeros.
    pub(crate) fn zero() -> Block {
        Block([0u64; common::QWORDS_IN_BLOCK])
    }
}

impl Index<usize> for Block {
    type Output = u64;
    fn index(&self, index: usize) -> &u64 {
        &self.0[index]
    }
}

#[cfg(test)]
mod tests {

    use crate::block::Block;
    use crate::common;

    #[test]
    fn as_u8_returns_correct_slice() {
        let block = Block::zero();
        let expected = vec![0u8; 1024];
        let actual = block.as_u8();
        assert_eq!(actual, expected);
    }

    #[test]
    fn as_u8_mut_returns_correct_slice() {
        let mut block = Block::zero();
        let expected = vec![0u8; 1024];
        let actual = block.as_u8_mut();
        assert_eq!(actual, expected);
    }

    #[test]
    fn zero_creates_block_will_all_zeros() {
        let actual = Block::zero();
        assert_eq!(actual.0, [0u64; common::QWORDS_IN_BLOCK]);
    }
}
