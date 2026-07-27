// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::block::Block;
use crate::common;
use crate::context::Context;
use crate::memory::Memory;
use crate::variant::Variant;
use crate::version::Version;
use blake2b_simd::Params;
use core::mem::MaybeUninit;

/// Position of the block currently being operated on.
#[derive(Clone, Debug)]
struct Position {
    pass: u32,
    lane: u32,
    slice: u32,
    index: u32,
}

/// Initializes the memory.
pub fn initialize(context: &Context, memory: &mut Memory) {
    fill_first_blocks(context, memory, &mut h0(context));
}

/// Fills all the memory blocks.
pub fn fill_memory_blocks(context: &Context, memory: &mut Memory) {
    fill_memory_blocks_st(context, memory);
}

const RANDOMX_LANE_LENGTH: u32 = 262_144;
const RANDOMX_SEGMENT_LENGTH: u32 = 65_536;

fn validate_randomx_context(context: &Context) {
    assert_eq!(context.config.lanes, 1);
    assert_eq!(context.config.mem_cost, RANDOMX_LANE_LENGTH);
    assert_eq!(context.config.time_cost, 3);
    assert_eq!(context.config.variant, Variant::Argon2d);
    assert_eq!(context.config.version, Version::Version13);
    assert_eq!(context.memory_blocks, RANDOMX_LANE_LENGTH);
    assert_eq!(context.lane_length, RANDOMX_LANE_LENGTH);
    assert_eq!(context.segment_length, RANDOMX_SEGMENT_LENGTH);
}

/// Allocates and initializes RandomX's fixed Argon2 memory without eagerly
/// writing zeroes to all 256 MiB before the first pass overwrites them.
///
/// The first two blocks are initialized locally. In the first pass, RandomX's
/// one-lane Argon2d reference rule reads only earlier blocks, and
/// `fill_block_raw::<false>` writes all 128 words of the current block without
/// reading it. Thus the initialized prefix grows by exactly one block on every
/// iteration. The boxed slice is converted from `MaybeUninit<Block>` only
/// after that prefix covers the complete allocation; later passes then operate
/// on ordinary initialized `Block` values.
pub fn initialize_memory_randomx(context: &Context) -> Memory {
    validate_randomx_context(context);

    let mut blocks = Box::<[Block]>::new_uninit_slice(RANDOMX_LANE_LENGTH as usize);
    let blocks_ptr = blocks.as_mut_ptr().cast::<Block>();
    let mut initial_hash = h0(context);
    let seed_position = common::PREHASH_DIGEST_LENGTH;

    for block_index in 0..2usize {
        initial_hash[seed_position..seed_position + 4]
            .copy_from_slice(&(block_index as u32).to_le_bytes());
        initial_hash[seed_position + 4..seed_position + 8].copy_from_slice(&0u32.to_le_bytes());
        let mut block = Block::zero();
        hprime(block.as_u8_mut(), &initial_hash);
        // SAFETY: the allocation contains RANDOMX_LANE_LENGTH properly aligned
        // Block slots. Each of the first two slots is written exactly once.
        unsafe { blocks_ptr.add(block_index).write(block) };
    }

    // SAFETY: blocks 0 and 1 are initialized above. The specialized first pass
    // visits every remaining block in increasing order, reads only the already
    // initialized prefix, and fully writes each new destination without first
    // reading it.
    unsafe { fill_memory_blocks_randomx_pass::<true, false>(blocks_ptr) };

    // SAFETY: the first-pass loop has now initialized every block in the
    // allocation exactly once, so all bit patterns are valid `Block` values.
    let initialized = unsafe { blocks.assume_init() };
    let mut memory = Memory::from_blocks(1, RANDOMX_LANE_LENGTH, initialized);
    let initialized_ptr = memory.blocks.as_mut_ptr();

    // SAFETY: every block is initialized, and the Argon2 reference formula
    // keeps both shared inputs distinct from the writable current block.
    unsafe {
        fill_memory_blocks_randomx_pass::<false, true>(initialized_ptr);
        fill_memory_blocks_randomx_pass::<false, true>(initialized_ptr);
    }
    memory
}

/// Fills memory for RandomX's fixed Argon2d v1.3 configuration.
///
/// This preserves the generic implementation above for all public Argon2
/// configurations. RandomX has one lane, three passes, four 65,536-block
/// segments, and a power-of-two 262,144-block lane. Stating those invariants
/// explicitly removes per-block lane selection, version/variant branches, and
/// general integer remainder operations from the zkVM execution.
pub fn fill_memory_blocks_randomx(context: &Context, memory: &mut Memory) {
    validate_randomx_context(context);
    assert_eq!(memory.blocks.len(), RANDOMX_LANE_LENGTH as usize);

    let blocks = memory.blocks.as_mut_ptr();
    // SAFETY: `Memory::new` initialized the complete allocation. The Argon2
    // reference formula keeps both shared inputs distinct from the current
    // writable block.
    unsafe {
        fill_memory_blocks_randomx_pass::<true, false>(blocks);
        fill_memory_blocks_randomx_pass::<false, true>(blocks);
        fill_memory_blocks_randomx_pass::<false, true>(blocks);
    }
}

#[inline(always)]
unsafe fn fill_memory_blocks_randomx_pass<const FIRST_PASS: bool, const WITH_XOR: bool>(
    blocks: *mut Block,
) {
    unsafe {
        fill_memory_blocks_randomx_segment::<FIRST_PASS, WITH_XOR, 0>(blocks);
        fill_memory_blocks_randomx_segment::<FIRST_PASS, WITH_XOR, 1>(blocks);
        fill_memory_blocks_randomx_segment::<FIRST_PASS, WITH_XOR, 2>(blocks);
        fill_memory_blocks_randomx_segment::<FIRST_PASS, WITH_XOR, 3>(blocks);
    }
}

#[inline(always)]
unsafe fn fill_memory_blocks_randomx_segment<
    const FIRST_PASS: bool,
    const WITH_XOR: bool,
    const SLICE: u32,
>(blocks: *mut Block) {
    const LANE_LENGTH: u32 = 262_144;
    const SEGMENT_LENGTH: u32 = 65_536;
    const LANE_MASK: u32 = LANE_LENGTH - 1;

    const { assert!(SLICE < common::SYNC_POINTS) };
    let starting_index = if FIRST_PASS && SLICE == 0 { 2 } else { 0 };
    let segment_start = SLICE * SEGMENT_LENGTH;
    let segment_end = segment_start + SEGMENT_LENGTH;
    for curr_offset in segment_start + starting_index..segment_end {
        let index_in_segment = curr_offset - segment_start;
        let prev_offset = curr_offset.wrapping_sub(1) & LANE_MASK;
        // SAFETY: all three offsets are masked or constructed within the
        // validated 262,144-block RandomX lane. Argon2's reference formula
        // excludes the current block from both read operands.
        let pseudo_rand = unsafe { (&*blocks.add(prev_offset as usize))[0] };

        let reference_area_size = if FIRST_PASS {
            curr_offset - 1
        } else {
            LANE_LENGTH - SEGMENT_LENGTH + index_in_segment - 1
        } as u64;
        let mut relative_position = (pseudo_rand & 0xffff_ffff) as u64;
        relative_position = (relative_position * relative_position) >> 32;
        relative_position =
            reference_area_size - 1 - ((reference_area_size * relative_position) >> 32);

        let start_position = if FIRST_PASS || SLICE == common::SYNC_POINTS - 1 {
            0
        } else {
            (SLICE + 1) * SEGMENT_LENGTH
        } as u64;
        let absolute_position = start_position + relative_position;
        let ref_offset = if FIRST_PASS || SLICE == common::SYNC_POINTS - 1 {
            // The first pass always starts at zero and can only reference an
            // earlier block. Later passes' final slice also starts at zero,
            // while its reference area remains shorter than the lane. These
            // compile-time-specialized cases therefore cannot wrap.
            debug_assert!(absolute_position < LANE_LENGTH as u64);
            absolute_position as usize
        } else {
            (absolute_position & LANE_MASK as u64) as usize
        };
        debug_assert_ne!(prev_offset as usize, curr_offset as usize);
        debug_assert_ne!(ref_offset, curr_offset as usize);
        if FIRST_PASS {
            debug_assert!((prev_offset as usize) < curr_offset as usize);
            debug_assert!(ref_offset < curr_offset as usize);
        }
        unsafe {
            fill_block_raw::<WITH_XOR>(
                &*blocks.add(prev_offset as usize),
                &*blocks.add(ref_offset),
                blocks.add(curr_offset as usize).cast::<u64>(),
            );
        }
    }
}

/// Calculates the final hash and returns it.
pub fn finalize(context: &Context, memory: &Memory) -> Vec<u8> {
    let mut blockhash = memory[context.lane_length - 1].clone();
    for l in 1..context.config.lanes {
        let last_block_in_lane = l * context.lane_length + (context.lane_length - 1);
        blockhash ^= &memory[last_block_in_lane];
    }

    let mut hash = vec![0u8; context.config.hash_length as usize];
    hprime(hash.as_mut_slice(), blockhash.as_u8());
    hash
}

fn blake2b(out: &mut [u8], input: &[&[u8]]) {
    let mut blake = Params::new().hash_length(out.len()).to_state();
    for slice in input {
        blake.update(slice);
    }
    out.copy_from_slice(blake.finalize().as_bytes());
}

fn f_bla_mka(x: u64, y: u64) -> u64 {
    let m = 0xFFFF_FFFFu64;
    let xy = (x & m) * (y & m);
    x.wrapping_add(y.wrapping_add(xy.wrapping_add(xy)))
}

#[inline(always)]
fn fill_block(prev_block: &Block, ref_block: &Block, next_block: &mut Block, with_xor: bool) {
    // SAFETY: `next_words` addresses all 128 initialized words of
    // `next_block`. The raw implementation only skips reading those words
    // when `with_xor` is false; otherwise this safe wrapper supplies a fully
    // initialized block.
    unsafe {
        if with_xor {
            fill_block_raw::<true>(prev_block, ref_block, next_block.as_mut_word_ptr())
        } else {
            fill_block_raw::<false>(prev_block, ref_block, next_block.as_mut_word_ptr())
        }
    }
}

/// Compresses one block into a raw 128-word destination.
///
/// # Safety
///
/// `next_words` must point to writable storage for 128 aligned `u64`s. If
/// `with_xor` is true, all 128 words must already be initialized. It may not
/// overlap either input block. On return, all 128 destination words are
/// initialized regardless of `with_xor`.
#[inline(always)]
unsafe fn fill_block_raw<const WITH_XOR: bool>(
    prev_block: &Block,
    ref_block: &Block,
    next_words: *mut u64,
) {
    // Build each 16-word column of R = ref_block XOR prev_block directly in
    // registers, accumulate R into the destination, permute it, and only then
    // write the initialized P(R) column to scratch. This removes an otherwise
    // redundant full scratch write/read pair and avoids clearing 1 KiB that is
    // overwritten before any read. Argon2's result is equivalently:
    //
    //     next = P(R) XOR R [XOR old_next on pass > 0]
    //
    // so the destination holds R (or old_next XOR R) from the outset.
    let mut block_r_words = [MaybeUninit::<u64>::uninit(); common::QWORDS_IN_BLOCK];

    // Apply Blake2 on columns of 64-bit words: (0,1,...,15), then
    // (16,17,...31), and finally (112,113,...127).
    for i in 0..8 {
        let base = 16 * i;
        let mut v0 = ref_block[base] ^ prev_block[base];
        let mut v1 = ref_block[base + 1] ^ prev_block[base + 1];
        let mut v2 = ref_block[base + 2] ^ prev_block[base + 2];
        let mut v3 = ref_block[base + 3] ^ prev_block[base + 3];
        let mut v4 = ref_block[base + 4] ^ prev_block[base + 4];
        let mut v5 = ref_block[base + 5] ^ prev_block[base + 5];
        let mut v6 = ref_block[base + 6] ^ prev_block[base + 6];
        let mut v7 = ref_block[base + 7] ^ prev_block[base + 7];
        let mut v8 = ref_block[base + 8] ^ prev_block[base + 8];
        let mut v9 = ref_block[base + 9] ^ prev_block[base + 9];
        let mut v10 = ref_block[base + 10] ^ prev_block[base + 10];
        let mut v11 = ref_block[base + 11] ^ prev_block[base + 11];
        let mut v12 = ref_block[base + 12] ^ prev_block[base + 12];
        let mut v13 = ref_block[base + 13] ^ prev_block[base + 13];
        let mut v14 = ref_block[base + 14] ^ prev_block[base + 14];
        let mut v15 = ref_block[base + 15] ^ prev_block[base + 15];

        if WITH_XOR {
            unsafe {
                *next_words.add(base) ^= v0;
                *next_words.add(base + 1) ^= v1;
                *next_words.add(base + 2) ^= v2;
                *next_words.add(base + 3) ^= v3;
                *next_words.add(base + 4) ^= v4;
                *next_words.add(base + 5) ^= v5;
                *next_words.add(base + 6) ^= v6;
                *next_words.add(base + 7) ^= v7;
                *next_words.add(base + 8) ^= v8;
                *next_words.add(base + 9) ^= v9;
                *next_words.add(base + 10) ^= v10;
                *next_words.add(base + 11) ^= v11;
                *next_words.add(base + 12) ^= v12;
                *next_words.add(base + 13) ^= v13;
                *next_words.add(base + 14) ^= v14;
                *next_words.add(base + 15) ^= v15;
            }
        } else {
            unsafe {
                next_words.add(base).write(v0);
                next_words.add(base + 1).write(v1);
                next_words.add(base + 2).write(v2);
                next_words.add(base + 3).write(v3);
                next_words.add(base + 4).write(v4);
                next_words.add(base + 5).write(v5);
                next_words.add(base + 6).write(v6);
                next_words.add(base + 7).write(v7);
                next_words.add(base + 8).write(v8);
                next_words.add(base + 9).write(v9);
                next_words.add(base + 10).write(v10);
                next_words.add(base + 11).write(v11);
                next_words.add(base + 12).write(v12);
                next_words.add(base + 13).write(v13);
                next_words.add(base + 14).write(v14);
                next_words.add(base + 15).write(v15);
            }
        }

        p(
            &mut v0, &mut v1, &mut v2, &mut v3, &mut v4, &mut v5, &mut v6, &mut v7, &mut v8,
            &mut v9, &mut v10, &mut v11, &mut v12, &mut v13, &mut v14, &mut v15,
        );

        block_r_words[base].write(v0);
        block_r_words[base + 1].write(v1);
        block_r_words[base + 2].write(v2);
        block_r_words[base + 3].write(v3);
        block_r_words[base + 4].write(v4);
        block_r_words[base + 5].write(v5);
        block_r_words[base + 6].write(v6);
        block_r_words[base + 7].write(v7);
        block_r_words[base + 8].write(v8);
        block_r_words[base + 9].write(v9);
        block_r_words[base + 10].write(v10);
        block_r_words[base + 11].write(v11);
        block_r_words[base + 12].write(v12);
        block_r_words[base + 13].write(v13);
        block_r_words[base + 14].write(v14);
        block_r_words[base + 15].write(v15);
    }

    // Apply Blake2 on rows of 64-bit words: (0,1,16,17,...112,113), then
    // (2,3,18,19,...,114,115).. finally (14,15,30,31,...,126,127)
    for i in 0..8 {
        // All 128 scratch elements were initialized exactly once by the
        // column loop. Each row index below is in 0..128 and each initialized
        // `u64` is copied out without constructing an intermediate block.
        let mut v0 = unsafe { block_r_words[2 * i].assume_init() };
        let mut v1 = unsafe { block_r_words[2 * i + 1].assume_init() };
        let mut v2 = unsafe { block_r_words[2 * i + 16].assume_init() };
        let mut v3 = unsafe { block_r_words[2 * i + 17].assume_init() };
        let mut v4 = unsafe { block_r_words[2 * i + 32].assume_init() };
        let mut v5 = unsafe { block_r_words[2 * i + 33].assume_init() };
        let mut v6 = unsafe { block_r_words[2 * i + 48].assume_init() };
        let mut v7 = unsafe { block_r_words[2 * i + 49].assume_init() };
        let mut v8 = unsafe { block_r_words[2 * i + 64].assume_init() };
        let mut v9 = unsafe { block_r_words[2 * i + 65].assume_init() };
        let mut v10 = unsafe { block_r_words[2 * i + 80].assume_init() };
        let mut v11 = unsafe { block_r_words[2 * i + 81].assume_init() };
        let mut v12 = unsafe { block_r_words[2 * i + 96].assume_init() };
        let mut v13 = unsafe { block_r_words[2 * i + 97].assume_init() };
        let mut v14 = unsafe { block_r_words[2 * i + 112].assume_init() };
        let mut v15 = unsafe { block_r_words[2 * i + 113].assume_init() };

        p(
            &mut v0, &mut v1, &mut v2, &mut v3, &mut v4, &mut v5, &mut v6, &mut v7, &mut v8,
            &mut v9, &mut v10, &mut v11, &mut v12, &mut v13, &mut v14, &mut v15,
        );

        // This is the final permutation pass. Fold P(R) into the destination
        // immediately instead of writing all 128 words back to `block_r` and
        // traversing that scratch block once more solely for the final XOR.
        unsafe {
            *next_words.add(2 * i) ^= v0;
            *next_words.add(2 * i + 1) ^= v1;
            *next_words.add(2 * i + 16) ^= v2;
            *next_words.add(2 * i + 17) ^= v3;
            *next_words.add(2 * i + 32) ^= v4;
            *next_words.add(2 * i + 33) ^= v5;
            *next_words.add(2 * i + 48) ^= v6;
            *next_words.add(2 * i + 49) ^= v7;
            *next_words.add(2 * i + 64) ^= v8;
            *next_words.add(2 * i + 65) ^= v9;
            *next_words.add(2 * i + 80) ^= v10;
            *next_words.add(2 * i + 81) ^= v11;
            *next_words.add(2 * i + 96) ^= v12;
            *next_words.add(2 * i + 97) ^= v13;
            *next_words.add(2 * i + 112) ^= v14;
            *next_words.add(2 * i + 113) ^= v15;
        }
    }
}

/// Returns a block from either side of a mutably borrowed current block.
///
/// Argon2's reference-area rules guarantee that neither the previous block nor
/// the selected reference block is the block currently being filled. Keeping
/// the split explicit lets Rust enforce disjointness without cloning the 1 KiB
/// current block on every iteration.
#[inline(always)]
fn block_around_current<'a>(
    before: &'a [Block],
    after: &'a [Block],
    current: usize,
    index: usize,
) -> &'a Block {
    debug_assert_ne!(index, current);
    if index < current {
        &before[index]
    } else {
        &after[index - current - 1]
    }
}

#[inline(always)]
fn fill_block_in_memory(
    memory: &mut Memory,
    prev_offset: u32,
    ref_offset: u64,
    curr_offset: u32,
    with_xor: bool,
) {
    let current = curr_offset as usize;
    let (before, current_and_after) = memory.blocks.split_at_mut(current);
    let (next_block, after) = current_and_after
        .split_first_mut()
        .expect("Argon2 current block offset is within memory");
    let prev_block = block_around_current(before, after, current, prev_offset as usize);
    let ref_block = block_around_current(before, after, current, ref_offset as usize);
    fill_block(prev_block, ref_block, next_block, with_xor);
}

fn fill_first_blocks(context: &Context, memory: &mut Memory, h0: &mut [u8]) {
    for lane in 0..context.config.lanes {
        let start = common::PREHASH_DIGEST_LENGTH;
        // H'(H0||0||i)
        h0[start..(start + 4)].clone_from_slice(&u32::to_le_bytes(0));
        h0[(start + 4)..(start + 8)].clone_from_slice(&u32::to_le_bytes(lane));
        hprime(memory[(lane, 0)].as_u8_mut(), &h0);

        // H'(H0||1||i)
        h0[start..(start + 4)].clone_from_slice(&u32::to_le_bytes(1));
        hprime(memory[(lane, 1)].as_u8_mut(), &h0);
    }
}

fn fill_memory_blocks_st(context: &Context, memory: &mut Memory) {
    for p in 0..context.config.time_cost {
        for s in 0..common::SYNC_POINTS {
            for l in 0..context.config.lanes {
                let position = Position {
                    pass: p,
                    lane: l,
                    slice: s,
                    index: 0,
                };
                fill_segment(context, &position, memory);
            }
        }
    }
}

fn fill_segment(context: &Context, position: &Position, memory: &mut Memory) {
    let mut position = position.clone();
    let data_independent_addressing = (context.config.variant == Variant::Argon2i)
        || (context.config.variant == Variant::Argon2id && position.pass == 0)
            && (position.slice < (common::SYNC_POINTS / 2));
    let zero_block = Block::zero();
    let mut input_block = Block::zero();
    let mut address_block = Block::zero();

    if data_independent_addressing {
        input_block[0] = position.pass as u64;
        input_block[1] = position.lane as u64;
        input_block[2] = position.slice as u64;
        input_block[3] = context.memory_blocks as u64;
        input_block[4] = context.config.time_cost as u64;
        input_block[5] = context.config.variant.as_u64();
    }

    let mut starting_index = 0u32;

    if position.pass == 0 && position.slice == 0 {
        starting_index = 2;

        // Don't forget to generate the first block of addresses:
        if data_independent_addressing {
            next_addresses(&mut address_block, &mut input_block, &zero_block);
        }
    }

    let mut curr_offset = (position.lane * context.lane_length)
        + (position.slice * context.segment_length)
        + starting_index;

    let mut prev_offset = if curr_offset % context.lane_length == 0 {
        // Last block in this lane
        curr_offset + context.lane_length - 1
    } else {
        curr_offset - 1
    };

    let mut pseudo_rand;
    for i in starting_index..context.segment_length {
        // 1.1 Rotating prev_offset if needed
        if curr_offset % context.lane_length == 1 {
            prev_offset = curr_offset - 1;
        }

        // 1.2 Computing the index of the reference block
        // 1.2.1 Taking pseudo-random value from the previous block
        if data_independent_addressing {
            if i % common::ADDRESSES_IN_BLOCK == 0 {
                next_addresses(&mut address_block, &mut input_block, &zero_block);
            }
            pseudo_rand = address_block[(i % common::ADDRESSES_IN_BLOCK) as usize];
        } else {
            pseudo_rand = memory[prev_offset][0];
        }

        // 1.2.2 Computing the lane of the reference block
        // If (position.pass == 0) && (position.slice == 0): can not reference other lanes yet
        let ref_lane = if (position.pass == 0) && (position.slice == 0) {
            position.lane as u64
        } else {
            (pseudo_rand >> 32) % context.config.lanes as u64
        };

        // 1.2.3 Computing the number of possible reference block within the lane.
        position.index = i;
        let pseudo_rand_u32 = (pseudo_rand & 0xFFFF_FFFF) as u32;
        let same_lane = ref_lane == (position.lane as u64);
        let ref_index = index_alpha(context, &position, pseudo_rand_u32, same_lane);

        // 2 Creating a new block
        let index = context.lane_length as u64 * ref_lane + ref_index as u64;
        let with_xor = context.config.version != Version::Version10 && position.pass != 0;
        fill_block_in_memory(memory, prev_offset, index, curr_offset, with_xor);
        curr_offset += 1;
        prev_offset += 1;
    }
}

fn g(a: &mut u64, b: &mut u64, c: &mut u64, d: &mut u64) {
    *a = f_bla_mka(*a, *b);
    *d = rotr64(*d ^ *a, 32);
    *c = f_bla_mka(*c, *d);
    *b = rotr64(*b ^ *c, 24);
    *a = f_bla_mka(*a, *b);
    *d = rotr64(*d ^ *a, 16);
    *c = f_bla_mka(*c, *d);
    *b = rotr64(*b ^ *c, 63);
}

fn h0(context: &Context) -> [u8; common::PREHASH_SEED_LENGTH] {
    let input = [
        &u32::to_le_bytes(context.config.lanes),
        &u32::to_le_bytes(context.config.hash_length),
        &u32::to_le_bytes(context.config.mem_cost),
        &u32::to_le_bytes(context.config.time_cost),
        &u32::to_le_bytes(context.config.version.as_u32()),
        &u32::to_le_bytes(context.config.variant.as_u32()),
        &len_as_32le(context.pwd),
        context.pwd,
        &len_as_32le(context.salt),
        context.salt,
        &len_as_32le(context.config.secret),
        context.config.secret,
        &len_as_32le(context.config.ad),
        context.config.ad,
    ];
    let mut out = [0u8; common::PREHASH_SEED_LENGTH];
    blake2b(&mut out[0..common::PREHASH_DIGEST_LENGTH], &input);
    out
}

fn hprime(out: &mut [u8], input: &[u8]) {
    let out_len = out.len();
    if out_len <= common::BLAKE2B_OUT_LENGTH {
        blake2b(out, &[&u32::to_le_bytes(out_len as u32), input]);
    } else {
        let ai_len = 32;
        let mut out_buffer = [0u8; common::BLAKE2B_OUT_LENGTH];
        let mut in_buffer = [0u8; common::BLAKE2B_OUT_LENGTH];
        blake2b(&mut out_buffer, &[&u32::to_le_bytes(out_len as u32), input]);
        out[0..ai_len].clone_from_slice(&out_buffer[0..ai_len]);
        let mut out_pos = ai_len;
        let mut to_produce = out_len - ai_len;

        while to_produce > common::BLAKE2B_OUT_LENGTH {
            in_buffer.clone_from_slice(&out_buffer);
            blake2b(&mut out_buffer, &[&in_buffer]);
            out[out_pos..out_pos + ai_len].clone_from_slice(&out_buffer[0..ai_len]);
            out_pos += ai_len;
            to_produce -= ai_len;
        }
        blake2b(&mut out[out_pos..out_len], &[&out_buffer]);
    }
}

fn index_alpha(context: &Context, position: &Position, pseudo_rand: u32, same_lane: bool) -> u32 {
    // Pass 0:
    // - This lane: all already finished segments plus already constructed blocks in this segment
    // - Other lanes: all already finished segments
    // Pass 1+:
    // - This lane: (SYNC_POINTS - 1) last segments plus already constructed blocks in this segment
    // - Other lanes : (SYNC_POINTS - 1) last segments
    let reference_area_size: u32 = if position.pass == 0 {
        // First pass
        if position.slice == 0 {
            // First slice
            position.index - 1
        } else if same_lane {
            // The same lane => add current segment
            position.slice * context.segment_length + position.index - 1
        } else if position.index == 0 {
            position.slice * context.segment_length - 1
        } else {
            position.slice * context.segment_length
        }
    } else {
        // Second pass
        if same_lane {
            context.lane_length - context.segment_length + position.index - 1
        } else if position.index == 0 {
            context.lane_length - context.segment_length - 1
        } else {
            context.lane_length - context.segment_length
        }
    };
    let reference_area_size = reference_area_size as u64;
    let mut relative_position = pseudo_rand as u64;
    relative_position = (relative_position * relative_position) >> 32;
    relative_position = reference_area_size - 1 - ((reference_area_size * relative_position) >> 32);

    // 1.2.5 Computing starting position
    let start_position: u32 = if position.pass != 0 {
        if position.slice == common::SYNC_POINTS - 1 {
            0u32
        } else {
            (position.slice + 1) * context.segment_length
        }
    } else {
        0u32
    };
    let start_position = start_position as u64;

    // 1.2.6. Computing absolute position
    ((start_position + relative_position) % context.lane_length as u64) as u32
}

fn len_as_32le(slice: &[u8]) -> [u8; 4] {
    u32::to_le_bytes(slice.len() as u32)
}

fn next_addresses(address_block: &mut Block, input_block: &mut Block, zero_block: &Block) {
    input_block[6] += 1;
    fill_block(zero_block, input_block, address_block, false);
    fill_block(zero_block, &address_block.clone(), address_block, false);
}

#[inline(always)]
fn p(
    v0: &mut u64,
    v1: &mut u64,
    v2: &mut u64,
    v3: &mut u64,
    v4: &mut u64,
    v5: &mut u64,
    v6: &mut u64,
    v7: &mut u64,
    v8: &mut u64,
    v9: &mut u64,
    v10: &mut u64,
    v11: &mut u64,
    v12: &mut u64,
    v13: &mut u64,
    v14: &mut u64,
    v15: &mut u64,
) {
    g(v0, v4, v8, v12);
    g(v1, v5, v9, v13);
    g(v2, v6, v10, v14);
    g(v3, v7, v11, v15);
    g(v0, v5, v10, v15);
    g(v1, v6, v11, v12);
    g(v2, v7, v8, v13);
    g(v3, v4, v9, v14);
}

fn rotr64(w: u64, c: u32) -> u64 {
    (w >> c) | (w << (64 - c))
}

#[cfg(test)]
mod randomx_specialization_tests {
    use super::*;
    use crate::config::Config;

    const LANE_LENGTH: u32 = 262_144;
    const SEGMENT_LENGTH: u32 = 65_536;

    fn specialized_index(pass: u32, slice: u32, index: u32, pseudo_rand: u32) -> u32 {
        let reference_area_size = if pass == 0 {
            slice * SEGMENT_LENGTH + index - 1
        } else {
            LANE_LENGTH - SEGMENT_LENGTH + index - 1
        } as u64;
        let mut relative_position = pseudo_rand as u64;
        relative_position = (relative_position * relative_position) >> 32;
        relative_position =
            reference_area_size - 1 - ((reference_area_size * relative_position) >> 32);
        let start_position = if pass == 0 || slice == common::SYNC_POINTS - 1 {
            0
        } else {
            (slice + 1) * SEGMENT_LENGTH
        } as u64;
        ((start_position + relative_position) & (LANE_LENGTH - 1) as u64) as u32
    }

    #[test]
    fn randomx_reference_formula_matches_generic_boundaries() {
        let config = Config {
            ad: &[],
            hash_length: 0,
            lanes: 1,
            mem_cost: LANE_LENGTH,
            secret: &[],
            time_cost: 3,
            variant: Variant::Argon2d,
            version: Version::Version13,
        };
        let context = Context {
            config,
            memory_blocks: LANE_LENGTH,
            pwd: &[],
            salt: b"RandomX\x03",
            lane_length: LANE_LENGTH,
            segment_length: SEGMENT_LENGTH,
        };
        let pseudo_random_values = [0, 1, u32::MAX, 0x8000_0000, 0x1234_5678];

        for pass in 0..3 {
            for slice in 0..common::SYNC_POINTS {
                let first = if pass == 0 && slice == 0 { 2 } else { 0 };
                let indices = [first, first + 1, 17.max(first), SEGMENT_LENGTH - 1];

                for index in indices {
                    for pseudo_rand in pseudo_random_values {
                        let position = Position {
                            pass,
                            lane: 0,
                            slice,
                            index,
                        };
                        let generic = index_alpha(&context, &position, pseudo_rand, true);
                        let specialized = specialized_index(pass, slice, index, pseudo_rand);
                        assert_eq!(specialized, generic);
                        assert!(specialized < LANE_LENGTH);

                        let current = slice * SEGMENT_LENGTH + index;
                        assert_ne!(specialized, current);
                        if pass == 0 {
                            assert!(specialized < current);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fused_scratch_index_maps_are_exact_partitions() {
        let mut column_writes = [0u8; common::QWORDS_IN_BLOCK];
        for column in 0..8 {
            let base = 16 * column;
            for offset in 0..16 {
                column_writes[base + offset] += 1;
            }
        }
        assert!(column_writes.iter().all(|&count| count == 1));

        let mut row_reads = [0u8; common::QWORDS_IN_BLOCK];
        let pair_offsets = [0usize, 16, 32, 48, 64, 80, 96, 112];
        for row in 0..8 {
            for pair_offset in pair_offsets {
                row_reads[2 * row + pair_offset] += 1;
                row_reads[2 * row + pair_offset + 1] += 1;
            }
        }
        assert!(row_reads.iter().all(|&count| count == 1));
    }
}
