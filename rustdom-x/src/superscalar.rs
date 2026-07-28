extern crate blake2b_simd;

use self::blake2b_simd::Params;
use std::convert::TryInto;
#[cfg(any(not(feature = "compact-superscalar"), test))]
use std::fmt;
use strum::Display;

use super::common::{mulh, randomx_reciprocal, smulh, u64_from_u32_imm};
use super::program::REG_NEEDS_DISPLACEMENT_IX;

const RANDOMX_SUPERSCALAR_LATENCY: usize = 170;
const CYCLE_MAP_SIZE: usize = RANDOMX_SUPERSCALAR_LATENCY + 4;
const SUPERSCALAR_MAX_SIZE: usize = 3 * RANDOMX_SUPERSCALAR_LATENCY + 2;
const LOOK_FORWARD_CYCLES: usize = 4;
const MAX_THROWAWAY_COUNT: usize = 256;

#[allow(nonstandard_style)]
#[derive(Copy, Clone, Display, Debug, PartialEq)]
pub enum ScOpcode {
	INVALID = -1,
	ISUB_R = 0,
	IXOR_R = 1,
	IADD_RS = 2,
	IMUL_R = 3,
	IROR_C = 4,
	IADD_C7 = 5,
	IXOR_C7 = 6,
	IADD_C8 = 7,
	IXOR_C8 = 8,
	IADD_C9 = 9,
	IXOR_C9 = 10,
	IMULH_R = 11,
	ISMULH_R = 12,
	IMUL_RCP = 13,
	COUNT = 14,
}

impl ScOpcode {
	fn is_multiplication(self) -> bool {
		self == ScOpcode::IMUL_R
			|| self == ScOpcode::IMULH_R
			|| self == ScOpcode::ISMULH_R
			|| self == ScOpcode::IMUL_RCP
	}
}

#[derive(Copy, Clone)]
struct RegisterInfo {
	pub last_op_group: ScOpcode,
	pub latency: usize,
	pub last_op_par: i32,
}

impl RegisterInfo {
	fn new() -> RegisterInfo {
		RegisterInfo {
			latency: 0,
			last_op_group: ScOpcode::INVALID,
			last_op_par: -1,
		}
	}
}

#[derive(Copy, Clone, Debug)]
pub struct ScInstr<'a> {
	pub info: &'a ScInstrInfo,
	pub dst: i32,
	pub src: i32,
	pub mod_v: u8,
	pub imm32: u32,
	/// Precomputed 64-bit reciprocal for `IMUL_RCP`, or zero otherwise.
	pub reciprocal: u64,
	pub op_group: ScOpcode,
	pub op_group_par: i32,
	pub can_reuse: bool,
	pub group_par_is_source: bool,
}

impl ScInstr<'_> {
	fn null() -> ScInstr<'static> {
		ScInstr {
			info: &NOP,
			dst: -1,
			src: -1,
			mod_v: 0,
			imm32: 0,
			reciprocal: 0,
			op_group: ScOpcode::INVALID,
			can_reuse: false,
			group_par_is_source: false,
			op_group_par: -1,
		}
	}

	pub fn mod_shift(&self) -> u64 {
		((self.mod_v >> 2) % 4) as u64
	}

	fn select_destination(
		&mut self,
		cycle: usize,
		allow_chain_mul: bool,
		registers: &[RegisterInfo; 8],
		generator: &mut Blake2Generator,
	) -> bool {
		let mut available_registers = Vec::with_capacity(8);
		for (i, v) in registers.iter().enumerate() {
			if v.latency <= cycle
				&& (self.can_reuse || i as i32 != self.src)
				&& (allow_chain_mul
					|| self.op_group != ScOpcode::IMUL_R
					|| v.last_op_group != ScOpcode::IMUL_R)
				&& (v.last_op_group != self.op_group || v.last_op_par != self.op_group_par)
				&& (self.info.op != ScOpcode::IADD_RS || i != REG_NEEDS_DISPLACEMENT_IX)
			{
				available_registers.push(i);
			}
		}
		self.select_register(&available_registers, generator, false)
	}

	fn select_source(
		&mut self,
		cycle: usize,
		registers: &[RegisterInfo; 8],
		generator: &mut Blake2Generator,
	) -> bool {
		let mut available_registers = Vec::with_capacity(8);

		for (i, v) in registers.iter().enumerate() {
			if v.latency <= cycle {
				available_registers.push(i);
			}
		}

		if available_registers.len() == 2
			&& self.info.op == ScOpcode::IADD_RS
			&& (available_registers[0] == REG_NEEDS_DISPLACEMENT_IX
				|| available_registers[1] == REG_NEEDS_DISPLACEMENT_IX)
		{
			self.op_group_par = REG_NEEDS_DISPLACEMENT_IX as i32;
			self.src = REG_NEEDS_DISPLACEMENT_IX as i32;
			return true;
		}

		if self.select_register(&available_registers, generator, true) {
			if self.group_par_is_source {
				self.op_group_par = self.src;
			}
			return true;
		}
		false
	}

	fn select_register(
		&mut self,
		available_registers: &[usize],
		generator: &mut Blake2Generator,
		reg_src: bool,
	) -> bool {
		if available_registers.is_empty() {
			return false;
		}
		let index = if available_registers.len() > 1 {
			generator.get_u32() as usize % available_registers.len()
		} else {
			0
		};

		if reg_src {
			self.src = available_registers[index] as i32;
		} else {
			self.dst = available_registers[index] as i32;
		}
		true
	}
}

static SLOT_3L: [&ScInstrInfo; 4] = [&ISUB_R, &IXOR_R, &IMULH_R, &ISMULH_R];
static SLOT_4: [&ScInstrInfo; 2] = [&IROR_C, &IADD_RS];
static SLOT_7: [&ScInstrInfo; 2] = [&IXOR_C7, &IADD_C7];
static SLOT_8: [&ScInstrInfo; 2] = [&IXOR_C8, &IADD_C8];
static SLOT_9: [&ScInstrInfo; 2] = [&IXOR_C9, &IADD_C9];
static SLOT_10: &ScInstrInfo = &IMUL_RCP;

fn is_zero_or_power_of_2(v: u32) -> bool {
	v & v.wrapping_sub(1) == 0
}

impl ScInstr<'_> {
	pub fn create_for_slot<'a>(
		generator: &mut Blake2Generator,
		slot_size: u32,
		fetch_type: u32,
		is_last: bool,
	) -> ScInstr<'a> {
		match slot_size {
			3 => {
				if is_last {
					ScInstr::create(SLOT_3L[(generator.get_byte() & 3) as usize], generator)
				} else {
					ScInstr::create(SLOT_3L[(generator.get_byte() & 1) as usize], generator)
				}
			}
			4 => {
				if fetch_type == 4 && !is_last {
					ScInstr::create(&IMUL_R, generator)
				} else {
					ScInstr::create(SLOT_4[(generator.get_byte() & 1) as usize], generator)
				}
			}
			7 => ScInstr::create(SLOT_7[(generator.get_byte() & 1) as usize], generator),
			8 => ScInstr::create(SLOT_8[(generator.get_byte() & 1) as usize], generator),
			9 => ScInstr::create(SLOT_9[(generator.get_byte() & 1) as usize], generator),
			10 => ScInstr::create(SLOT_10, generator),
			_ => panic!("illegal slot_size {}", slot_size),
		}
	}

	fn create<'a>(info: &'static ScInstrInfo, generator: &mut Blake2Generator) -> ScInstr<'a> {
		match info.op {
			ScOpcode::ISUB_R => ScInstr {
				info,
				dst: -1,
				src: -1,
				mod_v: 0,
				imm32: 0,
				reciprocal: 0,
				op_group: ScOpcode::IADD_RS,
				can_reuse: false,
				group_par_is_source: true,
				op_group_par: 0,
			},
			ScOpcode::IXOR_R => ScInstr {
				info,
				dst: -1,
				src: -1,
				mod_v: 0,
				imm32: 0,
				reciprocal: 0,
				op_group: ScOpcode::IXOR_R,
				can_reuse: false,
				group_par_is_source: true,
				op_group_par: 0,
			},
			ScOpcode::IADD_RS => ScInstr {
				info,
				dst: -1,
				src: -1,
				mod_v: generator.get_byte(),
				imm32: 0,
				reciprocal: 0,
				op_group: ScOpcode::IADD_RS,
				can_reuse: false,
				group_par_is_source: true,
				op_group_par: 0,
			},
			ScOpcode::IMUL_R => ScInstr {
				info,
				dst: -1,
				src: -1,
				mod_v: 0,
				imm32: 0,
				reciprocal: 0,
				op_group: ScOpcode::IMUL_R,
				can_reuse: false,
				group_par_is_source: true,
				op_group_par: 0,
			},
			ScOpcode::IROR_C => {
				let mut imm32;
				while {
					imm32 = generator.get_byte() & 63;
					imm32 == 0
				} {}
				ScInstr {
					info,
					dst: -1,
					src: -1,
					mod_v: 0,
					imm32: imm32 as u32,
					reciprocal: 0,
					op_group: ScOpcode::IROR_C,
					can_reuse: false,
					group_par_is_source: true,
					op_group_par: 0,
				}
			}
			ScOpcode::IADD_C7 | ScOpcode::IADD_C8 | ScOpcode::IADD_C9 => ScInstr {
				info,
				dst: -1,
				src: -1,
				mod_v: 0,
				imm32: generator.get_u32(),
				reciprocal: 0,
				op_group: ScOpcode::IADD_C7,
				can_reuse: false,
				group_par_is_source: false,
				op_group_par: -1,
			},
			ScOpcode::IXOR_C7 | ScOpcode::IXOR_C8 | ScOpcode::IXOR_C9 => ScInstr {
				info,
				dst: -1,
				src: -1,
				mod_v: 0,
				imm32: generator.get_u32(),
				reciprocal: 0,
				op_group: ScOpcode::IXOR_C7,
				can_reuse: false,
				group_par_is_source: false,
				op_group_par: -1,
			},
			ScOpcode::IMULH_R => ScInstr {
				info,
				dst: -1,
				src: -1,
				mod_v: 0,
				imm32: 0,
				reciprocal: 0,
				op_group: ScOpcode::IMULH_R,
				group_par_is_source: true,
				can_reuse: false,
				op_group_par: generator.get_u32() as i32,
			},
			ScOpcode::ISMULH_R => ScInstr {
				info,
				dst: -1,
				src: -1,
				mod_v: 0,
				imm32: 0,
				reciprocal: 0,
				op_group: ScOpcode::ISMULH_R,
				group_par_is_source: true,
				can_reuse: false,
				op_group_par: generator.get_u32() as i32,
			},
			ScOpcode::IMUL_RCP => {
				let mut imm32;
				while {
					imm32 = generator.get_u32();
					is_zero_or_power_of_2(imm32)
				} {}
				ScInstr {
					info,
					dst: -1,
					src: -1,
					mod_v: 0,
					imm32,
					reciprocal: {
						#[cfg(feature = "precompute-reciprocal")]
						{
							randomx_reciprocal(imm32 as u64)
						}
						#[cfg(not(feature = "precompute-reciprocal"))]
						{
							0
						}
					},
					op_group: ScOpcode::IMUL_RCP,
					can_reuse: false,
					group_par_is_source: true,
					op_group_par: -1,
				}
			}
			ScOpcode::INVALID | ScOpcode::COUNT => panic!("invalid opcode {} here", info.op),
		}
	}
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(u8)]
pub enum ExecutionPort {
	NULL = 0,
	P0 = 1,
	P1 = 2,
	P5 = 4,
	P01 = ExecutionPort::P0 as u8 | ExecutionPort::P1 as u8,
	P05 = ExecutionPort::P0 as u8 | ExecutionPort::P5 as u8,
	P015 = ExecutionPort::P0 as u8 | ExecutionPort::P1 as u8 | ExecutionPort::P5 as u8,
}

impl ExecutionPort {
	fn is(self, check: ExecutionPort) -> bool {
		(self as u8 & check as u8) != 0
	}
}

#[derive(Debug)]
pub struct ScMacroOp {
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	#[allow(dead_code)]
	name: &'static str,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	size: usize,
	latency: usize,
	uop1: ExecutionPort,
	uop2: ExecutionPort,
	dependent: bool,
}

impl ScMacroOp {
	pub const fn new(
		#[allow(unused_variables)]
		name: &'static str,
		#[allow(unused_variables)]
		size: usize,
		latency: usize,
		uop1: ExecutionPort,
		uop2: ExecutionPort,
	) -> ScMacroOp {
		ScMacroOp {
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			name,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			size,
			latency,
			uop1,
			uop2,
			dependent: false,
		}
	}
	pub const fn new_dep(
		#[allow(unused_variables)]
		name: &'static str,
		#[allow(unused_variables)]
		size: usize,
		latency: usize,
		uop1: ExecutionPort,
		uop2: ExecutionPort,
	) -> ScMacroOp {
		ScMacroOp {
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			name,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			size,
			latency,
			uop1,
			uop2,
			dependent: true,
		}
	}

	pub fn is_eliminated(&self) -> bool {
		self.uop1 == ExecutionPort::NULL
	}

	pub fn is_simple(&self) -> bool {
		self.uop2 == ExecutionPort::NULL
	}
}

static MOP_SUB_RR: ScMacroOp =
	ScMacroOp::new("SUB_RR", 3, 1, ExecutionPort::P015, ExecutionPort::NULL);
static MOP_XOR_RR: ScMacroOp =
	ScMacroOp::new("XOR_RR", 3, 1, ExecutionPort::P015, ExecutionPort::NULL);
static MOP_IMUL_R: ScMacroOp = ScMacroOp::new("IMUL_R", 3, 4, ExecutionPort::P1, ExecutionPort::P5);
static MOP_MUL_R: ScMacroOp = ScMacroOp::new("MUL_R", 3, 4, ExecutionPort::P1, ExecutionPort::P5);
static MOP_MOV_RR: ScMacroOp =
	ScMacroOp::new("MOV_RR", 3, 1, ExecutionPort::NULL, ExecutionPort::NULL);

static MOP_LEA_SIB: ScMacroOp =
	ScMacroOp::new("LEA_SIB", 4, 1, ExecutionPort::P01, ExecutionPort::NULL);
static MOP_IMUL_RR_DEP: ScMacroOp =
	ScMacroOp::new_dep("IMUL_RR_DEP", 4, 3, ExecutionPort::P1, ExecutionPort::NULL);
static MOP_ROR_RI: ScMacroOp =
	ScMacroOp::new("ROR_RI", 4, 1, ExecutionPort::P05, ExecutionPort::NULL);

static MOP_ADD_RI: ScMacroOp =
	ScMacroOp::new("ADD_RI", 7, 1, ExecutionPort::P015, ExecutionPort::NULL);
static MOP_XOR_RI: ScMacroOp =
	ScMacroOp::new("XOR_RI", 7, 1, ExecutionPort::P015, ExecutionPort::NULL);

static MOP_MOV_RI64: ScMacroOp =
	ScMacroOp::new("MOV_RI64", 10, 1, ExecutionPort::P015, ExecutionPort::NULL);

static MOP_IMUL_RR: ScMacroOp =
	ScMacroOp::new("IMUL_RR", 4, 3, ExecutionPort::P1, ExecutionPort::NULL);

#[allow(nonstandard_style)]
#[derive(Debug)]
pub struct ScInstrInfo {
	pub op: ScOpcode,
	pub macro_ops: &'static [&'static ScMacroOp],
	pub result_op: usize,
	pub src_op: i32,
	pub dst_op: i32,
}

impl ScInstrInfo {
	pub const fn new(
		op: ScOpcode,
		macro_ops: &'static [&ScMacroOp],
		result_op: usize,
		dst_op: i32,
		src_op: i32,
	) -> ScInstrInfo {
		ScInstrInfo {
			op,
			macro_ops,
			result_op,
			src_op,
			dst_op,
		}
	}

	pub fn size(&self) -> usize {
		self.macro_ops.len()
	}

	pub fn macro_op(&self, i: usize) -> &'static ScMacroOp {
		self.macro_ops[i]
	}
}

static NOP: ScInstrInfo = ScInstrInfo::new(ScOpcode::INVALID, &[], 0, 0, 0);

static ISUB_R: ScInstrInfo = ScInstrInfo::new(ScOpcode::ISUB_R, &[&MOP_SUB_RR], 0, 0, 0);
static IXOR_R: ScInstrInfo = ScInstrInfo::new(ScOpcode::IXOR_R, &[&MOP_XOR_RR], 0, 0, 0);
static IADD_RS: ScInstrInfo = ScInstrInfo::new(ScOpcode::IADD_RS, &[&MOP_LEA_SIB], 0, 0, 0);
static IMUL_R: ScInstrInfo = ScInstrInfo::new(ScOpcode::IMUL_R, &[&MOP_IMUL_RR], 0, 0, 0);
static IROR_C: ScInstrInfo = ScInstrInfo::new(ScOpcode::IROR_C, &[&MOP_ROR_RI], 0, 0, -1);

static IADD_C7: ScInstrInfo = ScInstrInfo::new(ScOpcode::IADD_C7, &[&MOP_ADD_RI], 0, 0, -1);
static IXOR_C7: ScInstrInfo = ScInstrInfo::new(ScOpcode::IXOR_C7, &[&MOP_XOR_RI], 0, 0, -1);
static IADD_C8: ScInstrInfo = ScInstrInfo::new(ScOpcode::IADD_C8, &[&MOP_ADD_RI], 0, 0, -1);
static IXOR_C8: ScInstrInfo = ScInstrInfo::new(ScOpcode::IXOR_C8, &[&MOP_XOR_RI], 0, 0, -1);
static IADD_C9: ScInstrInfo = ScInstrInfo::new(ScOpcode::IADD_C9, &[&MOP_ADD_RI], 0, 0, -1);
static IXOR_C9: ScInstrInfo = ScInstrInfo::new(ScOpcode::IXOR_C9, &[&MOP_XOR_RI], 0, 0, -1);

static IMULH_R: ScInstrInfo = ScInstrInfo::new(
	ScOpcode::IMULH_R,
	&[&MOP_MOV_RR, &MOP_MUL_R, &MOP_MOV_RR],
	1,
	0,
	1,
);
static ISMULH_R: ScInstrInfo = ScInstrInfo::new(
	ScOpcode::ISMULH_R,
	&[&MOP_MOV_RR, &MOP_IMUL_R, &MOP_MOV_RR],
	1,
	0,
	1,
);
static IMUL_RCP: ScInstrInfo = ScInstrInfo::new(
	ScOpcode::IMUL_RCP,
	&[&MOP_MOV_RI64, &MOP_IMUL_RR_DEP],
	1,
	1,
	-1,
);

const BLAKE_GEN_DATA_LEN: usize = 64;
pub struct Blake2Generator {
	index: usize,
	data: [u8; BLAKE_GEN_DATA_LEN],
	gen_params: Params,
}

impl Blake2Generator {
	pub fn new(seed: &[u8], nonce: u32) -> Blake2Generator {
		let mut params = Params::new();
		params.hash_length(BLAKE_GEN_DATA_LEN);

		let mut key: [u8; 60] = [0; 60];
		// RandomX's Blake2Generator uses at most the first 60 seed bytes;
		// Argon2 cache initialization still consumes the complete key.
		let copied = seed.len().min(BLAKE_GEN_DATA_LEN - 4);
		key[..copied].copy_from_slice(&seed[..copied]);

		let mut data: [u8; BLAKE_GEN_DATA_LEN] = [0; BLAKE_GEN_DATA_LEN];
		data[..BLAKE_GEN_DATA_LEN - 4].copy_from_slice(&key);
		data[BLAKE_GEN_DATA_LEN - 4..BLAKE_GEN_DATA_LEN].copy_from_slice(&nonce.to_le_bytes());

		Blake2Generator {
			index: BLAKE_GEN_DATA_LEN,
			data,
			gen_params: params,
		}
	}

	pub fn get_byte(&mut self) -> u8 {
		self.check_data(1);
		let v = self.data[self.index];
		self.index += 1;
		v
	}

	pub fn get_u32(&mut self) -> u32 {
		self.check_data(4);
		let v = u32::from_le_bytes(self.data[self.index..(self.index + 4)].try_into().unwrap());
		self.index += 4;
		v
	}
	fn check_data(&mut self, needed: usize) {
		if self.index + needed > BLAKE_GEN_DATA_LEN {
			let out = self.gen_params.hash(&self.data);
			self.data = *out.as_array();
			self.index = 0;
		}
	}
}

pub struct DecoderBuffer {
	index: u32,
	counts: &'static [u32],
}

static BUFFER_484: DecoderBuffer = DecoderBuffer {
	index: 0,
	counts: &[4, 8, 4],
};
static BUFFER_7333: DecoderBuffer = DecoderBuffer {
	index: 1,
	counts: &[7, 3, 3, 3],
};
static BUFFER_3733: DecoderBuffer = DecoderBuffer {
	index: 2,
	counts: &[3, 7, 3, 3],
};
static BUFFER_493: DecoderBuffer = DecoderBuffer {
	index: 3,
	counts: &[4, 9, 3],
};
static BUFFER_4444: DecoderBuffer = DecoderBuffer {
	index: 4,
	counts: &[4, 4, 4, 4],
};
static BUFFFER_3310: DecoderBuffer = DecoderBuffer {
	index: 5,
	counts: &[3, 3, 10],
};

static DECODE_BUFFERS: [&DecoderBuffer; 4] = [&BUFFER_484, &BUFFER_7333, &BUFFER_3733, &BUFFER_493];

impl DecoderBuffer {
	fn initial() -> DecoderBuffer {
		DecoderBuffer {
			index: 0,
			counts: &[],
		}
	}

	pub fn size(&self) -> usize {
		self.counts.len()
	}

	pub fn fetch_next(
		&self,
		instr: &ScInstr,
		decode_cycle: usize,
		mul_count: usize,
		generator: &mut Blake2Generator,
	) -> &'static DecoderBuffer {
		if instr.info.op == ScOpcode::IMULH_R || instr.info.op == ScOpcode::ISMULH_R {
			return &BUFFFER_3310;
		}
		if mul_count < decode_cycle + 1 {
			return &BUFFER_4444;
		}
		if instr.info.op == ScOpcode::IMUL_RCP {
			return if generator.get_byte() & 0x1 == 1 {
				&BUFFER_484
			} else {
				&BUFFER_493
			};
		}
		let ix = generator.get_byte();
		DECODE_BUFFERS[(ix & 3) as usize]
	}
}

#[cfg(feature = "compact-superscalar")]
#[derive(Copy, Clone)]
#[repr(u8)]
enum ScExecOpcode {
	IsubR,
	IxorR,
	IaddRs,
	ImulR,
	IrorC,
	IaddC,
	IxorC,
	ImulhR,
	IsmulhR,
	ImulRcp,
}

#[cfg(feature = "compact-superscalar")]
#[derive(Copy, Clone)]
struct ScExecInstr {
	immediate: u64,
	pair_handler_offset: u16,
	dst_offset: u8,
	src_offset: u8,
	opcode: ScExecOpcode,
}

#[cfg(feature = "compact-superscalar")]
impl ScExecInstr {
	fn decode(instr: &ScInstr) -> Self {
		let (opcode, immediate) = match instr.info.op {
			ScOpcode::ISUB_R => (ScExecOpcode::IsubR, 0),
			ScOpcode::IXOR_R => (ScExecOpcode::IxorR, 0),
			ScOpcode::IADD_RS => (ScExecOpcode::IaddRs, instr.mod_shift()),
			ScOpcode::IMUL_R => (ScExecOpcode::ImulR, 0),
			ScOpcode::IROR_C => (ScExecOpcode::IrorC, instr.imm32 as u64),
			ScOpcode::IADD_C7 | ScOpcode::IADD_C8 | ScOpcode::IADD_C9 => {
				(ScExecOpcode::IaddC, u64_from_u32_imm(instr.imm32))
			}
			ScOpcode::IXOR_C7 | ScOpcode::IXOR_C8 | ScOpcode::IXOR_C9 => {
				(ScExecOpcode::IxorC, u64_from_u32_imm(instr.imm32))
			}
			ScOpcode::IMULH_R => (ScExecOpcode::ImulhR, 0),
			ScOpcode::ISMULH_R => (ScExecOpcode::IsmulhR, 0),
			ScOpcode::IMUL_RCP => (ScExecOpcode::ImulRcp, instr.reciprocal),
			ScOpcode::COUNT | ScOpcode::INVALID => {
				panic!("invalid superscalar opcode in executable program")
			}
		};
		#[cfg(feature = "unchecked-superscalar")]
		{
			assert!((0..8).contains(&instr.dst));
			assert!(instr.src == -1 || (0..8).contains(&instr.src));
		}
		ScExecInstr {
			immediate,
			pair_handler_offset: 0,
			dst_offset: instr.dst as u8 * std::mem::size_of::<u64>() as u8,
			src_offset: if instr.src < 0 {
				0
			} else {
				instr.src as u8 * std::mem::size_of::<u64>() as u8
			},
			opcode,
		}
	}
}

pub struct ScProgram<'a> {
	#[cfg(all(feature = "compact-superscalar", not(test)))]
	_marker: std::marker::PhantomData<&'a ()>,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub prog: Vec<ScInstr<'a>>,
	#[cfg(feature = "compact-superscalar")]
	exec: Vec<ScExecInstr>,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub asic_latencies: Vec<usize>,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub cpu_latencies: Vec<usize>,
	address_reg: u8,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub ipc: f64,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub code_size: usize,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub macro_ops: usize,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub decode_cycles: usize,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub cpu_latency: usize,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub asic_latency: usize,
	#[cfg(any(not(feature = "compact-superscalar"), test))]
	pub mul_count: usize,
}

#[cfg(any(not(feature = "compact-superscalar"), test))]
impl fmt::Display for ScProgram<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for instr in &self.prog {
			writeln!(
				f,
				"op: {}, src: {}, dst: {}",
				instr.info.op, instr.src, instr.dst
			)
			.unwrap();
		}
		Ok(())
	}
}

impl ScProgram<'_> {
	pub fn generate(generator: &mut Blake2Generator) -> ScProgram<'static> {
		let mut prog = Vec::with_capacity(SUPERSCALAR_MAX_SIZE);

		let mut port_busy = [[ExecutionPort::NULL; 3]; CYCLE_MAP_SIZE];
		let mut registers = [RegisterInfo::new(); 8];

		let mut macro_op_index = 0;
		#[cfg(any(not(feature = "compact-superscalar"), test))]
		let mut code_size = 0;
		#[cfg(any(not(feature = "compact-superscalar"), test))]
		let mut macro_op_count = 0;
		let mut cycle = 0;
		let mut dep_cycle = 0;
		#[cfg(any(not(feature = "compact-superscalar"), test))]
		let mut retire_cycle = 0;
		let mut ports_saturated = false;
		let mut program_size = 0;
		let mut mul_count = 0;
		let mut decode_cycle = 0;
		let mut throw_away_count = 0;

		let mut decode_buffer = &DecoderBuffer::initial();
		let mut current_instr = ScInstr::null();
		while decode_cycle < RANDOMX_SUPERSCALAR_LATENCY
			&& !ports_saturated
			&& program_size < SUPERSCALAR_MAX_SIZE
		{
			decode_buffer = decode_buffer.fetch_next(&current_instr, decode_cycle, mul_count, generator);
			let mut buffer_index = 0;
			while buffer_index < decode_buffer.size() {
				let top_cycle = cycle;
				if macro_op_index >= current_instr.info.size() {
					if ports_saturated || program_size >= SUPERSCALAR_MAX_SIZE {
						break;
					}

					current_instr = ScInstr::create_for_slot(
						generator,
						decode_buffer.counts[buffer_index],
						decode_buffer.index,
						decode_buffer.size() == buffer_index + 1,
					);
					macro_op_index = 0
				}

				let mop = current_instr.info.macro_op(macro_op_index);
				let schedule_cycle_mop = schedule_mop(false, mop, &mut port_busy, cycle, dep_cycle);
				if schedule_cycle_mop.is_none() {
					ports_saturated = true;
					break;
				}

				let mut schedule_cycle = schedule_cycle_mop.unwrap();
				if macro_op_index as i32 == current_instr.info.src_op {
					let mut forward = 0;
					while forward < LOOK_FORWARD_CYCLES
						&& !current_instr.select_source(schedule_cycle, &registers, generator)
					{
						schedule_cycle += 1;
						cycle += 1;
						forward += 1;
					}

					if forward == LOOK_FORWARD_CYCLES {
						if throw_away_count < MAX_THROWAWAY_COUNT {
							throw_away_count += 1;
							macro_op_index = current_instr.info.size();
							continue;
						}
						current_instr = ScInstr::null();
						break;
					}
				}
				if macro_op_index as i32 == current_instr.info.dst_op {
					let mut forward = 0;
					while forward < LOOK_FORWARD_CYCLES
						&& !current_instr.select_destination(
							schedule_cycle,
							throw_away_count > 0,
							&registers,
							generator,
						) {
						schedule_cycle += 1;
						cycle += 1;
						forward += 1;
					}
					if forward == LOOK_FORWARD_CYCLES {
						if throw_away_count < MAX_THROWAWAY_COUNT {
							throw_away_count += 1;
							macro_op_index = current_instr.info.size();
							continue;
						}
						current_instr = ScInstr::null();
						break;
					}
				}
				throw_away_count = 0;

				let schedule_cycle_mop =
					schedule_mop(true, mop, &mut port_busy, schedule_cycle, schedule_cycle);
				if schedule_cycle_mop.is_none() {
					ports_saturated = true;
					break;
				}
				schedule_cycle = schedule_cycle_mop.unwrap();
				dep_cycle = schedule_cycle + mop.latency;

				if macro_op_index == current_instr.info.result_op {
					let ri = &mut registers[current_instr.dst as usize];
					#[cfg(any(not(feature = "compact-superscalar"), test))]
					{
					retire_cycle = dep_cycle;
					}
					ri.latency = dep_cycle;
					ri.last_op_group = current_instr.op_group;
					ri.last_op_par = current_instr.op_group_par;
				}
				#[cfg(any(not(feature = "compact-superscalar"), test))]
				{
					code_size += mop.size;
				}
				buffer_index += 1;
				macro_op_index += 1;
				#[cfg(any(not(feature = "compact-superscalar"), test))]
				{
					macro_op_count += 1;
				}

				if schedule_cycle >= RANDOMX_SUPERSCALAR_LATENCY {
					ports_saturated = true;
				}
				cycle = top_cycle;

				if macro_op_index >= current_instr.info.size() {
					if current_instr.info.op.is_multiplication() {
						mul_count += 1;
					}
					prog.push(current_instr);
					program_size += 1;
				}
			}
			cycle += 1;
			decode_cycle += 1;
		}

		#[cfg(any(not(feature = "compact-superscalar"), test))]
		let ipc = macro_op_count as f64 / retire_cycle as f64;
		let mut asic_latencies = vec![0; 8];
		for &instr in prog.iter().take(program_size) {
			let lat_dst = asic_latencies[instr.dst as usize] + 1;
			let lat_src = if instr.src < 0 || instr.src == instr.dst {
				0
			} else {
				asic_latencies[instr.src as usize] + 1
			};
			asic_latencies[instr.dst as usize] = lat_dst.max(lat_src);
		}

		let mut asic_latency_max = 0;
		let mut address_reg = 0;
		#[cfg(any(not(feature = "compact-superscalar"), test))]
		let mut cpu_latencies = vec![0; 8];
		for i in 0..8 {
			if asic_latencies[i] > asic_latency_max {
				asic_latency_max = asic_latencies[i];
				address_reg = i;
			}
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			{
				cpu_latencies[i] = registers[i].latency;
			}
		}

		#[cfg(feature = "compact-superscalar")]
		let mut exec: Vec<_> = prog.iter().map(ScExecInstr::decode).collect();
		#[cfg(feature = "compact-superscalar")]
		for pair in exec.chunks_exact_mut(2) {
			let pair_index = pair[0].opcode as usize * 10 + pair[1].opcode as usize;
			pair[0].pair_handler_offset =
				(pair_index * std::mem::size_of::<PairHandler>()) as u16;
		}

		ScProgram {
			#[cfg(all(feature = "compact-superscalar", not(test)))]
			_marker: std::marker::PhantomData,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			prog,
			#[cfg(feature = "compact-superscalar")]
			exec,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			asic_latencies,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			cpu_latencies,
			address_reg: address_reg as u8,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			ipc,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			mul_count,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			cpu_latency: retire_cycle,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			asic_latency: asic_latency_max,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			code_size,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			macro_ops: macro_op_count,
			#[cfg(any(not(feature = "compact-superscalar"), test))]
			decode_cycles: decode_cycle,
		}
	}

	#[inline(always)]
	pub fn address_register(&self, registers: &[u64; 8]) -> u64 {
		debug_assert!(self.address_reg < 8);
		#[cfg(feature = "unchecked-superscalar")]
		unsafe {
			// `address_reg` is private and is selected only by `generate`'s
			// fixed `0..8` loop.
			*registers.get_unchecked(self.address_reg as usize)
		}
		#[cfg(not(feature = "unchecked-superscalar"))]
		{
			registers[self.address_reg as usize]
		}
	}

	#[cfg(any(not(feature = "compact-superscalar"), test))]
	fn execute_rich(&self, ds: &mut [u64; 8]) {
		for instr in &self.prog {
			let dst = instr.dst as usize;
			let src = instr.src as usize;
			match instr.info.op {
				ScOpcode::ISUB_R => ds[dst] = ds[dst].wrapping_sub(ds[src]),
				ScOpcode::IXOR_R => ds[dst] ^= ds[src],
				ScOpcode::IADD_RS => ds[dst] = ds[dst].wrapping_add(ds[src] << instr.mod_shift()),
				ScOpcode::IMUL_R => {
					ds[dst] = ds[dst].wrapping_mul(ds[src]);
				}
				ScOpcode::IROR_C => ds[dst] = ds[dst].rotate_right(instr.imm32),
				ScOpcode::IADD_C7 | ScOpcode::IADD_C8 | ScOpcode::IADD_C9 => {
					ds[dst] = ds[dst].wrapping_add(u64_from_u32_imm(instr.imm32));
				}
				ScOpcode::IXOR_C7 | ScOpcode::IXOR_C8 | ScOpcode::IXOR_C9 => {
					ds[dst] ^= u64_from_u32_imm(instr.imm32);
				}
				ScOpcode::IMULH_R => ds[dst] = mulh(ds[dst], ds[src]),
				ScOpcode::ISMULH_R => ds[dst] = smulh(ds[dst], ds[src]),
				ScOpcode::IMUL_RCP => {
					#[cfg(feature = "precompute-reciprocal")]
					{
						ds[dst] = ds[dst].wrapping_mul(instr.reciprocal)
					}
					#[cfg(not(feature = "precompute-reciprocal"))]
					{
						ds[dst] = ds[dst]
							.wrapping_mul(randomx_reciprocal(instr.imm32 as u64))
					}
				}
				ScOpcode::COUNT => panic!("COUNT execution tried"),
				ScOpcode::INVALID => panic!("INVALLID execution tried"),
			}
		}
	}

	#[cfg(all(
		feature = "compact-superscalar",
		any(not(feature = "unchecked-superscalar"), test)
	))]
	fn execute_compact(&self, ds: &mut [u64; 8]) {
		for instr in &self.exec {
			let dst = instr.dst_offset as usize / std::mem::size_of::<u64>();
			let src = instr.src_offset as usize / std::mem::size_of::<u64>();
			match instr.opcode {
				ScExecOpcode::IsubR => ds[dst] = ds[dst].wrapping_sub(ds[src]),
				ScExecOpcode::IxorR => ds[dst] ^= ds[src],
				ScExecOpcode::IaddRs => {
					ds[dst] = ds[dst].wrapping_add(ds[src] << instr.immediate)
				}
				ScExecOpcode::ImulR => ds[dst] = ds[dst].wrapping_mul(ds[src]),
				ScExecOpcode::IrorC => ds[dst] = ds[dst].rotate_right(instr.immediate as u32),
				ScExecOpcode::IaddC => ds[dst] = ds[dst].wrapping_add(instr.immediate),
				ScExecOpcode::IxorC => ds[dst] ^= instr.immediate,
				ScExecOpcode::ImulhR => ds[dst] = mulh(ds[dst], ds[src]),
				ScExecOpcode::IsmulhR => ds[dst] = smulh(ds[dst], ds[src]),
				ScExecOpcode::ImulRcp => {
					ds[dst] = ds[dst].wrapping_mul(instr.immediate)
				}
			}
		}
	}

	#[cfg(feature = "unchecked-superscalar")]
	fn execute_compact_unchecked(&self, ds: &mut [u64; 8]) {
		let mut chunks = self.exec.chunks_exact(32);
		for chunk in &mut chunks {
			execute_compact_pair_unchecked(ds, &chunk[0], &chunk[1]);
			execute_compact_pair_unchecked(ds, &chunk[2], &chunk[3]);
			execute_compact_pair_unchecked(ds, &chunk[4], &chunk[5]);
			execute_compact_pair_unchecked(ds, &chunk[6], &chunk[7]);
			execute_compact_pair_unchecked(ds, &chunk[8], &chunk[9]);
			execute_compact_pair_unchecked(ds, &chunk[10], &chunk[11]);
			execute_compact_pair_unchecked(ds, &chunk[12], &chunk[13]);
			execute_compact_pair_unchecked(ds, &chunk[14], &chunk[15]);
			execute_compact_pair_unchecked(ds, &chunk[16], &chunk[17]);
			execute_compact_pair_unchecked(ds, &chunk[18], &chunk[19]);
			execute_compact_pair_unchecked(ds, &chunk[20], &chunk[21]);
			execute_compact_pair_unchecked(ds, &chunk[22], &chunk[23]);
			execute_compact_pair_unchecked(ds, &chunk[24], &chunk[25]);
			execute_compact_pair_unchecked(ds, &chunk[26], &chunk[27]);
			execute_compact_pair_unchecked(ds, &chunk[28], &chunk[29]);
			execute_compact_pair_unchecked(ds, &chunk[30], &chunk[31]);
		}
		let remainder = chunks.remainder();
		let mut pairs = remainder.chunks_exact(2);
		for pair in &mut pairs {
			execute_compact_pair_unchecked(ds, &pair[0], &pair[1]);
		}
		if let Some(instruction) = pairs.remainder().first() {
			execute_compact_instruction_unchecked(ds, instruction);
		}
	}

	pub fn execute(&self, ds: &mut [u64; 8]) {
		#[cfg(not(feature = "compact-superscalar"))]
		self.execute_rich(ds);

		#[cfg(all(
			feature = "compact-superscalar",
			not(feature = "unchecked-superscalar")
		))]
		self.execute_compact(ds);

		#[cfg(feature = "unchecked-superscalar")]
		self.execute_compact_unchecked(ds);
	}
}

#[cfg(feature = "compact-superscalar")]
type PairHandler = fn(&mut [u64; 8], &ScExecInstr, &ScExecInstr);

#[cfg(feature = "unchecked-superscalar")]
static PAIR_HANDLERS: [PairHandler; 100] = [
	execute_compact_pair_known::<0, 0>, execute_compact_pair_known::<0, 1>,
	execute_compact_pair_known::<0, 2>, execute_compact_pair_known::<0, 3>,
	execute_compact_pair_known::<0, 4>, execute_compact_pair_known::<0, 5>,
	execute_compact_pair_known::<0, 6>, execute_compact_pair_known::<0, 7>,
	execute_compact_pair_known::<0, 8>, execute_compact_pair_known::<0, 9>,
	execute_compact_pair_known::<1, 0>, execute_compact_pair_known::<1, 1>,
	execute_compact_pair_known::<1, 2>, execute_compact_pair_known::<1, 3>,
	execute_compact_pair_known::<1, 4>, execute_compact_pair_known::<1, 5>,
	execute_compact_pair_known::<1, 6>, execute_compact_pair_known::<1, 7>,
	execute_compact_pair_known::<1, 8>, execute_compact_pair_known::<1, 9>,
	execute_compact_pair_known::<2, 0>, execute_compact_pair_known::<2, 1>,
	execute_compact_pair_known::<2, 2>, execute_compact_pair_known::<2, 3>,
	execute_compact_pair_known::<2, 4>, execute_compact_pair_known::<2, 5>,
	execute_compact_pair_known::<2, 6>, execute_compact_pair_known::<2, 7>,
	execute_compact_pair_known::<2, 8>, execute_compact_pair_known::<2, 9>,
	execute_compact_pair_known::<3, 0>, execute_compact_pair_known::<3, 1>,
	execute_compact_pair_known::<3, 2>, execute_compact_pair_known::<3, 3>,
	execute_compact_pair_known::<3, 4>, execute_compact_pair_known::<3, 5>,
	execute_compact_pair_known::<3, 6>, execute_compact_pair_known::<3, 7>,
	execute_compact_pair_known::<3, 8>, execute_compact_pair_known::<3, 9>,
	execute_compact_pair_known::<4, 0>, execute_compact_pair_known::<4, 1>,
	execute_compact_pair_known::<4, 2>, execute_compact_pair_known::<4, 3>,
	execute_compact_pair_known::<4, 4>, execute_compact_pair_known::<4, 5>,
	execute_compact_pair_known::<4, 6>, execute_compact_pair_known::<4, 7>,
	execute_compact_pair_known::<4, 8>, execute_compact_pair_known::<4, 9>,
	execute_compact_pair_known::<5, 0>, execute_compact_pair_known::<5, 1>,
	execute_compact_pair_known::<5, 2>, execute_compact_pair_known::<5, 3>,
	execute_compact_pair_known::<5, 4>, execute_compact_pair_known::<5, 5>,
	execute_compact_pair_known::<5, 6>, execute_compact_pair_known::<5, 7>,
	execute_compact_pair_known::<5, 8>, execute_compact_pair_known::<5, 9>,
	execute_compact_pair_known::<6, 0>, execute_compact_pair_known::<6, 1>,
	execute_compact_pair_known::<6, 2>, execute_compact_pair_known::<6, 3>,
	execute_compact_pair_known::<6, 4>, execute_compact_pair_known::<6, 5>,
	execute_compact_pair_known::<6, 6>, execute_compact_pair_known::<6, 7>,
	execute_compact_pair_known::<6, 8>, execute_compact_pair_known::<6, 9>,
	execute_compact_pair_known::<7, 0>, execute_compact_pair_known::<7, 1>,
	execute_compact_pair_known::<7, 2>, execute_compact_pair_known::<7, 3>,
	execute_compact_pair_known::<7, 4>, execute_compact_pair_known::<7, 5>,
	execute_compact_pair_known::<7, 6>, execute_compact_pair_known::<7, 7>,
	execute_compact_pair_known::<7, 8>, execute_compact_pair_known::<7, 9>,
	execute_compact_pair_known::<8, 0>, execute_compact_pair_known::<8, 1>,
	execute_compact_pair_known::<8, 2>, execute_compact_pair_known::<8, 3>,
	execute_compact_pair_known::<8, 4>, execute_compact_pair_known::<8, 5>,
	execute_compact_pair_known::<8, 6>, execute_compact_pair_known::<8, 7>,
	execute_compact_pair_known::<8, 8>, execute_compact_pair_known::<8, 9>,
	execute_compact_pair_known::<9, 0>, execute_compact_pair_known::<9, 1>,
	execute_compact_pair_known::<9, 2>, execute_compact_pair_known::<9, 3>,
	execute_compact_pair_known::<9, 4>, execute_compact_pair_known::<9, 5>,
	execute_compact_pair_known::<9, 6>, execute_compact_pair_known::<9, 7>,
	execute_compact_pair_known::<9, 8>, execute_compact_pair_known::<9, 9>,
];

#[cfg(feature = "unchecked-superscalar")]
#[inline(always)]
fn execute_compact_pair_unchecked(
	ds: &mut [u64; 8],
	first: &ScExecInstr,
	second: &ScExecInstr,
) {
	debug_assert_eq!(first.pair_handler_offset as usize % std::mem::size_of::<PairHandler>(), 0);
	debug_assert!(
		first.pair_handler_offset as usize
			<= std::mem::size_of_val(&PAIR_HANDLERS) - std::mem::size_of::<PairHandler>()
	);
	// `pair_handler_offset` is private and is derived during program generation
	// from two validated `ScExecOpcode` discriminants in the range 0..10.
	let handler = unsafe {
		*PAIR_HANDLERS
			.as_ptr()
			.cast::<u8>()
			.add(first.pair_handler_offset as usize)
			.cast::<PairHandler>()
	};
	handler(ds, first, second);
}

#[cfg(feature = "unchecked-superscalar")]
fn execute_compact_pair_known<const FIRST: u8, const SECOND: u8>(
	ds: &mut [u64; 8],
	first: &ScExecInstr,
	second: &ScExecInstr,
) {
	execute_compact_instruction_known::<FIRST>(ds, first);
	execute_compact_instruction_known::<SECOND>(ds, second);
}

#[cfg(feature = "unchecked-superscalar")]
#[inline(always)]
fn execute_compact_instruction_known<const OPCODE: u8>(ds: &mut [u64; 8], instr: &ScExecInstr) {
	let dst_value = exec_read_register(ds, instr.dst_offset);
	let result = match OPCODE {
		0 => dst_value.wrapping_sub(exec_read_register(ds, instr.src_offset)),
		1 => dst_value ^ exec_read_register(ds, instr.src_offset),
		2 => dst_value.wrapping_add(exec_read_register(ds, instr.src_offset) << instr.immediate),
		3 => dst_value.wrapping_mul(exec_read_register(ds, instr.src_offset)),
		4 => dst_value.rotate_right(instr.immediate as u32),
		5 => dst_value.wrapping_add(instr.immediate),
		6 => dst_value ^ instr.immediate,
		7 => mulh(dst_value, exec_read_register(ds, instr.src_offset)),
		8 => smulh(dst_value, exec_read_register(ds, instr.src_offset)),
		9 => dst_value.wrapping_mul(instr.immediate),
		// Every monomorphization stored in `PAIR_HANDLERS` uses 0..=9. Keep the
		// fallback defined anyway: an accidental future table entry must trap,
		// not turn into undefined behavior inside a proving guest.
		_ => unreachable!("invalid statically dispatched superscalar opcode"),
	};
	exec_write_register(ds, instr.dst_offset, result);
}

#[cfg(feature = "unchecked-superscalar")]
#[inline(always)]
fn execute_compact_instruction_unchecked(ds: &mut [u64; 8], instr: &ScExecInstr) {
	let dst_value = exec_read_register(ds, instr.dst_offset);
	let result = match instr.opcode {
		ScExecOpcode::IsubR => {
			dst_value.wrapping_sub(exec_read_register(ds, instr.src_offset))
		}
		ScExecOpcode::IxorR => dst_value ^ exec_read_register(ds, instr.src_offset),
		ScExecOpcode::IaddRs => {
			dst_value.wrapping_add(exec_read_register(ds, instr.src_offset) << instr.immediate)
		}
		ScExecOpcode::ImulR => {
			dst_value.wrapping_mul(exec_read_register(ds, instr.src_offset))
		}
		ScExecOpcode::IrorC => dst_value.rotate_right(instr.immediate as u32),
		ScExecOpcode::IaddC => dst_value.wrapping_add(instr.immediate),
		ScExecOpcode::IxorC => dst_value ^ instr.immediate,
		ScExecOpcode::ImulhR => mulh(dst_value, exec_read_register(ds, instr.src_offset)),
		ScExecOpcode::IsmulhR => smulh(dst_value, exec_read_register(ds, instr.src_offset)),
		ScExecOpcode::ImulRcp => dst_value.wrapping_mul(instr.immediate),
	};
	exec_write_register(ds, instr.dst_offset, result);
}

// Safety invariant for the unchecked candidate: `ScExecInstr` is private and
// can only be constructed by `decode`, which rejects register indices outside
// the fixed RandomX register file before the executable vector is stored.
#[cfg(feature = "unchecked-superscalar")]
#[inline(always)]
fn exec_read_register(registers: &[u64; 8], byte_offset: u8) -> u64 {
	debug_assert_eq!(byte_offset as usize % std::mem::size_of::<u64>(), 0);
	debug_assert!(
		byte_offset as usize
			<= std::mem::size_of_val(registers) - std::mem::size_of::<u64>()
	);
	unsafe {
		*registers
			.as_ptr()
			.cast::<u8>()
			.add(byte_offset as usize)
			.cast::<u64>()
	}
}

#[cfg(feature = "unchecked-superscalar")]
#[inline(always)]
fn exec_write_register(registers: &mut [u64; 8], byte_offset: u8, value: u64) {
	debug_assert_eq!(byte_offset as usize % std::mem::size_of::<u64>(), 0);
	debug_assert!(
		byte_offset as usize
			<= std::mem::size_of_val(registers) - std::mem::size_of::<u64>()
	);
	unsafe {
		*registers
			.as_mut_ptr()
			.cast::<u8>()
			.add(byte_offset as usize)
			.cast::<u64>() = value;
	}
}

#[allow(clippy::unnecessary_unwrap)]
fn schedule_mop(
	commit: bool,
	mop: &ScMacroOp,
	port_busy: &mut [[ExecutionPort; 3]; CYCLE_MAP_SIZE],
	cycle_in: usize,
	dep_cycle: usize,
) -> Option<usize> {
	let mut cycle = if mop.dependent {
		usize::max(cycle_in, dep_cycle)
	} else {
		cycle_in
	};

	if mop.is_eliminated() {
		return Some(cycle);
	} else if mop.is_simple() {
		return schedule_uop(commit, mop.uop1, port_busy, cycle);
	} else {
		while cycle < CYCLE_MAP_SIZE {
			let cycle_1 = schedule_uop(false, mop.uop1, port_busy, cycle);
			let cycle_2 = schedule_uop(false, mop.uop2, port_busy, cycle);

			if cycle_1.is_some() && cycle_1 == cycle_2 {
				if commit {
					schedule_uop(true, mop.uop1, port_busy, cycle_1.unwrap());
					schedule_uop(true, mop.uop2, port_busy, cycle_2.unwrap());
				}
				return cycle_1;
			}
			cycle += 1
		}
	}
	None
}

fn schedule_uop(
	commit: bool,
	uop: ExecutionPort,
	port_busy: &mut [[ExecutionPort; 3]; CYCLE_MAP_SIZE],
	cycle_in: usize,
) -> Option<usize> {
	let mut cycle = cycle_in;
	while cycle < CYCLE_MAP_SIZE {
		if uop.is(ExecutionPort::P5) && port_busy[cycle][2] == ExecutionPort::NULL {
			if commit {
				port_busy[cycle][2] = uop;
			}
			return Some(cycle);
		}
		if uop.is(ExecutionPort::P0) && port_busy[cycle][0] == ExecutionPort::NULL {
			if commit {
				port_busy[cycle][0] = uop;
			}
			return Some(cycle);
		}
		if uop.is(ExecutionPort::P1) && port_busy[cycle][1] == ExecutionPort::NULL {
			if commit {
				port_busy[cycle][1] = uop;
			}
			return Some(cycle);
		}
		cycle += 1
	}
	None
}

#[cfg(test)]
mod generator_tests {
	use super::*;

	#[test]
	fn blake_generator_truncates_long_seed_to_sixty_bytes() {
		let long_seed: Vec<u8> = (0..=255).collect();
		let mut long = Blake2Generator::new(&long_seed, 0x89ab_cdef);
		let mut truncated = Blake2Generator::new(&long_seed[..60], 0x89ab_cdef);

		for _ in 0..256 {
			assert_eq!(long.get_u32(), truncated.get_u32());
		}
	}
}

#[cfg(all(test, feature = "precompute-reciprocal"))]
mod reciprocal_tests {
	use super::*;

	#[test]
	fn superscalar_reciprocal_is_computed_during_generation() {
		let mut generator = Blake2Generator::new(b"reciprocal test", 0);
		for _ in 0..64 {
			let instr = ScInstr::create(&IMUL_RCP, &mut generator);
			assert!(!is_zero_or_power_of_2(instr.imm32));
			assert_eq!(
				instr.reciprocal,
				randomx_reciprocal(instr.imm32 as u64)
			);
			let input = 0x0123_4567_89ab_cdef_u64;
			assert_eq!(
				input.wrapping_mul(instr.reciprocal),
				input.wrapping_mul(randomx_reciprocal(instr.imm32 as u64))
			);
		}
	}
}

#[cfg(all(test, feature = "compact-superscalar"))]
mod compact_tests {
	use super::*;

	#[test]
	fn compact_execution_matches_rich_execution() {
		assert_eq!(std::mem::size_of::<ScExecInstr>(), 16);
		let mut address_registers_seen = 0u8;
		for seed_index in 0..128_u64 {
			let mut seed = [0_u8; 32];
			for (chunk_index, chunk) in seed.chunks_exact_mut(8).enumerate() {
				chunk.copy_from_slice(
					&seed_index
						.wrapping_add(chunk_index as u64)
						.wrapping_mul(0x9e37_79b9_7f4a_7c15)
						.to_le_bytes(),
				);
			}
			let mut generator = Blake2Generator::new(&seed, seed_index as u32);
			for program_index in 0..8_u64 {
				let program = ScProgram::generate(&mut generator);
				address_registers_seen |= 1 << program.address_reg;
				for input_index in 0..64_u64 {
					let mut rich = [0_u64; 8];
					for (register, value) in rich.iter_mut().enumerate() {
						*value = seed_index.wrapping_mul(0xa076_1d64_78bd_642f)
							^ program_index.wrapping_mul(0xe703_7ed1_a0b4_28db)
							^ input_index.wrapping_mul(0x8ebc_6af0_9c88_c6e3)
							^ (register as u64).wrapping_mul(0x5899_65cc_7537_4cc3);
					}
					let mut compact = rich;
					assert_eq!(
						program.address_register(&rich),
						rich[program.address_reg as usize]
					);
					program.execute_rich(&mut rich);
					program.execute_compact(&mut compact);
					assert_eq!(compact, rich);
					#[cfg(feature = "unchecked-superscalar")]
					{
						let mut unchecked = rich;
						// `rich` now contains one execution, so reconstruct the
						// original input before comparing the unchecked path.
						for (register, value) in unchecked.iter_mut().enumerate() {
							*value = seed_index.wrapping_mul(0xa076_1d64_78bd_642f)
								^ program_index.wrapping_mul(0xe703_7ed1_a0b4_28db)
								^ input_index.wrapping_mul(0x8ebc_6af0_9c88_c6e3)
								^ (register as u64).wrapping_mul(0x5899_65cc_7537_4cc3);
						}
						program.execute_compact_unchecked(&mut unchecked);
						assert_eq!(unchecked, rich);
					}
				}
			}
		}
		assert_eq!(address_registers_seen, u8::MAX);
	}

	#[cfg(feature = "unchecked-superscalar")]
	#[test]
	fn every_paired_handler_matches_sequential_dispatch() {
		let opcodes = [
			ScExecOpcode::IsubR,
			ScExecOpcode::IxorR,
			ScExecOpcode::IaddRs,
			ScExecOpcode::ImulR,
			ScExecOpcode::IrorC,
			ScExecOpcode::IaddC,
			ScExecOpcode::IxorC,
			ScExecOpcode::ImulhR,
			ScExecOpcode::IsmulhR,
			ScExecOpcode::ImulRcp,
		];
		let instruction = |opcode, dst_offset, src_offset| ScExecInstr {
			immediate: match opcode {
				ScExecOpcode::IaddRs => 3,
				ScExecOpcode::IrorC => 29,
				ScExecOpcode::IaddC | ScExecOpcode::IxorC => 0xffff_ffff_8123_4567,
				ScExecOpcode::ImulRcp => 0xd6e8_feb8_6659_fd93,
				_ => 0,
			},
			pair_handler_offset: 0,
			dst_offset,
			src_offset,
			opcode,
		};

		for (first_index, &first_opcode) in opcodes.iter().enumerate() {
			for (second_index, &second_opcode) in opcodes.iter().enumerate() {
				let mut first = instruction(first_opcode, 3 * 8, 5 * 8);
				let second = instruction(second_opcode, 5 * 8, 3 * 8);
				first.pair_handler_offset = ((first_index * 10 + second_index)
					* std::mem::size_of::<PairHandler>())
					as u16;
				let input = [
					0,
					1,
					u64::MAX,
					0x0123_4567_89ab_cdef,
					0x8000_0000_0000_0000,
					0xfedc_ba98_7654_3210,
					0x5555_aaaa_5555_aaaa,
					0xaaaa_5555_aaaa_5555,
				];
				let mut sequential = input;
				execute_compact_instruction_unchecked(&mut sequential, &first);
				execute_compact_instruction_unchecked(&mut sequential, &second);
				let mut paired = input;
				execute_compact_pair_unchecked(&mut paired, &first, &second);
				assert_eq!(paired, sequential, "pair {first_index}/{second_index}");
			}
		}
	}
}
