//! Claim-preserving compact execution for the RandomX VM.
//!
//! The cache, dataset items, AES program bytes, and all VM iterations are still
//! derived and executed by the guest.  This module only decodes each 8-byte VM
//! instruction into a flat representation before the 2,048-iteration loop.

use std::mem::size_of;
#[cfg(feature = "differential-audit")]
use std::sync::Arc;

use blake2b_simd::{blake2b, Hash, Params};
use randomx_softfp::{add2, div2, mul2, sqrt2, sub2, RoundingMode};
use rustdom_x::common::{mulh, randomx_reciprocal, smulh, u64_from_i32_imm};
use rustdom_x::hash::{gen_program_aes_4rx4, hash_aes_1rx4};
use rustdom_x::m128::{m128d, m128i};
use rustdom_x::memory::CACHE_LINE_SIZE;
#[cfg(feature = "differential-audit")]
use rustdom_x::memory::VmMemory;
#[cfg(feature = "differential-audit")]
use rustdom_x::program::{Opcode as RichOpcode, Program as RichProgram};
use rustdom_x::vm::Vm;

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
/// transformed into a branch increment as appropriate for `effect`.
#[repr(C)]
#[derive(Clone, Copy)]
struct CompactInstr {
    effect: Effect,
    imm: u64,
    target: i32,
    dst: u8,
    src: u8,
    mode: u8,
    _reserved: u8,
}

const _: () = assert!(size_of::<CompactInstr>() == 24);

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
            dst,
            src,
            mode,
            _reserved: 0,
        }
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

/// Executes a complete RandomX hash with the same memory provider and VM state
/// used by `rustdom_x::Vm::calculate_hash`.
pub fn calculate_hash(vm: &mut Vm, input: &[u8]) -> Hash {
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

/// Locates the first state divergence between the rich and compact decoders.
/// This is intentionally excluded from verifier builds.
#[cfg(feature = "differential-audit")]
pub fn differential_audit(input: &[u8]) -> Hash {
    let memory = Arc::new(VmMemory::no_memory());
    let mut rich = rustdom_x::new_vm(Arc::clone(&memory));
    let mut compact = rustdom_x::new_vm(memory);
    let initial_hash = blake2b(input);
    let mut seed = hash_to_m128i_array(&initial_hash);

    let rich_next = rich.init_scratchpad(&seed);
    let compact_next = compact.init_scratchpad(&seed);
    assert_eq!(rich_next, compact_next);
    assert_eq!(rich.scratchpad, compact.scratchpad);
    rich.reset_rounding_mode();
    compact.reset_rounding_mode();

    let mut next_seed = rich_next;
    for program_index in 0..PROGRAM_COUNT {
        run_differential(&mut rich, &mut compact, &next_seed, program_index);
        assert_eq!(rich.reg.to_bytes(), compact.reg.to_bytes());
        assert_eq!(rich.scratchpad, compact.scratchpad);
        if program_index + 1 < PROGRAM_COUNT {
            seed = hash_to_m128i_array(&blake2b(&rich.reg.to_bytes()));
            next_seed = seed;
        }
    }

    let final_hash = hash_aes_1rx4(&rich.scratchpad);
    for index in 0..MAX_FLOAT_REG {
        rich.reg.a[index] = final_hash[index].as_m128d();
        compact.reg.a[index] = final_hash[index].as_m128d();
    }
    assert_eq!(rich.reg.to_bytes(), compact.reg.to_bytes());
    let mut params = Params::new();
    params.hash_length(HASH_SIZE);
    params.hash(&rich.reg.to_bytes())
}

#[cfg(feature = "differential-audit")]
fn run_differential(rich: &mut Vm, compact: &mut Vm, seed: &[m128i; 4], program_index: usize) {
    let bytes = gen_program_aes_4rx4(seed, 136);
    let rich_program = RichProgram::from_bytes(bytes.clone());
    let compact_program = CompactProgram::from_bytes(&bytes);
    rich.init_vm(&rich_program);
    init_vm(compact, &compact_program.entropy);

    let mut rich_sp0 = rich.mem_reg.mx as u32;
    let mut rich_sp1 = rich.mem_reg.ma as u32;
    let mut compact_sp0 = compact.mem_reg.mx as u32;
    let mut compact_sp1 = compact.mem_reg.ma as u32;

    for iteration in 0..PROGRAM_ITERATIONS {
        prepare_iteration(rich, &mut rich_sp0, &mut rich_sp1);
        prepare_iteration(compact, &mut compact_sp0, &mut compact_sp1);
        assert_vm_state(rich, compact, program_index, iteration, -1, None);

        rich.pc = 0;
        compact.pc = 0;
        while rich.pc < PROGRAM_SIZE {
            assert_eq!(rich.pc, compact.pc, "program {program_index} iteration {iteration}");
            let pc = rich.pc;
            let rich_instr = &rich_program.program[pc as usize];
            let compact_instr = &compact_program.instructions[pc as usize];
            rich_instr.execute(rich);
            (compact_instr.effect)(compact, compact_instr);
            assert_vm_state(
                rich,
                compact,
                program_index,
                iteration,
                pc,
                Some(&rich_instr.op),
            );
            rich.pc += 1;
            compact.pc += 1;
        }

        finish_iteration(rich, rich_sp0 as usize, rich_sp1 as usize);
        finish_iteration(compact, compact_sp0 as usize, compact_sp1 as usize);
        assert_vm_state(rich, compact, program_index, iteration, PROGRAM_SIZE, None);
        rich_sp0 = 0;
        rich_sp1 = 0;
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
    vm.mem_reg.mx ^=
        (vm.reg.r[vm.config.read_reg[2]] ^ vm.reg.r[vm.config.read_reg[3]]) as usize;
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
    rich: &Vm,
    compact: &Vm,
    program: usize,
    iteration: usize,
    pc: i32,
    operation: Option<&RichOpcode>,
) {
    assert_eq!(
        rich.reg.to_bytes(),
        compact.reg.to_bytes(),
        "register divergence: program {program} iteration {iteration} pc {pc} op {operation:?} rich_mode={} compact_mode={}",
        rich.get_rounding_mode(),
        compact.get_rounding_mode(),
    );
    assert_eq!(
        rich.pc,
        compact.pc,
        "pc divergence: program {program} iteration {iteration} pc {pc} op {operation:?}"
    );
}

fn run(vm: &mut Vm, seed: &[m128i; 4]) {
    let bytes = gen_program_aes_4rx4(seed, 136);
    let program = CompactProgram::from_bytes(&bytes);
    init_vm(vm, &program.entropy);

    let mut sp_addr_0 = vm.mem_reg.mx as u32;
    let mut sp_addr_1 = vm.mem_reg.ma as u32;

    for _ in 0..PROGRAM_ITERATIONS {
        let sp_mix = r(vm, vm.config.read_reg[0] as u8) ^ r(vm, vm.config.read_reg[1] as u8);

        sp_addr_0 ^= sp_mix as u32;
        sp_addr_0 = (sp_addr_0 & SCRATCHPAD_L3_MASK_U32) >> 3;
        sp_addr_1 ^= (sp_mix >> 32) as u32;
        sp_addr_1 = (sp_addr_1 & SCRATCHPAD_L3_MASK_U32) >> 3;

        let addr0 = sp_addr_0 as usize;
        let addr1 = sp_addr_1 as usize;
        for i in 0..MAX_REG {
            vm.reg.r[i] ^= vm.scratchpad[addr0 + i];
        }
        for i in 0..MAX_FLOAT_REG {
            vm.reg.f[i] = m128i::from_u64(0, vm.scratchpad[addr1 + i]).lower_to_m128d();
        }
        for i in 0..MAX_FLOAT_REG {
            let value =
                m128i::from_u64(0, vm.scratchpad[addr1 + i + MAX_FLOAT_REG]).lower_to_m128d();
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

        vm.mem_reg.mx ^=
            (r(vm, vm.config.read_reg[2] as u8) ^ r(vm, vm.config.read_reg[3] as u8)) as usize;
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

        sp_addr_0 = 0;
        sp_addr_1 = 0;
    }
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
        return CompactInstr::new(effect, float_index, NO_REG, 0, 0);
    }
    if op < 0x8c {
        return CompactInstr::new(
            exec_fadd_r,
            (dst % MAX_FLOAT_REG) as u8,
            (src % MAX_FLOAT_REG) as u8,
            0,
            0,
        );
    }
    if op < 0x91 {
        return decode_memory(
            exec_fadd_m,
            (dst % MAX_FLOAT_REG) as u8,
            src_r,
            imm32,
            modifier,
            false,
        );
    }
    if op < 0xa1 {
        return CompactInstr::new(
            exec_fsub_r,
            (dst % MAX_FLOAT_REG) as u8,
            (src % MAX_FLOAT_REG) as u8,
            0,
            0,
        );
    }
    if op < 0xa6 {
        return decode_memory(
            exec_fsub_m,
            (dst % MAX_FLOAT_REG) as u8,
            src_r,
            imm32,
            modifier,
            false,
        );
    }
    if op < 0xac {
        return CompactInstr::new(exec_fscal_r, (dst % MAX_FLOAT_REG) as u8, NO_REG, 0, 0);
    }
    if op < 0xcc {
        return CompactInstr::new(
            exec_fmul_r,
            (dst % MAX_FLOAT_REG) as u8,
            (src % MAX_FLOAT_REG) as u8,
            0,
            0,
        );
    }
    if op < 0xd0 {
        return decode_memory(
            exec_fdiv_m,
            (dst % MAX_FLOAT_REG) as u8,
            src_r,
            imm32,
            modifier,
            false,
        );
    }
    if op < 0xd6 {
        return CompactInstr::new(exec_fsqrt_r, (dst % MAX_FLOAT_REG) as u8, NO_REG, 0, 0);
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
        return CompactInstr::new(exec_istore, dst_r, src_r, u64_from_i32_imm(imm32), mode);
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
        CompactInstr::new(
            effect,
            dst,
            NO_REG,
            (imm32 as u32 as u64) & SCRATCHPAD_L3_MASK,
            MEM_L3,
        )
    } else {
        let mode = if modifier & 3 == 0 { MEM_L2 } else { MEM_L1 };
        CompactInstr::new(effect, dst, address_reg, u64_from_i32_imm(imm32), mode)
    }
}

#[inline(always)]
fn r(vm: &Vm, index: u8) -> u64 {
    debug_assert!((index as usize) < MAX_REG);
    unsafe { *vm.reg.r.get_unchecked(index as usize) }
}

#[inline(always)]
fn set_r(vm: &mut Vm, index: u8, value: u64) {
    debug_assert!((index as usize) < MAX_REG);
    unsafe { *vm.reg.r.get_unchecked_mut(index as usize) = value }
}

#[inline(always)]
fn f(vm: &Vm, index: u8) -> m128d {
    debug_assert!((index as usize) < MAX_FLOAT_REG);
    unsafe { *vm.reg.f.get_unchecked(index as usize) }
}

#[inline(always)]
fn set_f(vm: &mut Vm, index: u8, value: m128d) {
    debug_assert!((index as usize) < MAX_FLOAT_REG);
    unsafe { *vm.reg.f.get_unchecked_mut(index as usize) = value }
}

#[inline(always)]
fn e(vm: &Vm, index: u8) -> m128d {
    debug_assert!((index as usize) < MAX_FLOAT_REG);
    unsafe { *vm.reg.e.get_unchecked(index as usize) }
}

#[inline(always)]
fn set_e(vm: &mut Vm, index: u8, value: m128d) {
    debug_assert!((index as usize) < MAX_FLOAT_REG);
    unsafe { *vm.reg.e.get_unchecked_mut(index as usize) = value }
}

#[inline(always)]
fn a(vm: &Vm, index: u8) -> m128d {
    debug_assert!((index as usize) < MAX_FLOAT_REG);
    unsafe { *vm.reg.a.get_unchecked(index as usize) }
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

#[cfg(not(feature = "nearest-only-audit"))]
#[inline(always)]
fn rounding_mode(vm: &Vm) -> RoundingMode {
    RoundingMode::from_fprc(vm.get_rounding_mode())
}

#[cfg(feature = "nearest-only-audit")]
#[inline(always)]
fn rounding_mode(_: &Vm) -> RoundingMode {
    RoundingMode::Nearest
}

#[inline(always)]
fn memory_mask(mode: u8) -> u64 {
    match mode {
        MEM_L1 => SCRATCHPAD_L1_MASK,
        MEM_L2 => SCRATCHPAD_L2_MASK,
        MEM_L3 => SCRATCHPAD_L3_MASK,
        _ => unreachable!(),
    }
}

#[inline(always)]
fn scratchpad_src_ix(vm: &Vm, instr: &CompactInstr) -> usize {
    let address = if instr.src == NO_REG {
        instr.imm
    } else {
        r(vm, instr.src).wrapping_add(instr.imm)
    };
    ((address & memory_mask(instr.mode)) >> 3) as usize
}

#[inline(always)]
fn scratchpad_dst_ix(vm: &Vm, instr: &CompactInstr) -> usize {
    ((r(vm, instr.dst).wrapping_add(instr.imm) & memory_mask(instr.mode)) >> 3) as usize
}

#[inline(always)]
fn scratch(vm: &Vm, index: usize) -> u64 {
    debug_assert!(index < vm.scratchpad.len());
    unsafe { *vm.scratchpad.get_unchecked(index) }
}

fn exec_nop(_: &mut Vm, _: &CompactInstr) {}

fn exec_iadd_rs(vm: &mut Vm, instr: &CompactInstr) {
    let addend = (r(vm, instr.src) << instr.mode).wrapping_add(instr.imm);
    set_r(vm, instr.dst, r(vm, instr.dst).wrapping_add(addend));
}

fn exec_iadd_m(vm: &mut Vm, instr: &CompactInstr) {
    let value = scratch(vm, scratchpad_src_ix(vm, instr));
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
    let value = scratch(vm, scratchpad_src_ix(vm, instr));
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
    let value = scratch(vm, scratchpad_src_ix(vm, instr));
    set_r(vm, instr.dst, r(vm, instr.dst).wrapping_mul(value));
}

fn exec_imulh_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, mulh(r(vm, instr.src), r(vm, instr.dst)));
}

fn exec_imulh_m(vm: &mut Vm, instr: &CompactInstr) {
    let source = scratch(vm, scratchpad_src_ix(vm, instr));
    set_r(vm, instr.dst, mulh(source, r(vm, instr.dst)));
}

fn exec_ismulh_r(vm: &mut Vm, instr: &CompactInstr) {
    set_r(vm, instr.dst, smulh(r(vm, instr.src), r(vm, instr.dst)));
}

fn exec_ismulh_m(vm: &mut Vm, instr: &CompactInstr) {
    let source = scratch(vm, scratchpad_src_ix(vm, instr));
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
    let source = scratch(vm, scratchpad_src_ix(vm, instr));
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
    let source = m128i::from_u64(0, scratch(vm, scratchpad_src_ix(vm, instr))).lower_to_m128d();
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
    let source = m128i::from_u64(0, scratch(vm, scratchpad_src_ix(vm, instr))).lower_to_m128d();
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
    let source = m128i::from_u64(0, scratch(vm, scratchpad_src_ix(vm, instr))).lower_to_m128d();
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
    let index = scratchpad_dst_ix(vm, instr);
    debug_assert!(index < vm.scratchpad.len());
    unsafe { *vm.scratchpad.get_unchecked_mut(index) = r(vm, instr.src) }
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

    use rustdom_x::{new_vm, VmMemory};

    #[test]
    fn compact_instruction_is_24_bytes() {
        assert_eq!(size_of::<CompactInstr>(), 24);
    }

    #[test]
    fn directed_cfround_hash_matches_official_randomx() {
        let seed = [
            0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54,
            0x16, 0xe5, 0xb6, 0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74,
            0x6b, 0xb7, 0x8e, 0xb4, 0xbe, 0x8e,
        ];
        let blob = [
            0x10, 0x10, 0xc5, 0xa2, 0x99, 0xd3, 0x06, 0x5e, 0xd0, 0x66, 0x57, 0x3b, 0x62,
            0xcd, 0xcc, 0x0d, 0x24, 0x3d, 0x8b, 0x71, 0x30, 0xcf, 0x8b, 0xe8, 0x7f, 0xf7,
            0x1e, 0xc3, 0x02, 0xce, 0xdd, 0x31, 0xdb, 0x9f, 0x6f, 0x4f, 0x6e, 0x10, 0xe8,
            0x5d, 0x5a, 0x4c, 0x10, 0x76, 0xf9, 0xef, 0x57, 0xaa, 0xbb, 0x92, 0x00, 0x4f,
            0xaf, 0xeb, 0xc6, 0x8b, 0x9a, 0x54, 0xbc, 0x9d, 0x35, 0x84, 0xec, 0x8f, 0x94,
            0x3e, 0x94, 0x9b, 0xc4, 0xc3, 0x72, 0xa5, 0x01, 0x00, 0x00, 0x00,
        ];
        let expected = [
            0xc1, 0x9a, 0xe2, 0xf2, 0xf5, 0x0a, 0x2e, 0x33, 0xec, 0x73, 0x74, 0x84, 0xe6,
            0xc4, 0x47, 0xd9, 0xb0, 0xff, 0xe4, 0x44, 0x31, 0xa3, 0x32, 0x01, 0x02, 0x6b,
            0xa9, 0xec, 0xa7, 0x0f, 0xda, 0x95,
        ];

        let memory = Arc::new(VmMemory::light(&seed));
        let mut rich = new_vm(Arc::clone(&memory));
        let mut compact = new_vm(memory);
        let rich_hash = rich.calculate_hash(&blob);
        let compact_hash = calculate_hash(&mut compact, &blob);

        assert_eq!(rich_hash.as_bytes(), &expected);
        assert_eq!(compact_hash.as_bytes(), &expected);
        assert_eq!(rich.reg.to_bytes(), compact.reg.to_bytes());
        assert_eq!(rich.scratchpad, compact.scratchpad);
    }

    #[test]
    fn nearest_only_hash_matches_rich_and_official_randomx() {
        let seed = [
            0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54,
            0x16, 0xe5, 0xb6, 0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74,
            0x6b, 0xb7, 0x8e, 0xb4, 0xbe, 0x8e,
        ];
        let blob = [
            0x10, 0x10, 0xc5, 0xa2, 0x99, 0xd3, 0x06, 0x5e, 0xd0, 0x66, 0x57, 0x3b, 0x62,
            0xcd, 0xcc, 0x0d, 0x24, 0x3d, 0x8b, 0x71, 0x30, 0xcf, 0x8b, 0xe8, 0x7f, 0xf7,
            0x1e, 0xc3, 0x02, 0xce, 0xdd, 0x31, 0xdb, 0x9f, 0x6f, 0x4f, 0x6e, 0x10, 0xe8,
            0x5d, 0x5a, 0x4c, 0x10, 0x76, 0xf9, 0xef, 0x57, 0xaa, 0xbb, 0x92, 0x00, 0x4f,
            0xaf, 0xeb, 0xc6, 0x8b, 0x9a, 0x54, 0xbc, 0x9d, 0x35, 0x84, 0xec, 0x8f, 0x94,
            0x3e, 0x94, 0x9b, 0xc4, 0xc3, 0x72, 0xa5, 0xf3, 0xb4, 0xe6, 0x1d,
        ];
        let expected = [
            0x04, 0x3f, 0x95, 0xd6, 0xe6, 0x12, 0xd7, 0xc9, 0x68, 0x79, 0xdd, 0x25, 0xab,
            0x78, 0x45, 0x64, 0x81, 0xcf, 0xbb, 0x63, 0x01, 0x43, 0xa5, 0x20, 0x1c, 0x38,
            0x92, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        // This audited vector contains no CFROUND in any of its eight
        // programs, so the original rich VM is a valid nearest-even oracle.
        let memory = Arc::new(VmMemory::light(&seed));
        let mut rich = new_vm(Arc::clone(&memory));
        let mut compact = new_vm(memory);
        let rich_hash = rich.calculate_hash(&blob);
        let compact_hash = calculate_hash(&mut compact, &blob);

        assert_eq!(rich_hash.as_bytes(), &expected);
        assert_eq!(compact_hash.as_bytes(), &expected);
        assert_eq!(rich.reg.to_bytes(), compact.reg.to_bytes());
        assert_eq!(rich.scratchpad, compact.scratchpad);
    }
}
