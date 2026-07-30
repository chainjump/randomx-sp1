use std::{mem::align_of, mem::size_of};

use randomx_sp1_argon2::Block;

use super::superscalar::{Blake2Generator, ScProgram};

const RANDOMX_ARGON_MEMORY: u32 = 262144;
const RANDOMX_CACHE_ACCESSES: usize = 8;

const ARGON_BLOCK_SIZE: u32 = 1024;
const ARGON_BLOCK_WORDS: usize = ARGON_BLOCK_SIZE as usize / size_of::<u64>();

pub const CACHE_LINE_SIZE: u64 = 64;
const CACHE_LINE_WORDS: usize = CACHE_LINE_SIZE as usize / size_of::<u64>();
const CACHE_LINE_COUNT: u64 =
    (RANDOMX_ARGON_MEMORY as u64 * ARGON_BLOCK_SIZE as u64) / CACHE_LINE_SIZE;
const CACHE_LINE_MASK: u64 = CACHE_LINE_COUNT - 1;
const CACHE_WORD_COUNT: usize = RANDOMX_ARGON_MEMORY as usize * ARGON_BLOCK_WORDS;

const _: () = {
    assert!(size_of::<Block>() == ARGON_BLOCK_SIZE as usize);
    assert!(align_of::<Block>() == align_of::<u64>());
    assert!(CACHE_LINE_SIZE as usize == CACHE_LINE_WORDS * size_of::<u64>());
    assert!(CACHE_LINE_COUNT.is_power_of_two());
    assert!(CACHE_WORD_COUNT == CACHE_LINE_COUNT as usize * CACHE_LINE_WORDS);
};

const SUPERSCALAR_MUL_0: u64 = 6364136223846793005;
const SUPERSCALAR_ADD_1: u64 = 9298411001130361340;
const SUPERSCALAR_ADD_2: u64 = 12065312585734608966;
const SUPERSCALAR_ADD_3: u64 = 9306329213124626780;
const SUPERSCALAR_ADD_4: u64 = 5281919268842080866;
const SUPERSCALAR_ADD_5: u64 = 10536153434571861004;
const SUPERSCALAR_ADD_6: u64 = 3398623926847679864;
const SUPERSCALAR_ADD_7: u64 = 9549104520008361294;

//256MiB, always used, named randomx_cache in the reference implementation
pub struct SeedMemory {
    blocks: Box<[Block]>,
    programs: Vec<ScProgram<'static>>,
}

impl SeedMemory {
    pub fn no_memory() -> SeedMemory {
        SeedMemory {
            blocks: Box::new([]),
            programs: Vec::with_capacity(0),
        }
    }

    /// Creates a new initialised seed memory.
    pub fn new_initialised(key: &[u8]) -> SeedMemory {
        let blocks = randomx_sp1_argon2::initialize_randomx(key);

        let mut programs = Vec::with_capacity(RANDOMX_CACHE_ACCESSES);
        let mut generator = Blake2Generator::new(key, 0);
        for _ in 0..RANDOMX_CACHE_ACCESSES {
            programs.push(ScProgram::generate(&mut generator));
        }

        SeedMemory { blocks, programs }
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn program_count(&self) -> usize {
        self.programs.len()
    }
}

#[inline(always)]
fn mix_word_index(reg_value: u64, r: usize) -> usize {
    debug_assert!(r < CACHE_LINE_WORDS);
    (reg_value & CACHE_LINE_MASK) as usize * CACHE_LINE_WORDS + r
}

#[inline(always)]
fn mix_block_value(seed_mem: &SeedMemory, reg_value: u64, r: usize) -> u64 {
    debug_assert_eq!(seed_mem.blocks.len() * ARGON_BLOCK_WORDS, CACHE_WORD_COUNT);
    let word_index = mix_word_index(reg_value, r);
    debug_assert!(word_index < CACHE_WORD_COUNT);

    // SAFETY: `SeedMemory`'s fields are private, and `new_initialised` always
    // installs exactly `RANDOMX_ARGON_MEMORY` blocks before generating the
    // nonempty program list. `no_memory` has no programs, so this helper is
    // never called for its empty block slice. `Block` is `repr(transparent)`
    // over `[u64; 128]`; the size/alignment assertions above and contiguous
    // boxed-slice layout therefore make the allocation one flat array of
    // `CACHE_WORD_COUNT` valid `u64`s. Masking selects one of all cache lines,
    // and `r` selects one of that line's eight words.
    unsafe { *seed_mem.blocks.as_ptr().cast::<u64>().add(word_index) }
}

pub fn init_dataset_item(seed_mem: &SeedMemory, item_num: u64) -> [u64; 8] {
    let mut ds = [0; 8];

    let mut reg_value = item_num;
    ds[0] = (item_num + 1).wrapping_mul(SUPERSCALAR_MUL_0);
    ds[1] = ds[0] ^ SUPERSCALAR_ADD_1;
    ds[2] = ds[0] ^ SUPERSCALAR_ADD_2;
    ds[3] = ds[0] ^ SUPERSCALAR_ADD_3;
    ds[4] = ds[0] ^ SUPERSCALAR_ADD_4;
    ds[5] = ds[0] ^ SUPERSCALAR_ADD_5;
    ds[6] = ds[0] ^ SUPERSCALAR_ADD_6;
    ds[7] = ds[0] ^ SUPERSCALAR_ADD_7;

    for prog in &seed_mem.programs {
        prog.execute(&mut ds);

        for (r, v) in ds.iter_mut().enumerate() {
            let mix_value = mix_block_value(seed_mem, reg_value, r);
            *v ^= mix_value;
        }
        reg_value = prog.address_register(&ds);
    }
    ds
}

pub struct VmMemory {
    pub seed_memory: SeedMemory,
}

impl VmMemory {
    //only useful for testing
    pub fn no_memory() -> VmMemory {
        VmMemory {
            seed_memory: SeedMemory::no_memory(),
        }
    }

    pub fn light(key: &[u8]) -> VmMemory {
        VmMemory {
            seed_memory: SeedMemory::new_initialised(key),
        }
    }

    /// Derives and mixes one dataset item in RandomX light mode.
    #[inline(always)]
    pub fn dataset_read(&self, offset: u64, reg: &mut [u64; 8]) {
        let item_num = offset / CACHE_LINE_SIZE;
        let rl = init_dataset_item(&self.seed_memory, item_num);
        for i in 0..8 {
            reg[i] ^= rl[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattened_mix_index_matches_block_and_word_addressing() {
        let mut values = vec![
            0,
            1,
            CACHE_LINE_WORDS as u64 - 1,
            CACHE_LINE_WORDS as u64,
            CACHE_LINE_COUNT - 1,
            CACHE_LINE_COUNT,
            u32::MAX as u64,
            u64::MAX,
        ];
        let mut state = 0x243f_6a88_85a3_08d3u64;
        for _ in 0..10_000 {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
            values.push(state);
        }

        for reg_value in values {
            for r in 0..CACHE_LINE_WORDS {
                let byte_offset = ((reg_value & CACHE_LINE_MASK) * CACHE_LINE_SIZE)
                    + size_of::<u64>() as u64 * r as u64;
                let block_index = byte_offset / ARGON_BLOCK_SIZE as u64;
                let block_word =
                    (byte_offset - block_index * ARGON_BLOCK_SIZE as u64) / size_of::<u64>() as u64;
                let reference = block_index as usize * ARGON_BLOCK_WORDS + block_word as usize;
                assert_eq!(mix_word_index(reg_value, r), reference);
                assert!(reference < CACHE_WORD_COUNT);
            }
        }
    }
}
