#![deny(missing_docs)]

//! SP1-optimized RandomX hashing.
//!
//! [`hash`] is the stable public entry point. The library derives the cache,
//! dataset items, and RandomX programs from caller-supplied inputs and executes
//! every VM iteration. No RandomX input is embedded in the library.

use std::{mem::size_of, sync::Arc};

use blake2b_simd::{blake2b, Hash, Params};
use randomx_softfp::{add2, div2, mul2, sqrt2, sub2, RoundingMode};
use randomx_sp1_core::common::{mulh, randomx_reciprocal, smulh, u64_from_i32_imm};
use randomx_sp1_core::hash::{gen_program_aes_4rx4, hash_aes_1rx4};
use randomx_sp1_core::m128::{m128d, m128i};
use randomx_sp1_core::memory::CACHE_LINE_SIZE;
#[cfg(feature = "differential-audit")]
use randomx_sp1_core::program::{Opcode as ReferenceOpcode, Program as ReferenceProgram};
use randomx_sp1_core::vm::Vm;
use randomx_sp1_core::{new_vm, VmMemory};

const MAX_REG: usize = 8;
const MAX_FLOAT_REG: usize = 4;
const PROGRAM_COUNT: usize = 8;
const PROGRAM_SIZE: i32 = 256;
const PROGRAM_ITERATIONS: usize = 2048;
const DATASET_BASE_SIZE: usize = 2_147_483_648;
const DATASET_ITEM_SIZE: usize = 64;
const DATASET_EXTRA_SIZE: usize = 33_554_368;
const DATASET_EXTRA_ITEMS: usize = DATASET_EXTRA_SIZE / DATASET_ITEM_SIZE;
const HASH_SIZE: usize = 32;

const SCRATCHPAD_L1_MASK: u64 = 0x3ff8;
const SCRATCHPAD_L2_MASK: u64 = 0x3fff8;
const SCRATCHPAD_L3_MASK: u64 = 0x1ffff8;
const SCRATCHPAD_L3_MASK_U32: u32 = 0x1fffc0;
const SCRATCHPAD_WORDS: usize = 262_144;
const CACHE_LINE_ALIGN_MASK: u64 = ((DATASET_BASE_SIZE - 1) & !(DATASET_ITEM_SIZE - 1)) as u64;

const MANTISSA_SIZE: u64 = 52;
const MANTISSA_MASK: u64 = (1 << MANTISSA_SIZE) - 1;
const EXPONENT_SIZE: u64 = 11;
const EXPONENT_BIAS: u64 = 1023;
const EXPONENT_MASK: u64 = (1 << EXPONENT_SIZE) - 1;
const EXPONENT_BITS: u64 = 0x300;
const DYNAMIC_EXPONENT_BITS: u64 = 4;
const STATIC_EXPONENT_BITS: u64 = 4;
const DYNAMIC_MANTISSA_MASK: u64 = (1 << (MANTISSA_SIZE + DYNAMIC_EXPONENT_BITS)) - 1;

const CONDITION_OFFSET: u8 = 8;
const CONDITION_MASK: u64 = (1 << CONDITION_OFFSET) - 1;
const STORE_L3_CONDITION: u8 = 14;
const NO_REG: u8 = u8::MAX;
const MEM_L1: u8 = 0;
const MEM_L2: u8 = 1;
const MEM_L3: u8 = 2;

type Effect = fn(&mut Vm, &CompactInstr);

/// A decoded RandomX instruction with no enums, boxes, or optional operands.
///
/// `imm` is already sign-extended, masked, converted to a reciprocal, or
/// transformed into a branch increment as appropriate for `effect`. The
/// power-of-two stride lets the 4.2-million-dispatch hot path address an
/// instruction with one shift.
#[repr(C)]
#[derive(Clone, Copy)]
struct CompactInstr {
    effect: Effect,
    imm: u64,
    target: i32,
    memory_mask: u32,
    dst: u8,
    src: u8,
    mode: u8,
    _reserved: [u8; 5],
}

const _: () = {
    assert!(size_of::<CompactInstr>() == 32);
    assert!(size_of::<m128d>() == 16);
    assert!((MAX_REG - 1) * size_of::<u64>() <= u8::MAX as usize);
    assert!((MAX_FLOAT_REG - 1) * size_of::<m128d>() <= u8::MAX as usize);
    // Instruction memory operands select one u64 from the full scratchpad.
    assert!((SCRATCHPAD_L3_MASK as usize >> 3) + 1 == SCRATCHPAD_WORDS);
    // Iteration mixing selects an aligned group of eight u64 words.
    assert!((SCRATCHPAD_L3_MASK_U32 as usize >> 3) + MAX_REG == SCRATCHPAD_WORDS);
};

struct CompactProgram {
    entropy: [u64; 16],
    instructions: Vec<CompactInstr>,
}

impl CompactInstr {
    #[inline(always)]
    fn new(effect: Effect, dst: u8, src: u8, imm: u64, mode: u8) -> Self {
        Self {
            effect,
            imm,
            target: 0,
            memory_mask: 0,
            dst: register_byte_offset(dst, size_of::<u64>() as u8),
            src: register_byte_offset(src, size_of::<u64>() as u8),
            mode,
            _reserved: [0; 5],
        }
    }

    #[inline(always)]
    fn new_float(effect: Effect, dst: u8, src: u8, imm: u64, mode: u8) -> Self {
        let mut instr = Self::new(effect, dst, src, imm, mode);
        instr.dst = register_byte_offset(dst, size_of::<m128d>() as u8);
        instr.src = register_byte_offset(src, size_of::<m128d>() as u8);
        instr
    }

    #[inline(always)]
    fn new_memory(effect: Effect, dst: u8, src: u8, imm: u64, mode: u8) -> Self {
        let mut instr = Self::new(effect, dst, src, imm, mode);
        instr.memory_mask = memory_mask(mode);
        instr
    }
}

#[inline(always)]
fn register_byte_offset(index: u8, stride: u8) -> u8 {
    if index == NO_REG {
        NO_REG
    } else {
        debug_assert!((index as usize) < MAX_REG);
        index * stride
    }
}

impl CompactProgram {
    fn from_bytes(bytes: &[m128i]) -> Self {
        assert_eq!(bytes.len(), 136);

        let mut entropy = [0u64; 16];
        for (i, word) in bytes.iter().take(8).enumerate() {
            let (high, low) = word.as_i64();
            entropy[2 * i] = low as u64;
            entropy[2 * i + 1] = high as u64;
        }

        let mut register_usage = [-1i32; MAX_REG];
        let mut instructions = Vec::with_capacity(PROGRAM_SIZE as usize);
        for (i, word) in bytes.iter().skip(8).enumerate() {
            let (high, low) = word.as_i64();
            instructions.push(decode_instruction(low, (2 * i) as i32, &mut register_usage));
            instructions.push(decode_instruction(
                high,
                (2 * i + 1) as i32,
                &mut register_usage,
            ));
        }
        assert_eq!(instructions.len(), PROGRAM_SIZE as usize);

        Self {
            entropy,
            instructions,
        }
    }
}

/// Calculates one complete RandomX hash.
///
/// Both inputs are supplied at runtime. The function constructs the 256 MiB
/// light-mode cache from `key`, derives dataset items on demand, executes all
/// eight RandomX programs, and returns the canonical 32-byte digest.
///
/// This operation is intentionally expensive: each call constructs a new
/// cache and performs one complete RandomX hash. Callers are responsible for
/// imposing any application-specific input-length or resource limits.
#[must_use]
pub fn hash(key: &[u8], blob: &[u8]) -> [u8; HASH_SIZE] {
    let memory = Arc::new(VmMemory::light(key));
    let mut vm = new_vm(memory);
    let digest = calculate_hash_impl(&mut vm, blob);
    let mut output = [0; HASH_SIZE];
    output.copy_from_slice(digest.as_bytes());
    output
}

fn calculate_hash_impl(vm: &mut Vm, input: &[u8]) -> Hash {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    let _host_rounding_mode = randomx_sp1_core::vm::HostRoundingModeGuard::capture();
    // Establish the allocation invariant used by unchecked accesses throughout all eight programs.
    assert_eq!(vm.scratchpad.len(), SCRATCHPAD_WORDS);
    let initial_hash = blake2b(input);
    let mut seed = hash_to_m128i_array(&initial_hash);

    let mut next_seed = vm.init_scratchpad(&seed);
    vm.reset_rounding_mode();

    for _ in 0..(PROGRAM_COUNT - 1) {
        run(vm, &next_seed);
        seed = hash_to_m128i_array(&blake2b(&vm.reg.to_bytes()));
        next_seed = seed;
    }

    run(vm, &next_seed);
    let final_hash = hash_aes_1rx4(&vm.scratchpad);
    vm.reg.a[0] = final_hash[0].as_m128d();
    vm.reg.a[1] = final_hash[1].as_m128d();
    vm.reg.a[2] = final_hash[2].as_m128d();
    vm.reg.a[3] = final_hash[3].as_m128d();

    let mut params = Params::new();
    params.hash_length(HASH_SIZE);
    params.hash(&vm.reg.to_bytes())
}

/// Executes a complete hash with repository-internal VM state.
///
/// This function exists only for audit and profiling binaries and has no
/// compatibility guarantee. Production consumers should use [`hash`].
#[cfg(feature = "audit-internals")]
#[doc(hidden)]
pub fn hash_with_vm_for_audit(vm: &mut Vm, input: &[u8]) -> Hash {
    calculate_hash_impl(vm, input)
}

/// Locates the first state divergence between the reference and optimized
/// interpreters.
/// This is intentionally excluded from verifier builds.
#[cfg(feature = "differential-audit")]
pub fn differential_audit(input: &[u8]) -> Hash {
    let memory = Arc::new(VmMemory::no_memory());
    let mut reference = randomx_sp1_core::new_vm(Arc::clone(&memory));
    let mut compact = randomx_sp1_core::new_vm(memory);
    let initial_hash = blake2b(input);
    let mut seed = hash_to_m128i_array(&initial_hash);

    let reference_next = reference.init_scratchpad(&seed);
    let compact_next = compact.init_scratchpad(&seed);
    assert_eq!(reference_next, compact_next);
    assert_eq!(reference.scratchpad, compact.scratchpad);
    reference.reset_rounding_mode();
    compact.reset_rounding_mode();

    let mut next_seed = reference_next;
    for program_index in 0..PROGRAM_COUNT {
        run_differential(&mut reference, &mut compact, &next_seed, program_index);
        assert_eq!(reference.reg.to_bytes(), compact.reg.to_bytes());
        assert_eq!(reference.scratchpad, compact.scratchpad);
        if program_index + 1 < PROGRAM_COUNT {
            seed = hash_to_m128i_array(&blake2b(&reference.reg.to_bytes()));
            next_seed = seed;
        }
    }

    let final_hash = hash_aes_1rx4(&reference.scratchpad);
    for (index, value) in final_hash.iter().enumerate() {
        reference.reg.a[index] = value.as_m128d();
        compact.reg.a[index] = value.as_m128d();
    }
    assert_eq!(reference.reg.to_bytes(), compact.reg.to_bytes());
    let mut params = Params::new();
    params.hash_length(HASH_SIZE);
    params.hash(&reference.reg.to_bytes())
}

#[cfg(feature = "differential-audit")]
fn run_differential(reference: &mut Vm, compact: &mut Vm, seed: &[m128i; 4], program_index: usize) {
    let bytes = gen_program_aes_4rx4(seed, 136);
    let reference_program = ReferenceProgram::from_bytes(bytes.clone());
    let compact_program = CompactProgram::from_bytes(&bytes);
    reference.init_vm(&reference_program);
    init_vm(compact, &compact_program.entropy);

    let mut reference_sp0 = reference.mem_reg.mx as u32;
    let mut reference_sp1 = reference.mem_reg.ma as u32;
    let mut compact_sp0 = compact.mem_reg.mx as u32;
    let mut compact_sp1 = compact.mem_reg.ma as u32;

    for iteration in 0..PROGRAM_ITERATIONS {
        prepare_iteration(reference, &mut reference_sp0, &mut reference_sp1);
        prepare_iteration(compact, &mut compact_sp0, &mut compact_sp1);
        assert_vm_state(reference, compact, program_index, iteration, -1, None);

        reference.pc = 0;
        compact.pc = 0;
        while reference.pc < PROGRAM_SIZE {
            assert_eq!(
                reference.pc, compact.pc,
                "program {program_index} iteration {iteration}"
            );
            let pc = reference.pc;
            let reference_instr = &reference_program.program[pc as usize];
            let compact_instr = &compact_program.instructions[pc as usize];
            reference_instr.execute(reference);
            (compact_instr.effect)(compact, compact_instr);
            assert_vm_state(
                reference,
                compact,
                program_index,
                iteration,
                pc,
                Some(&reference_instr.op),
            );
            reference.pc += 1;
            compact.pc += 1;
        }

        finish_iteration(reference, reference_sp0 as usize, reference_sp1 as usize);
        finish_iteration(compact, compact_sp0 as usize, compact_sp1 as usize);
        assert_vm_state(
            reference,
            compact,
            program_index,
            iteration,
            PROGRAM_SIZE,
            None,
        );
        reference_sp0 = 0;
        reference_sp1 = 0;
        compact_sp0 = 0;
        compact_sp1 = 0;
    }
}

#[cfg(feature = "differential-audit")]
fn prepare_iteration(vm: &mut Vm, sp_addr_0: &mut u32, sp_addr_1: &mut u32) {
    let sp_mix = vm.reg.r[vm.config.read_reg[0]] ^ vm.reg.r[vm.config.read_reg[1]];
    *sp_addr_0 ^= sp_mix as u32;
    *sp_addr_0 = (*sp_addr_0 & SCRATCHPAD_L3_MASK_U32) >> 3;
    *sp_addr_1 ^= (sp_mix >> 32) as u32;
    *sp_addr_1 = (*sp_addr_1 & SCRATCHPAD_L3_MASK_U32) >> 3;

    let addr0 = *sp_addr_0 as usize;
    let addr1 = *sp_addr_1 as usize;
    for i in 0..MAX_REG {
        vm.reg.r[i] ^= vm.scratchpad[addr0 + i];
    }
    for i in 0..MAX_FLOAT_REG {
        vm.reg.f[i] = m128i::from_u64(0, vm.scratchpad[addr1 + i]).lower_to_m128d();
    }
    for i in 0..MAX_FLOAT_REG {
        let value = m128i::from_u64(0, vm.scratchpad[addr1 + i + MAX_FLOAT_REG]).lower_to_m128d();
        vm.reg.e[i] = mask_register_exponent_mantissa(vm, value);
    }
}

#[cfg(feature = "differential-audit")]
fn finish_iteration(vm: &mut Vm, addr0: usize, addr1: usize) {
    vm.mem_reg.mx ^= (vm.reg.r[vm.config.read_reg[2]] ^ vm.reg.r[vm.config.read_reg[3]]) as usize;
    vm.mem_reg.mx &= CACHE_LINE_ALIGN_MASK as usize;
    vm.mem
        .dataset_read(vm.dataset_offset + vm.mem_reg.ma as u64, &mut vm.reg.r);
    std::mem::swap(&mut vm.mem_reg.mx, &mut vm.mem_reg.ma);

    for i in 0..MAX_REG {
        vm.scratchpad[addr1 + i] = vm.reg.r[i];
    }
    for i in 0..MAX_FLOAT_REG {
        vm.reg.f[i] = vm.reg.f[i] ^ vm.reg.e[i];
    }
    for i in 0..MAX_FLOAT_REG {
        let (high, low) = vm.reg.f[i].as_u64();
        let ix = addr0 + 2 * i;
        vm.scratchpad[ix] = low;
        vm.scratchpad[ix + 1] = high;
    }
}

#[cfg(feature = "differential-audit")]
fn assert_vm_state(
    reference: &Vm,
    compact: &Vm,
    program: usize,
    iteration: usize,
    pc: i32,
    operation: Option<&ReferenceOpcode>,
) {
    assert_eq!(
        reference.reg.to_bytes(),
        compact.reg.to_bytes(),
        "register divergence: program {program} iteration {iteration} pc {pc} op {operation:?} reference_mode={} optimized_mode={}",
        reference.get_rounding_mode(),
        compact.get_rounding_mode(),
    );
    assert_eq!(
        reference.pc, compact.pc,
        "pc divergence: program {program} iteration {iteration} pc {pc} op {operation:?}"
    );
    assert_eq!(
        reference.mem_reg.mx, compact.mem_reg.mx,
        "mx divergence: program {program} iteration {iteration} pc {pc} op {operation:?}"
    );
    assert_eq!(
        reference.mem_reg.ma, compact.mem_reg.ma,
        "ma divergence: program {program} iteration {iteration} pc {pc} op {operation:?}"
    );
    assert_eq!(
        reference.config.read_reg, compact.config.read_reg,
        "read-register divergence: program {program} iteration {iteration} pc {pc} op {operation:?}"
    );
    assert_eq!(
        reference.config.e_mask, compact.config.e_mask,
        "exponent-mask divergence: program {program} iteration {iteration} pc {pc} op {operation:?}"
    );
    assert_eq!(
        reference.dataset_offset, compact.dataset_offset,
        "dataset-offset divergence: program {program} iteration {iteration} pc {pc} op {operation:?}"
    );
    assert_eq!(
        reference.get_rounding_mode(),
        compact.get_rounding_mode(),
        "rounding divergence: program {program} iteration {iteration} pc {pc} op {operation:?}"
    );
}

fn run(vm: &mut Vm, seed: &[m128i; 4]) {
    let bytes = gen_program_aes_4rx4(seed, 136);
    let program = CompactProgram::from_bytes(&bytes);
    init_vm(vm, &program.entropy);
    debug_assert_eq!(vm.scratchpad.len(), SCRATCHPAD_WORDS);

    let mut mx = vm.mem_reg.mx;
    let mut ma = vm.mem_reg.ma;
    let dataset_offset = vm.dataset_offset;
    let read_reg = [
        register_byte_offset(vm.config.read_reg[0] as u8, size_of::<u64>() as u8),
        register_byte_offset(vm.config.read_reg[1] as u8, size_of::<u64>() as u8),
        register_byte_offset(vm.config.read_reg[2] as u8, size_of::<u64>() as u8),
        register_byte_offset(vm.config.read_reg[3] as u8, size_of::<u64>() as u8),
    ];
    let mut sp_addr_0 = mx as u32;
    let mut sp_addr_1 = ma as u32;

    for _ in 0..PROGRAM_ITERATIONS {
        let sp_mix = r(vm, read_reg[0]) ^ r(vm, read_reg[1]);

        sp_addr_0 ^= sp_mix as u32;
        sp_addr_0 = (sp_addr_0 & SCRATCHPAD_L3_MASK_U32) >> 3;
        sp_addr_1 ^= (sp_mix >> 32) as u32;
        sp_addr_1 = (sp_addr_1 & SCRATCHPAD_L3_MASK_U32) >> 3;

        let addr0 = sp_addr_0 as usize;
        let addr1 = sp_addr_1 as usize;
        for i in 0..MAX_REG {
            vm.reg.r[i] ^= scratch(vm, addr0 + i);
        }
        for i in 0..MAX_FLOAT_REG {
            vm.reg.f[i] = m128i::from_u64(0, scratch(vm, addr1 + i)).lower_to_m128d();
        }
        for i in 0..MAX_FLOAT_REG {
            let value = m128i::from_u64(0, scratch(vm, addr1 + i + MAX_FLOAT_REG)).lower_to_m128d();
            vm.reg.e[i] = mask_register_exponent_mantissa(vm, value);
        }

        vm.pc = 0;
        while vm.pc < PROGRAM_SIZE {
            // Branch targets are decode-time instruction indices in -1..255;
            // the loop increment makes the next fetched index 0..255.
            let instr = unsafe { program.instructions.get_unchecked(vm.pc as usize) };
            (instr.effect)(vm, instr);
            vm.pc += 1;
        }

        mx ^= (r(vm, read_reg[2]) ^ r(vm, read_reg[3])) as usize;
        mx &= CACHE_LINE_ALIGN_MASK as usize;
        vm.mem
            .dataset_read(dataset_offset + ma as u64, &mut vm.reg.r);
        std::mem::swap(&mut mx, &mut ma);

        for i in 0..MAX_REG {
            set_scratch(vm, addr1 + i, vm.reg.r[i]);
        }
        for i in 0..MAX_FLOAT_REG {
            vm.reg.f[i] = vm.reg.f[i] ^ vm.reg.e[i];
        }
        for i in 0..MAX_FLOAT_REG {
            let (high, low) = vm.reg.f[i].as_u64();
            let ix = addr0 + 2 * i;
            set_scratch(vm, ix, low);
            set_scratch(vm, ix + 1, high);
        }

        sp_addr_0 = 0;
        sp_addr_1 = 0;
    }

    vm.mem_reg.mx = mx;
    vm.mem_reg.ma = ma;
}

fn init_vm(vm: &mut Vm, entropy: &[u64; 16]) {
    vm.reg.a[0] = m128d::from_u64(
        small_positive_float_bit(entropy[1]),
        small_positive_float_bit(entropy[0]),
    );
    vm.reg.a[1] = m128d::from_u64(
        small_positive_float_bit(entropy[3]),
        small_positive_float_bit(entropy[2]),
    );
    vm.reg.a[2] = m128d::from_u64(
        small_positive_float_bit(entropy[5]),
        small_positive_float_bit(entropy[4]),
    );
    vm.reg.a[3] = m128d::from_u64(
        small_positive_float_bit(entropy[7]),
        small_positive_float_bit(entropy[6]),
    );

    vm.mem_reg.ma = ((entropy[8] & CACHE_LINE_ALIGN_MASK) as u32) as usize;
    vm.mem_reg.mx = (entropy[10] as u32) as usize;

    let mut address_reg = entropy[12] as usize;
    vm.config.read_reg[0] = address_reg & 1;
    address_reg >>= 1;
    vm.config.read_reg[1] = 2 + (address_reg & 1);
    address_reg >>= 1;
    vm.config.read_reg[2] = 4 + (address_reg & 1);
    address_reg >>= 1;
    vm.config.read_reg[3] = 6 + (address_reg & 1);

    vm.dataset_offset = (entropy[13] % (DATASET_EXTRA_ITEMS as u64 + 1)) * CACHE_LINE_SIZE;
    vm.config.e_mask[0] = float_mask(entropy[14]);
    vm.config.e_mask[1] = float_mask(entropy[15]);
    vm.reg.r = [0; MAX_REG];
}

#[allow(overflowing_literals)]
fn decode_instruction(raw: i64, index: i32, usage: &mut [i32; MAX_REG]) -> CompactInstr {
    let op = raw & 0xff;
    let dst = ((raw >> 8) & 0xff) as usize;
    let src = ((raw >> 16) & 0xff) as usize;
    let modifier = ((raw >> 24) & 0xff) as u8;
    let imm32 = (raw >> 32) as i32;
    let dst_r = (dst % MAX_REG) as u8;
    let src_r = (src % MAX_REG) as u8;

    if op < 0x10 {
        usage[dst_r as usize] = index;
        let imm = if dst_r == 5 {
            u64_from_i32_imm(imm32)
        } else {
            0
        };
        return CompactInstr::new(exec_iadd_rs, dst_r, src_r, imm, (modifier >> 2) & 3);
    }
    if op < 0x17 {
        usage[dst_r as usize] = index;
        return decode_memory(exec_iadd_m, dst_r, src_r, imm32, modifier, dst_r == src_r);
    }
    if op < 0x27 {
        usage[dst_r as usize] = index;
        return decode_reg_or_imm(exec_isub_r, exec_isub_imm, dst_r, src_r, imm32);
    }
    if op < 0x2e {
        usage[dst_r as usize] = index;
        return decode_memory(exec_isub_m, dst_r, src_r, imm32, modifier, dst_r == src_r);
    }
    if op < 0x3e {
        usage[dst_r as usize] = index;
        return decode_reg_or_imm(exec_imul_r, exec_imul_imm, dst_r, src_r, imm32);
    }
    if op < 0x42 {
        usage[dst_r as usize] = index;
        return decode_memory(exec_imul_m, dst_r, src_r, imm32, modifier, dst_r == src_r);
    }
    if op < 0x46 {
        usage[dst_r as usize] = index;
        return CompactInstr::new(exec_imulh_r, dst_r, src_r, 0, 0);
    }
    if op < 0x47 {
        usage[dst_r as usize] = index;
        return decode_memory(exec_imulh_m, dst_r, src_r, imm32, modifier, dst_r == src_r);
    }
    if op < 0x4b {
        usage[dst_r as usize] = index;
        return CompactInstr::new(exec_ismulh_r, dst_r, src_r, 0, 0);
    }
    if op < 0x4c {
        usage[dst_r as usize] = index;
        return decode_memory(exec_ismulh_m, dst_r, src_r, imm32, modifier, dst_r == src_r);
    }
    if op < 0x54 {
        let divisor = imm32 as u32 as u64;
        let reciprocal = if is_zero_or_power_of_two(divisor) {
            0
        } else {
            usage[dst_r as usize] = index;
            randomx_reciprocal(divisor)
        };
        return CompactInstr::new(exec_imul_rcp, dst_r, NO_REG, reciprocal, 0);
    }
    if op < 0x56 {
        usage[dst_r as usize] = index;
        return CompactInstr::new(exec_ineg_r, dst_r, NO_REG, 0, 0);
    }
    if op < 0x65 {
        usage[dst_r as usize] = index;
        return decode_reg_or_imm(exec_ixor_r, exec_ixor_imm, dst_r, src_r, imm32);
    }
    if op < 0x6a {
        usage[dst_r as usize] = index;
        return decode_memory(exec_ixor_m, dst_r, src_r, imm32, modifier, dst_r == src_r);
    }
    if op < 0x72 {
        usage[dst_r as usize] = index;
        return decode_reg_or_imm(exec_iror_r, exec_iror_imm, dst_r, src_r, imm32 & 63);
    }
    if op < 0x74 {
        usage[dst_r as usize] = index;
        return decode_reg_or_imm(exec_irol_r, exec_irol_imm, dst_r, src_r, imm32 & 63);
    }
    if op < 0x78 {
        if src_r == dst_r {
            return CompactInstr::new(exec_nop, NO_REG, NO_REG, 0, 0);
        }
        usage[dst_r as usize] = index;
        usage[src_r as usize] = index;
        return CompactInstr::new(exec_iswap_r, dst_r, src_r, 0, 0);
    }
    if op < 0x7c {
        let float_index = dst_r % MAX_FLOAT_REG as u8;
        let effect = if dst_r >= MAX_FLOAT_REG as u8 {
            exec_fswap_e
        } else {
            exec_fswap_f
        };
        return CompactInstr::new_float(effect, float_index, NO_REG, 0, 0);
    }
    if op < 0x8c {
        return CompactInstr::new_float(
            exec_fadd_r,
            (dst % MAX_FLOAT_REG) as u8,
            (src % MAX_FLOAT_REG) as u8,
            0,
            0,
        );
    }
    if op < 0x91 {
        return decode_float_memory(
            exec_fadd_m,
            (dst % MAX_FLOAT_REG) as u8,
            src_r,
            imm32,
            modifier,
            false,
        );
    }
    if op < 0xa1 {
        return CompactInstr::new_float(
            exec_fsub_r,
            (dst % MAX_FLOAT_REG) as u8,
            (src % MAX_FLOAT_REG) as u8,
            0,
            0,
        );
    }
    if op < 0xa6 {
        return decode_float_memory(
            exec_fsub_m,
            (dst % MAX_FLOAT_REG) as u8,
            src_r,
            imm32,
            modifier,
            false,
        );
    }
    if op < 0xac {
        return CompactInstr::new_float(exec_fscal_r, (dst % MAX_FLOAT_REG) as u8, NO_REG, 0, 0);
    }
    if op < 0xcc {
        return CompactInstr::new_float(
            exec_fmul_r,
            (dst % MAX_FLOAT_REG) as u8,
            (src % MAX_FLOAT_REG) as u8,
            0,
            0,
        );
    }
    if op < 0xd0 {
        return decode_float_memory(
            exec_fdiv_m,
            (dst % MAX_FLOAT_REG) as u8,
            src_r,
            imm32,
            modifier,
            false,
        );
    }
    if op < 0xd6 {
        return CompactInstr::new_float(exec_fsqrt_r, (dst % MAX_FLOAT_REG) as u8, NO_REG, 0, 0);
    }
    if op < 0xef {
        let condition_shift = (modifier >> 4) + CONDITION_OFFSET;
        let mut increment = u64_from_i32_imm(imm32) | (1u64 << condition_shift);
        increment &= !(1u64 << (condition_shift - 1));
        let mut instr = CompactInstr::new(exec_cbranch, dst_r, NO_REG, increment, condition_shift);
        instr.target = usage[dst_r as usize];
        for used in usage.iter_mut() {
            *used = index;
        }
        return instr;
    }
    if op < 0xf0 {
        return CompactInstr::new(exec_cfround, NO_REG, src_r, (imm32 & 63) as u64, 0);
    }
    if op < 0x100 {
        let condition = modifier >> 4;
        let mode = if condition >= STORE_L3_CONDITION {
            MEM_L3
        } else if modifier & 3 == 0 {
            MEM_L2
        } else {
            MEM_L1
        };
        return CompactInstr::new_memory(exec_istore, dst_r, src_r, u64_from_i32_imm(imm32), mode);
    }
    CompactInstr::new(exec_nop, NO_REG, NO_REG, 0, 0)
}

#[inline]
fn decode_reg_or_imm(
    register_effect: Effect,
    immediate_effect: Effect,
    dst: u8,
    src: u8,
    imm32: i32,
) -> CompactInstr {
    if dst == src {
        CompactInstr::new(immediate_effect, dst, NO_REG, u64_from_i32_imm(imm32), 0)
    } else {
        CompactInstr::new(register_effect, dst, src, 0, 0)
    }
}

#[inline]
fn decode_memory(
    effect: Effect,
    dst: u8,
    address_reg: u8,
    imm32: i32,
    modifier: u8,
    same_register: bool,
) -> CompactInstr {
    if same_register {
        CompactInstr::new_memory(
            effect,
            dst,
            NO_REG,
            (imm32 as u32 as u64) & SCRATCHPAD_L3_MASK,
            MEM_L3,
        )
    } else {
        let mode = if modifier & 3 == 0 { MEM_L2 } else { MEM_L1 };
        CompactInstr::new_memory(effect, dst, address_reg, u64_from_i32_imm(imm32), mode)
    }
}

#[inline]
fn decode_float_memory(
    effect: Effect,
    dst: u8,
    address_reg: u8,
    imm32: i32,
    modifier: u8,
    same_register: bool,
) -> CompactInstr {
    let mut instr = decode_memory(effect, dst, address_reg, imm32, modifier, same_register);
    instr.dst = register_byte_offset(dst, size_of::<m128d>() as u8);
    instr
}

#[inline(always)]
fn r(vm: &Vm, byte_offset: u8) -> u64 {
    debug_assert_eq!(byte_offset as usize % size_of::<u64>(), 0);
    debug_assert!((byte_offset as usize) < MAX_REG * size_of::<u64>());
    unsafe {
        *vm.reg
            .r
            .as_ptr()
            .cast::<u8>()
            .add(byte_offset as usize)
            .cast::<u64>()
    }
}

#[inline(always)]
fn set_r(vm: &mut Vm, byte_offset: u8, value: u64) {
    debug_assert_eq!(byte_offset as usize % size_of::<u64>(), 0);
    debug_assert!((byte_offset as usize) < MAX_REG * size_of::<u64>());
    unsafe {
        *vm.reg
            .r
            .as_mut_ptr()
            .cast::<u8>()
            .add(byte_offset as usize)
            .cast::<u64>() = value
    }
}

#[inline(always)]
fn f(vm: &Vm, byte_offset: u8) -> m128d {
    debug_assert_eq!(byte_offset as usize % size_of::<m128d>(), 0);
    debug_assert!((byte_offset as usize) < MAX_FLOAT_REG * size_of::<m128d>());
    unsafe {
        *vm.reg
            .f
            .as_ptr()
            .cast::<u8>()
            .add(byte_offset as usize)
            .cast::<m128d>()
    }
}

#[inline(always)]
fn set_f(vm: &mut Vm, byte_offset: u8, value: m128d) {
    debug_assert_eq!(byte_offset as usize % size_of::<m128d>(), 0);
    debug_assert!((byte_offset as usize) < MAX_FLOAT_REG * size_of::<m128d>());
    unsafe {
        *vm.reg
            .f
            .as_mut_ptr()
            .cast::<u8>()
            .add(byte_offset as usize)
            .cast::<m128d>() = value
    }
}

#[inline(always)]
fn e(vm: &Vm, byte_offset: u8) -> m128d {
    debug_assert_eq!(byte_offset as usize % size_of::<m128d>(), 0);
    debug_assert!((byte_offset as usize) < MAX_FLOAT_REG * size_of::<m128d>());
    unsafe {
        *vm.reg
            .e
            .as_ptr()
            .cast::<u8>()
            .add(byte_offset as usize)
            .cast::<m128d>()
    }
}

#[inline(always)]
fn set_e(vm: &mut Vm, byte_offset: u8, value: m128d) {
    debug_assert_eq!(byte_offset as usize % size_of::<m128d>(), 0);
    debug_assert!((byte_offset as usize) < MAX_FLOAT_REG * size_of::<m128d>());
    unsafe {
        *vm.reg
            .e
            .as_mut_ptr()
            .cast::<u8>()
            .add(byte_offset as usize)
            .cast::<m128d>() = value
    }
}

#[inline(always)]
fn a(vm: &Vm, byte_offset: u8) -> m128d {
    debug_assert_eq!(byte_offset as usize % size_of::<m128d>(), 0);
    debug_assert!((byte_offset as usize) < MAX_FLOAT_REG * size_of::<m128d>());
    unsafe {
        *vm.reg
            .a
            .as_ptr()
            .cast::<u8>()
            .add(byte_offset as usize)
            .cast::<m128d>()
    }
}

#[inline(always)]
fn lanes(value: m128d) -> [u64; 2] {
    let (high, low) = value.as_u64();
    [low, high]
}

#[inline(always)]
fn from_lanes(value: [u64; 2]) -> m128d {
    m128d::from_u64(value[1], value[0])
}

#[inline(always)]
fn rounding_mode(vm: &Vm) -> RoundingMode {
    let mode = vm.get_rounding_mode();
    debug_assert!(mode < 4);
    // SAFETY: `Vm` stores the mode in a private field. Its constructor and
    // reset path write zero, and the only setter rejects values above three.
    unsafe { RoundingMode::from_valid_fprc(mode) }
}

#[inline(always)]
fn memory_mask(mode: u8) -> u32 {
    match mode {
        MEM_L1 => SCRATCHPAD_L1_MASK as u32,
        MEM_L2 => SCRATCHPAD_L2_MASK as u32,
        MEM_L3 => SCRATCHPAD_L3_MASK as u32,
        _ => unreachable!(),
    }
}

#[inline(always)]
fn scratchpad_src_offset(vm: &Vm, instr: &CompactInstr) -> usize {
    debug_assert_ne!(instr.memory_mask, 0);
    let address = if instr.src == NO_REG {
        instr.imm
    } else {
        r(vm, instr.src).wrapping_add(instr.imm)
    };
    (address & instr.memory_mask as u64) as usize
}

#[inline(always)]
fn scratchpad_dst_offset(vm: &Vm, instr: &CompactInstr) -> usize {
    debug_assert_ne!(instr.memory_mask, 0);
    (r(vm, instr.dst).wrapping_add(instr.imm) & instr.memory_mask as u64) as usize
}

#[inline(always)]
fn scratch(vm: &Vm, index: usize) -> u64 {
    debug_assert!(index < vm.scratchpad.len());
    unsafe { *vm.scratchpad.get_unchecked(index) }
}

#[inline(always)]
fn set_scratch(vm: &mut Vm, index: usize, value: u64) {
    debug_assert!(index < vm.scratchpad.len());
    unsafe { *vm.scratchpad.get_unchecked_mut(index) = value }
}

#[inline(always)]
fn scratch_at_offset(vm: &Vm, byte_offset: usize) -> u64 {
    debug_assert_eq!(byte_offset & (size_of::<u64>() - 1), 0);
    debug_assert!(byte_offset + size_of::<u64>() <= vm.scratchpad.len() * size_of::<u64>());
    // SAFETY: decoded memory masks clear the low three bits and limit the
    // offset to at most `SCRATCHPAD_L3_MASK`. `calculate_hash_impl` validates the
    // exact allocation length before execution.
    unsafe {
        *vm.scratchpad
            .as_ptr()
            .cast::<u8>()
            .add(byte_offset)
            .cast::<u64>()
    }
}

#[inline(always)]
fn set_scratch_at_offset(vm: &mut Vm, byte_offset: usize, value: u64) {
    debug_assert_eq!(byte_offset & (size_of::<u64>() - 1), 0);
    debug_assert!(byte_offset + size_of::<u64>() <= vm.scratchpad.len() * size_of::<u64>());
    // SAFETY: same mask, alignment, and allocation invariant as
    // `scratch_at_offset`; `&mut Vm` provides exclusive access.
    unsafe {
        *vm.scratchpad
            .as_mut_ptr()
            .cast::<u8>()
            .add(byte_offset)
            .cast::<u64>() = value
    }
}

fn exec_nop(_: &mut Vm, _: &CompactInstr) {}

fn exec_iadd_rs(vm: &mut Vm, instr: &CompactInstr) {
    let addend = (r(vm, instr.src) << instr.mode).wrapping_add(instr.imm);
    set_r(vm, instr.dst, r(vm, instr.dst).wrapping_add(addend));
}

fn exec_iadd_m(vm: &mut Vm, instr: &CompactInstr) {
    let value = scratch_at_offset(vm, scratchpad_src_offset(vm, instr));
    set_r(vm, instr.dst, r(vm, instr.dst).wrapping_add(value));
}

fn exec_isub_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(
        vm,
        instr.dst,
        r(vm, instr.dst).wrapping_sub(r(vm, instr.src)),
    );
}

fn exec_isub_imm(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, r(vm, instr.dst).wrapping_sub(instr.imm));
}

fn exec_isub_m(vm: &mut Vm, instr: &CompactInstr) {
    let value = scratch_at_offset(vm, scratchpad_src_offset(vm, instr));
    set_r(vm, instr.dst, r(vm, instr.dst).wrapping_sub(value));
}

fn exec_imul_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(
        vm,
        instr.dst,
        r(vm, instr.dst).wrapping_mul(r(vm, instr.src)),
    );
}

fn exec_imul_imm(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, r(vm, instr.dst).wrapping_mul(instr.imm));
}

fn exec_imul_m(vm: &mut Vm, instr: &CompactInstr) {
    let value = scratch_at_offset(vm, scratchpad_src_offset(vm, instr));
    set_r(vm, instr.dst, r(vm, instr.dst).wrapping_mul(value));
}

fn exec_imulh_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, mulh(r(vm, instr.src), r(vm, instr.dst)));
}

fn exec_imulh_m(vm: &mut Vm, instr: &CompactInstr) {
    let source = scratch_at_offset(vm, scratchpad_src_offset(vm, instr));
    set_r(vm, instr.dst, mulh(source, r(vm, instr.dst)));
}

fn exec_ismulh_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, smulh(r(vm, instr.src), r(vm, instr.dst)));
}

fn exec_ismulh_m(vm: &mut Vm, instr: &CompactInstr) {
    let source = scratch_at_offset(vm, scratchpad_src_offset(vm, instr));
    set_r(vm, instr.dst, smulh(source, r(vm, instr.dst)));
}

fn exec_imul_rcp(vm: &mut Vm, instr: &CompactInstr) {
    if instr.imm != 0 {
        set_r(vm, instr.dst, r(vm, instr.dst).wrapping_mul(instr.imm));
    }
}

fn exec_ineg_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, (!r(vm, instr.dst)).wrapping_add(1));
}

fn exec_ixor_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, r(vm, instr.dst) ^ r(vm, instr.src));
}

fn exec_ixor_imm(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, r(vm, instr.dst) ^ instr.imm);
}

fn exec_ixor_m(vm: &mut Vm, instr: &CompactInstr) {
    let source = scratch_at_offset(vm, scratchpad_src_offset(vm, instr));
    set_r(vm, instr.dst, r(vm, instr.dst) ^ source);
}

fn exec_iror_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(
        vm,
        instr.dst,
        r(vm, instr.dst).rotate_right((r(vm, instr.src) & 0xff_ffff) as u32),
    );
}

fn exec_iror_imm(vm: &mut Vm, instr: &CompactInstr) {
    set_r(
        vm,
        instr.dst,
        r(vm, instr.dst).rotate_right(instr.imm as u32),
    );
}

fn exec_irol_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(
        vm,
        instr.dst,
        r(vm, instr.dst).rotate_left((r(vm, instr.src) & 0xff_ffff) as u32),
    );
}

fn exec_irol_imm(vm: &mut Vm, instr: &CompactInstr) {
    set_r(
        vm,
        instr.dst,
        r(vm, instr.dst).rotate_left(instr.imm as u32),
    );
}

fn exec_iswap_r(vm: &mut Vm, instr: &CompactInstr) {
    let source = r(vm, instr.src);
    let destination = r(vm, instr.dst);
    set_r(vm, instr.dst, source);
    set_r(vm, instr.src, destination);
}

fn exec_fswap_f(vm: &mut Vm, instr: &CompactInstr) {
    let value = f(vm, instr.dst);
    set_f(vm, instr.dst, value.shuffle_1(&value));
}

fn exec_fswap_e(vm: &mut Vm, instr: &CompactInstr) {
    let value = e(vm, instr.dst);
    set_e(vm, instr.dst, value.shuffle_1(&value));
}

fn exec_fadd_r(vm: &mut Vm, instr: &CompactInstr) {
    let destination = f(vm, instr.dst);
    let source = a(vm, instr.src);
    let mode = rounding_mode(vm);
    let result = if mode == RoundingMode::Nearest {
        destination + source
    } else {
        from_lanes(add2(lanes(destination), lanes(source), mode))
    };
    set_f(vm, instr.dst, result);
}

fn exec_fadd_m(vm: &mut Vm, instr: &CompactInstr) {
    let source = m128i::from_u64(0, scratch_at_offset(vm, scratchpad_src_offset(vm, instr)))
        .lower_to_m128d();
    let destination = f(vm, instr.dst);
    let mode = rounding_mode(vm);
    let result = if mode == RoundingMode::Nearest {
        destination + source
    } else {
        from_lanes(add2(lanes(destination), lanes(source), mode))
    };
    set_f(vm, instr.dst, result);
}

fn exec_fsub_r(vm: &mut Vm, instr: &CompactInstr) {
    let destination = f(vm, instr.dst);
    let source = a(vm, instr.src);
    let mode = rounding_mode(vm);
    let result = if mode == RoundingMode::Nearest {
        destination - source
    } else {
        from_lanes(sub2(lanes(destination), lanes(source), mode))
    };
    set_f(vm, instr.dst, result);
}

fn exec_fsub_m(vm: &mut Vm, instr: &CompactInstr) {
    let source = m128i::from_u64(0, scratch_at_offset(vm, scratchpad_src_offset(vm, instr)))
        .lower_to_m128d();
    let destination = f(vm, instr.dst);
    let mode = rounding_mode(vm);
    let result = if mode == RoundingMode::Nearest {
        destination - source
    } else {
        from_lanes(sub2(lanes(destination), lanes(source), mode))
    };
    set_f(vm, instr.dst, result);
}

fn exec_fscal_r(vm: &mut Vm, instr: &CompactInstr) {
    let mask = m128d::from_u64(0x80f0000000000000, 0x80f0000000000000);
    set_f(vm, instr.dst, f(vm, instr.dst) ^ mask);
}

fn exec_fmul_r(vm: &mut Vm, instr: &CompactInstr) {
    let destination = e(vm, instr.dst);
    let source = a(vm, instr.src);
    let mode = rounding_mode(vm);
    let result = if mode == RoundingMode::Nearest {
        destination * source
    } else {
        from_lanes(mul2(lanes(destination), lanes(source), mode))
    };
    set_e(vm, instr.dst, result);
}

fn exec_fdiv_m(vm: &mut Vm, instr: &CompactInstr) {
    let source = m128i::from_u64(0, scratch_at_offset(vm, scratchpad_src_offset(vm, instr)))
        .lower_to_m128d();
    let source = mask_register_exponent_mantissa(vm, source);
    let destination = e(vm, instr.dst);
    let mode = rounding_mode(vm);
    let result = if mode == RoundingMode::Nearest {
        destination / source
    } else {
        from_lanes(div2(lanes(destination), lanes(source), mode))
    };
    set_e(vm, instr.dst, result);
}

fn exec_fsqrt_r(vm: &mut Vm, instr: &CompactInstr) {
    let destination = e(vm, instr.dst);
    let mode = rounding_mode(vm);
    let result = if mode == RoundingMode::Nearest {
        destination.sqrt()
    } else {
        from_lanes(sqrt2(lanes(destination), mode))
    };
    set_e(vm, instr.dst, result);
}

fn exec_cbranch(vm: &mut Vm, instr: &CompactInstr) {
    let destination = r(vm, instr.dst).wrapping_add(instr.imm);
    set_r(vm, instr.dst, destination);
    if destination & (CONDITION_MASK << instr.mode) == 0 {
        vm.pc = instr.target;
    }
}

fn exec_cfround(vm: &mut Vm, instr: &CompactInstr) {
    let mode = (r(vm, instr.src).rotate_right(instr.imm as u32) & 3) as u32;
    vm.set_rounding_mode(mode);
}

fn exec_istore(vm: &mut Vm, instr: &CompactInstr) {
    let byte_offset = scratchpad_dst_offset(vm, instr);
    set_scratch_at_offset(vm, byte_offset, r(vm, instr.src));
}

#[inline(always)]
fn mask_register_exponent_mantissa(vm: &Vm, value: m128d) -> m128d {
    let mantissa_mask = m128d::from_u64(DYNAMIC_MANTISSA_MASK, DYNAMIC_MANTISSA_MASK);
    let exponent_mask = m128d::from_u64(vm.config.e_mask[1], vm.config.e_mask[0]);
    (value & mantissa_mask) | exponent_mask
}

#[inline]
fn hash_to_m128i_array(hash: &Hash) -> [m128i; 4] {
    let bytes = hash.as_bytes();
    [
        m128i::from_u8(&bytes[0..16]),
        m128i::from_u8(&bytes[16..32]),
        m128i::from_u8(&bytes[32..48]),
        m128i::from_u8(&bytes[48..64]),
    ]
}

#[inline]
fn is_zero_or_power_of_two(value: u64) -> bool {
    value & value.wrapping_sub(1) == 0
}

fn small_positive_float_bit(entropy: u64) -> u64 {
    let mut exponent = entropy >> 59;
    let mantissa = entropy & MANTISSA_MASK;
    exponent += EXPONENT_BIAS;
    exponent &= EXPONENT_MASK;
    (exponent << MANTISSA_SIZE) | mantissa
}

fn float_mask(entropy: u64) -> u64 {
    let mask22bit = (1 << 22) - 1;
    (entropy & mask22bit) | static_exponent(entropy)
}

fn static_exponent(entropy: u64) -> u64 {
    let exponent =
        EXPONENT_BITS | ((entropy >> (64 - STATIC_EXPONENT_BITS)) << DYNAMIC_EXPONENT_BITS);
    exponent << MANTISSA_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use randomx_sp1_core::program::decode_instruction as decode_reference_instruction;

    fn reset_instruction_state(vm: &mut Vm, salt: u64, mode: u32) {
        for (index, register) in vm.reg.r.iter_mut().enumerate() {
            *register = salt.wrapping_add((index as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        }
        vm.reg.f = [
            m128d::from_f64(-17.25, 3.5),
            m128d::from_f64(0.125, -91.75),
            m128d::from_f64(65_537.5, -0.75),
            m128d::from_f64(-1.0 / 3.0, 7.0 / 11.0),
        ];
        vm.reg.e = [
            m128d::from_f64(1.5, 2.25),
            m128d::from_f64(3.75, 4.5),
            m128d::from_f64(5.125, 6.875),
            m128d::from_f64(7.25, 8.625),
        ];
        vm.reg.a = [
            m128d::from_f64(1.0 / 7.0, 1.0 / 13.0),
            m128d::from_f64(11.0 / 17.0, 19.0 / 23.0),
            m128d::from_f64(29.0 / 31.0, 37.0 / 41.0),
            m128d::from_f64(43.0 / 47.0, 53.0 / 59.0),
        ];
        vm.config.e_mask = [0x3ff0_0000_0012_3456, 0x4000_0000_0065_4321];
        vm.pc = 0;
        vm.set_rounding_mode(mode);
    }

    fn assert_complete_vm_state(reference: &Vm, compact: &Vm) {
        assert_eq!(reference.reg.to_bytes(), compact.reg.to_bytes());
        assert_eq!(reference.scratchpad, compact.scratchpad);
        assert_eq!(reference.mem_reg.mx, compact.mem_reg.mx);
        assert_eq!(reference.mem_reg.ma, compact.mem_reg.ma);
        assert_eq!(reference.pc, compact.pc);
        assert_eq!(reference.config.read_reg, compact.config.read_reg);
        assert_eq!(reference.config.e_mask, compact.config.e_mask);
        assert_eq!(reference.dataset_offset, compact.dataset_offset);
        assert_eq!(reference.get_rounding_mode(), compact.get_rounding_mode());
    }

    #[test]
    fn compact_instruction_is_32_bytes() {
        assert_eq!(size_of::<CompactInstr>(), 32);
    }

    #[test]
    fn predecoded_register_offsets_are_aligned_and_bounded() {
        for index in 0..MAX_REG as u8 {
            let offset = register_byte_offset(index, size_of::<u64>() as u8) as usize;
            assert_eq!(offset, index as usize * size_of::<u64>());
            assert_eq!(offset % size_of::<u64>(), 0);
            assert!(offset < MAX_REG * size_of::<u64>());
        }
        for index in 0..MAX_FLOAT_REG as u8 {
            let offset = register_byte_offset(index, size_of::<m128d>() as u8) as usize;
            assert_eq!(offset, index as usize * size_of::<m128d>());
            assert_eq!(offset % size_of::<m128d>(), 0);
            assert!(offset < MAX_FLOAT_REG * size_of::<m128d>());
        }
        assert_eq!(register_byte_offset(NO_REG, size_of::<u64>() as u8), NO_REG);
    }

    #[test]
    #[should_panic]
    fn vm_rejects_invalid_rounding_mode() {
        let memory = Arc::new(VmMemory::no_memory());
        let mut vm = new_vm(memory);
        vm.set_rounding_mode(4);
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed")]
    fn compact_hash_rejects_malformed_scratchpad() {
        let memory = Arc::new(VmMemory::no_memory());
        let mut vm = new_vm(memory);
        vm.scratchpad.pop();
        let _ = calculate_hash_impl(&mut vm, &[]);
    }

    #[test]
    fn every_opcode_byte_matches_reference_decoder_at_boundaries() {
        let memory = Arc::new(VmMemory::no_memory());
        let mut reference = new_vm(Arc::clone(&memory));
        let mut compact = new_vm(memory);
        for (index, (reference_word, compact_word)) in reference
            .scratchpad
            .iter_mut()
            .zip(compact.scratchpad.iter_mut())
            .enumerate()
        {
            let value = (index as u64)
                .wrapping_mul(0xd6e8_feb8_6659_fd93)
                .rotate_left((index & 63) as u32);
            *reference_word = value;
            *compact_word = value;
        }

        let operand_cases = [
            (0u8, 0u8, 0u8, 0i32),
            (255, 254, 255, -1),
            (5, 5, 0xe0, i32::MIN),
            (0x9d, 0x63, 0x5b, 0x1357_9bdf),
        ];
        for opcode in 0..=u8::MAX {
            for (case, &(dst, src, modifier, immediate)) in operand_cases.iter().enumerate() {
                let raw = (opcode as u64)
                    | ((dst as u64) << 8)
                    | ((src as u64) << 16)
                    | ((modifier as u64) << 24)
                    | ((immediate as u32 as u64) << 32);
                let mut reference_usage = [-1, 3, 7, 11, 19, 23, 29, 31];
                let mut compact_usage = reference_usage;
                let reference_instr =
                    decode_reference_instruction(raw as i64, 37, &mut reference_usage);
                let compact_instr = decode_instruction(raw as i64, 37, &mut compact_usage);
                assert_eq!(
                    reference_usage, compact_usage,
                    "opcode {opcode:#04x} case {case}"
                );
                let is_memory = matches!(
                    opcode,
                    0x10..=0x16
                        | 0x27..=0x2d
                        | 0x3e..=0x41
                        | 0x46
                        | 0x4b
                        | 0x65..=0x69
                        | 0x8c..=0x90
                        | 0xa1..=0xa5
                        | 0xcc..=0xcf
                        | 0xf0..=0xff
                );
                if is_memory {
                    assert_eq!(
                        compact_instr.memory_mask,
                        memory_mask(compact_instr.mode),
                        "memory mask mismatch for opcode {opcode:#04x} case {case}"
                    );
                } else {
                    assert_eq!(
                        compact_instr.memory_mask, 0,
                        "unexpected memory mask for opcode {opcode:#04x} case {case}"
                    );
                }

                let salt = raw ^ 0xa076_1d64_78bd_642f;
                let mode = case as u32;
                reset_instruction_state(&mut reference, salt, mode);
                reset_instruction_state(&mut compact, salt, mode);
                reference_instr.execute(&mut reference);
                (compact_instr.effect)(&mut compact, &compact_instr);

                assert_eq!(
                    reference.reg.to_bytes(),
                    compact.reg.to_bytes(),
                    "register divergence for opcode {opcode:#04x} case {case} ({:?})",
                    reference_instr.op
                );
                assert_eq!(
                    reference.pc, compact.pc,
                    "PC divergence for opcode {opcode:#04x} case {case} ({:?})",
                    reference_instr.op
                );
                assert_eq!(
                    reference.get_rounding_mode(),
                    compact.get_rounding_mode(),
                    "rounding divergence for opcode {opcode:#04x} case {case} ({:?})",
                    reference_instr.op
                );
                if opcode >= 0xf0 {
                    assert_eq!(
                        reference.scratchpad, compact.scratchpad,
                        "store divergence for opcode {opcode:#04x} case {case}"
                    );
                }
            }
        }
        compact.reset_rounding_mode();
    }

    #[test]
    fn all_rounding_modes_hash_matches_official_randomx() {
        let seed = [
            0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54, 0x16,
            0xe5, 0xb6, 0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74, 0x6b, 0xb7,
            0x8e, 0xb4, 0xbe, 0x8e,
        ];
        let blob = [
            0x10, 0x10, 0xc5, 0xa2, 0x99, 0xd3, 0x06, 0x5e, 0xd0, 0x66, 0x57, 0x3b, 0x62, 0xcd,
            0xcc, 0x0d, 0x24, 0x3d, 0x8b, 0x71, 0x30, 0xcf, 0x8b, 0xe8, 0x7f, 0xf7, 0x1e, 0xc3,
            0x02, 0xce, 0xdd, 0x31, 0xdb, 0x9f, 0x6f, 0x4f, 0x6e, 0x10, 0xe8, 0x5d, 0x5a, 0x4c,
            0x10, 0x76, 0xf9, 0xef, 0x57, 0xaa, 0xbb, 0x92, 0x00, 0x4f, 0xaf, 0xeb, 0xc6, 0x8b,
            0x9a, 0x54, 0xbc, 0x9d, 0x35, 0x84, 0xec, 0x8f, 0x94, 0x3e, 0x94, 0x9b, 0xc4, 0xc3,
            0x72, 0xa5, 0x01, 0x00, 0x00, 0x00,
        ];
        let expected = [
            0xc1, 0x9a, 0xe2, 0xf2, 0xf5, 0x0a, 0x2e, 0x33, 0xec, 0x73, 0x74, 0x84, 0xe6, 0xc4,
            0x47, 0xd9, 0xb0, 0xff, 0xe4, 0x44, 0x31, 0xa3, 0x32, 0x01, 0x02, 0x6b, 0xa9, 0xec,
            0xa7, 0x0f, 0xda, 0x95,
        ];

        let memory = Arc::new(VmMemory::light(&seed));
        let mut reference = new_vm(Arc::clone(&memory));
        let mut compact = new_vm(memory);
        let reference_hash = reference.calculate_hash(&blob);
        let compact_hash = calculate_hash_impl(&mut compact, &blob);

        assert_eq!(reference_hash.as_bytes(), &expected);
        assert_eq!(compact_hash.as_bytes(), &expected);
        assert_complete_vm_state(&reference, &compact);
    }

    #[test]
    fn original_block_hash_matches_reference_and_official_randomx() {
        let seed = [
            0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54, 0x16,
            0xe5, 0xb6, 0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74, 0x6b, 0xb7,
            0x8e, 0xb4, 0xbe, 0x8e,
        ];
        let blob = [
            0x10, 0x10, 0xc5, 0xa2, 0x99, 0xd3, 0x06, 0x5e, 0xd0, 0x66, 0x57, 0x3b, 0x62, 0xcd,
            0xcc, 0x0d, 0x24, 0x3d, 0x8b, 0x71, 0x30, 0xcf, 0x8b, 0xe8, 0x7f, 0xf7, 0x1e, 0xc3,
            0x02, 0xce, 0xdd, 0x31, 0xdb, 0x9f, 0x6f, 0x4f, 0x6e, 0x10, 0xe8, 0x5d, 0x5a, 0x4c,
            0x10, 0x76, 0xf9, 0xef, 0x57, 0xaa, 0xbb, 0x92, 0x00, 0x4f, 0xaf, 0xeb, 0xc6, 0x8b,
            0x9a, 0x54, 0xbc, 0x9d, 0x35, 0x84, 0xec, 0x8f, 0x94, 0x3e, 0x94, 0x9b, 0xc4, 0xc3,
            0x72, 0xa5, 0xf3, 0xb4, 0xe6, 0x1d,
        ];
        let expected = [
            0x04, 0x3f, 0x95, 0xd6, 0xe6, 0x12, 0xd7, 0xc9, 0x68, 0x79, 0xdd, 0x25, 0xab, 0x78,
            0x45, 0x64, 0x81, 0xcf, 0xbb, 0x63, 0x01, 0x43, 0xa5, 0x20, 0x1c, 0x38, 0x92, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        let memory = Arc::new(VmMemory::light(&seed));
        let mut reference = new_vm(Arc::clone(&memory));
        let mut compact = new_vm(memory);
        let reference_hash = reference.calculate_hash(&blob);
        let compact_hash = calculate_hash_impl(&mut compact, &blob);

        assert_eq!(reference_hash.as_bytes(), &expected);
        assert_eq!(compact_hash.as_bytes(), &expected);
        assert_complete_vm_state(&reference, &compact);
    }

    fn decode_hex(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0);
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let digit = |byte| match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    b'A'..=b'F' => byte - b'A' + 10,
                    _ => panic!("invalid hexadecimal digit"),
                };
                (digit(pair[0]) << 4) | digit(pair[1])
            })
            .collect()
    }

    #[cfg(target_arch = "x86_64")]
    fn host_rounding_mode() -> Option<u32> {
        let mut mxcsr = 0u32;
        unsafe {
            std::arch::asm!(
                "stmxcsr [{address}]",
                address = in(reg) &mut mxcsr as *mut u32,
                options(nostack)
            );
        }
        Some((mxcsr >> 13) & 3)
    }

    #[cfg(target_arch = "aarch64")]
    fn host_rounding_mode() -> Option<u32> {
        let fpcr: u64;
        unsafe {
            std::arch::asm!("mrs {value}, fpcr", value = out(reg) fpcr);
        }
        Some(match (fpcr >> 22) & 3 {
            0 => 0,
            1 => 2,
            2 => 1,
            3 => 3,
            _ => unreachable!(),
        })
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    fn host_rounding_mode() -> Option<u32> {
        None
    }

    fn assert_canonical_hashes(key: &[u8], cases: &[(&[u8], &str)]) {
        let memory = Arc::new(VmMemory::light(key));
        let mut reference = new_vm(Arc::clone(&memory));
        let mut compact = new_vm(memory);

        for &(input, expected) in cases {
            reference.reset_rounding_mode();
            let reference_hash = reference.calculate_hash(input);
            assert_eq!(reference_hash.as_bytes(), decode_hex(expected));
            if let Some(mode) = host_rounding_mode() {
                assert_eq!(
                    mode, 0,
                    "reference hash did not preserve caller rounding mode"
                );
            }

            compact.reset_rounding_mode();
            let compact_hash = calculate_hash_impl(&mut compact, input);
            assert_eq!(compact_hash.as_bytes(), decode_hex(expected));
            if let Some(mode) = host_rounding_mode() {
                assert_eq!(
                    mode, 0,
                    "compact hash did not preserve caller rounding mode"
                );
            }
            assert_complete_vm_state(&reference, &compact);
        }
    }

    #[test]
    fn canonical_v1_interpreter_hash_and_rounding_vectors() {
        let key_000_cases: [(&[u8], &str); 3] = [
            (
                b"This is a test",
                "639183aae1bf4c9a35884cb46b09cad9175f04efd7684e7262a0ac1c2f0b4e3f",
            ),
            (
                b"Lorem ipsum dolor sit amet",
                "300a0adb47603dedb42228ccb2b211104f4da45af709cd7547cd049e9489c969",
            ),
            (
                b"sed do eiusmod tempor incididunt ut labore et dolore magna aliqua",
                "c36d4ed4191e617309867ed66a443be4075014e2b061bcdaf9ce7b721d2b77a8",
            ),
        ];
        assert_canonical_hashes(b"test key 000", &key_000_cases);

        let input_e = decode_hex(
            "0b0b98bea7e805e0010a2126d287a2a0cc833d312cb786385a7c2f9de69d2553\
             7f584a9bc9977b00000000666fd8753bf61a8631f12984e3fd44f4014eca6292\
             76817b56f32e9b68bd82f416",
        );
        let key_001_cases: [(&[u8], &str); 2] = [
            (
                b"sed do eiusmod tempor incididunt ut labore et dolore magna aliqua",
                "e9ff4503201c0c2cca26d285c93ae883f9b1d30c9eb240b820756f2d5a7905fc",
            ),
            (
                &input_e,
                "c56414121acda1713c2f2a819d8ae38aed7c80c35c2a769298d34f03833cd5f1",
            ),
        ];
        assert_canonical_hashes(b"test key 001", &key_001_cases);

        let key_f = [
            0x77, 0x97, 0x37, 0x3e, 0xa4, 0x63, 0x31, 0x94, 0x64, 0x0b, 0xf8, 0xd8, 0xc3, 0xb6,
            0x67, 0x24, 0xd6, 0xaa, 0x7b, 0xd2, 0xdc, 0x20, 0xe0, 0x09, 0xdf, 0x2f, 0x8f, 0x17,
            0x10, 0xab, 0xe8,
        ];
        let input_f = decode_hex(
            "1010e1eaf8cf067b37b5f0ee031ab23ed1755e090a3af4415830145853e2be3e\
             1f6821fed84dae58d00e00da5214d6c1f2d0622e0abd51f9373d04e0b0f8e6d\
             6514d90689721c4aac5a9bb0d",
        );
        assert_canonical_hashes(
            &key_f,
            &[(
                &input_f,
                "78af2a1864c42abce36d2e8983e13df99b2af0ce1362999af09fab004d4435a8",
            )],
        );
    }

    #[test]
    fn stable_hash_api_matches_canonical_vector() {
        assert_eq!(
            hash(b"test key 000", b"This is a test").as_slice(),
            decode_hex("639183aae1bf4c9a35884cb46b09cad9175f04efd7684e7262a0ac1c2f0b4e3f")
        );
    }
}
