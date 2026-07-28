extern crate argon2;

use std::sync::{Arc, RwLock};
use std::time::Instant;
use std::{mem::align_of, mem::size_of};

use self::argon2::block::Block;

use super::byte_string;
use super::superscalar::{Blake2Generator, ScProgram};

const RANDOMX_ARGON_LANES: u32 = 1;
const RANDOMX_ARGON_MEMORY: u32 = 262144;
const RANDOMX_ARGON_SALT: &[u8; 8] = b"RandomX\x03";
const RANDOMX_ARGON_ITERATIONS: u32 = 3;
const RANDOMX_CACHE_ACCESSES: usize = 8;

const ARGON2_SYNC_POINTS: u32 = 4;
const ARGON_BLOCK_SIZE: u32 = 1024;
const ARGON_BLOCK_WORDS: usize = ARGON_BLOCK_SIZE as usize / size_of::<u64>();

pub const CACHE_LINE_SIZE: u64 = 64;
pub const DATASET_ITEM_COUNT: usize = (2147483648 + 33554368) / 64; //34.078.719
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

pub type DatasetItemInitializer = fn(&SeedMemory, u64) -> [u64; 8];

impl SeedMemory {
    pub fn no_memory() -> SeedMemory {
        SeedMemory {
            blocks: Box::new([]),
            programs: Vec::with_capacity(0),
        }
    }

    /// Creates a new initialised seed memory.
    pub fn new_initialised(key: &[u8]) -> SeedMemory {
        let context = create_argon_context(key);
        let mem = argon2::core::initialize_memory_randomx(&context);

        let mut programs = Vec::with_capacity(RANDOMX_CACHE_ACCESSES);
        let mut generator = Blake2Generator::new(key, 0);
        for _ in 0..RANDOMX_CACHE_ACCESSES {
            programs.push(ScProgram::generate(&mut generator));
        }

        SeedMemory {
            blocks: mem.blocks,
            programs,
        }
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn program_count(&self) -> usize {
        self.programs.len()
    }

    /// Mixes one cache line into a dataset register file. Fixed-epoch
    /// straight-line superscalar code uses this after each compiled program.
    #[doc(hidden)]
    #[inline(always)]
    pub fn xor_cache_line(&self, reg_value: u64, registers: &mut [u64; 8]) {
        for (index, register) in registers.iter_mut().enumerate() {
            *register ^= mix_block_value(self, reg_value, index);
        }
    }
}

fn create_argon_context<'a>(key: &'a [u8]) -> argon2::context::Context<'a> {
    let segment_length = RANDOMX_ARGON_MEMORY / (RANDOMX_ARGON_LANES * ARGON2_SYNC_POINTS);
    let config = argon2::config::Config {
        ad: &[],
        hash_length: 0,
        lanes: RANDOMX_ARGON_LANES,
        mem_cost: RANDOMX_ARGON_MEMORY,
        secret: &[],
        time_cost: RANDOMX_ARGON_ITERATIONS,
        variant: argon2::Variant::Argon2d,
        version: argon2::Version::Version13,
    };
    argon2::context::Context {
        config,
        memory_blocks: RANDOMX_ARGON_MEMORY,
        pwd: key,
        salt: RANDOMX_ARGON_SALT,
        lane_length: segment_length * ARGON2_SYNC_POINTS,
        segment_length,
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

#[derive(Clone)]
pub struct VmMemoryAllocator {
    pub vm_memory_seed: String,
    pub vm_memory: Arc<VmMemory>,
}

impl VmMemoryAllocator {
    pub fn initial() -> VmMemoryAllocator {
        VmMemoryAllocator {
            vm_memory_seed: "".to_string(),
            vm_memory: Arc::new(VmMemory::no_memory()),
        }
    }

    pub fn reallocate(&mut self, seed: String) {
        if seed != self.vm_memory_seed {
            let mem_init_start = Instant::now();
            self.vm_memory = Arc::new(VmMemory::full(&byte_string::string_to_u8_array(&seed)));
            self.vm_memory_seed = seed;
            info!(
                "memory init took {}ms with seed_hash: {}",
                mem_init_start.elapsed().as_millis(),
                self.vm_memory_seed,
            );
        }
    }
}

pub struct VmMemory {
    pub seed_memory: SeedMemory,
    pub dataset_memory: RwLock<Vec<Option<[u64; 8]>>>,
    pub cache: bool,
    dataset_item_initializer: DatasetItemInitializer,
}

impl VmMemory {
    //only useful for testing
    pub fn no_memory() -> VmMemory {
        VmMemory {
            seed_memory: SeedMemory::no_memory(),
            cache: false,
            dataset_memory: RwLock::new(Vec::with_capacity(0)),
            dataset_item_initializer: init_dataset_item,
        }
    }

    pub fn light(key: &[u8]) -> VmMemory {
        Self::light_with_dataset_item_initializer(key, init_dataset_item)
    }

    pub fn light_with_dataset_item_initializer(
        key: &[u8],
        dataset_item_initializer: DatasetItemInitializer,
    ) -> VmMemory {
        VmMemory {
            seed_memory: SeedMemory::new_initialised(key),
            cache: false,
            dataset_memory: RwLock::new(Vec::with_capacity(0)),
            dataset_item_initializer,
        }
    }
    pub fn full(key: &[u8]) -> VmMemory {
        let seed_mem = SeedMemory::new_initialised(key);
        let mem = vec![None; DATASET_ITEM_COUNT];
        VmMemory {
            seed_memory: seed_mem,
            cache: true,
            dataset_memory: RwLock::new(mem),
            dataset_item_initializer: init_dataset_item,
        }
    }

    pub fn dataset_read(&self, offset: u64, reg: &mut [u64; 8]) {
        let item_num = offset / CACHE_LINE_SIZE;

        if self.cache {
            {
                let mem = self.dataset_memory.read().unwrap();
                let rl_cached = &mem[item_num as usize];
                if let Some(rl) = rl_cached {
                    for i in 0..8 {
                        reg[i] ^= rl[i];
                    }
                    return;
                }
            }
            {
                let rl = init_dataset_item(&self.seed_memory, item_num);
                let mut mem_mut = self.dataset_memory.write().unwrap();
                mem_mut[item_num as usize] = Some(rl);
                for i in 0..8 {
                    reg[i] ^= rl[i];
                }
            }
        } else {
            let rl = init_dataset_item(&self.seed_memory, item_num);
            for i in 0..8 {
                reg[i] ^= rl[i];
            }
        }
    }

    /// Derives and mixes a dataset item without consulting the optional full
    /// dataset cache. The compact verifier is deliberately light-mode only,
    /// so keeping that hot path separate avoids carrying the locking and
    /// cache-population branches through every VM iteration.
    #[inline(always)]
    pub fn dataset_read_light(&self, offset: u64, reg: &mut [u64; 8]) {
        debug_assert!(!self.cache);
        let item_num = offset / CACHE_LINE_SIZE;
        let rl = (self.dataset_item_initializer)(&self.seed_memory, item_num);
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
