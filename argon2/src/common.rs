// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// Number of synchronization points between lanes per pass.
pub const SYNC_POINTS: u32 = 4;

/// Memory block size in bytes.
pub const BLOCK_SIZE: usize = 1024;

/// Number of quad words in a block.
pub const QWORDS_IN_BLOCK: usize = BLOCK_SIZE / 8;

/// Pre-hashing digest length.
pub const PREHASH_DIGEST_LENGTH: usize = 64;

/// Pre-hashing digest length with extension.
pub const PREHASH_SEED_LENGTH: usize = 72;

/// Blake2b output length in bytes.
pub const BLAKE2B_OUT_LENGTH: usize = 64;
