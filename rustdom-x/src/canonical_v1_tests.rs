use std::sync::Arc;

use blake2b_simd::Params;

use crate::common::randomx_reciprocal;
use crate::hash::fill_aes_1rx4_u64;
use crate::m128::{m128d, m128i};
use crate::memory::{VmMemory, init_dataset_item};
use crate::program::{Instr, MAX_REG, Mode, Opcode, Store, decode_instruction};
use crate::superscalar::{Blake2Generator, ScProgram};

const CANONICAL_KEY: &[u8] = b"test key 000";

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

fn encode_hex(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(value.len() * 2);
    for &byte in value {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0xf) as usize] as char);
    }
    result
}

fn cache_word(memory: &VmMemory, word_index: usize) -> u64 {
    const WORDS_PER_BLOCK: usize = 1024 / 8;
    let block = &memory.seed_memory.blocks()[word_index / WORDS_PER_BLOCK];
    let offset = (word_index % WORDS_PER_BLOCK) * 8;
    u64::from_le_bytes(block.as_u8()[offset..offset + 8].try_into().unwrap())
}

#[test]
fn canonical_cache_and_dataset_initialization_vectors() {
    let memory = VmMemory::light(CANONICAL_KEY);

    assert_eq!(cache_word(&memory, 0), 0x191e_0e1d_23c0_2186);
    assert_eq!(cache_word(&memory, 1_568_413), 0xf1b6_2fe6_210b_f8b1);
    assert_eq!(cache_word(&memory, 33_554_431), 0x1f47_f056_d05c_d99b);

    let expected = [
        (0, 0x6805_88a8_5ae2_22db),
        (10_000_000, 0x7943_a1f6_186f_fb72),
        (20_000_000, 0x9035_244d_7180_95e1),
        (30_000_000, 0x145a_5091_f785_3099),
    ];
    for (item, expected_first_word) in expected {
        assert_eq!(
            init_dataset_item(&memory.seed_memory, item)[0],
            expected_first_word,
            "canonical dataset item {item}"
        );
    }
}

fn canonical_superscalar_bytes(program: &ScProgram<'_>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(program.prog.len() * 8);
    for instruction in &program.prog {
        bytes.push(instruction.info.op as u8);
        bytes.push(instruction.dst as u8);
        bytes.push(if instruction.src < 0 {
            instruction.dst as u8
        } else {
            instruction.src as u8
        });
        bytes.push(instruction.mod_v);
        bytes.extend_from_slice(&instruction.imm32.to_le_bytes());
    }
    bytes
}

#[test]
fn canonical_superscalar_generator_vectors() {
    let expected = [
        "d3a4a6623738756f77e6104469102f082eff2a3e60be7ad696285ef7dfc72a61",
        "f5e7e0bbc7e93c609003d6359208688070afb4a77165a552ff7be63b38dfbc86",
        "85ed8b11734de5b3e9836641413a8f36e99e89694f419c8cd25c3f3f16c40c5a",
        "5dd956292cf5d5704ad99e362d70098b2777b2a1730520be52f772ca48cd3bc0",
        "6f14018ca7d519e9b48d91af094c0f2d7e12e93af0228782671a8640092af9e5",
        "134be097c92e2c45a92f23208cacd89e4ce51f1009a0b900dbe83b38de11d791",
        "268f9392c20c6e31371a5131f82bd7713d3910075f2f0468baafaa1abd2f3187",
        "c668a05fd909714ed4a91e8d96d67b17e44329e88bc71e0672b529a3fc16be47",
        "99739351315840963011e4c5d8e90ad0bfed3facdcb713fe8f7138fbf01c4c94",
        "14ab53d61880471f66e80183968d97effd5492b406876060e595fcf9682f9295",
    ];
    let mut generator = Blake2Generator::new(CANONICAL_KEY, 0);
    for (index, expected_hash) in expected.into_iter().enumerate() {
        let program = ScProgram::generate(&mut generator);
        let bytes = canonical_superscalar_bytes(&program);
        let hash = Params::new().hash_length(32).hash(&bytes);
        assert_eq!(
            encode_hex(hash.as_bytes()),
            expected_hash,
            "program {index}"
        );
    }
}

#[test]
fn canonical_reciprocal_vectors() {
    let expected = [
        (3, 12_297_829_382_473_034_410),
        (13, 11_351_842_506_898_185_609),
        (33, 17_887_751_829_051_686_415),
        (65_537, 18_446_462_603_027_742_720),
        (15_000_001, 10_316_166_306_300_415_204),
        (3_845_182_035, 10_302_264_209_224_146_340),
        (0xffff_ffff, 9_223_372_039_002_259_456),
    ];
    for (divisor, reciprocal) in expected {
        assert_eq!(randomx_reciprocal(divisor), reciprocal, "divisor {divisor}");
    }
}

#[test]
fn canonical_aes_generator_1r_vector() {
    let mut state = [0u8; 64];
    let initial = decode_hex("6c19536eb2de31b6c0065f7f116e86f960d8af0c57210a6584c3237b9d064dc7");
    state[..initial.len()].copy_from_slice(&initial);
    let input = [
        m128i::from_u8(&state[0..16]),
        m128i::from_u8(&state[16..32]),
        m128i::from_u8(&state[32..48]),
        m128i::from_u8(&state[48..64]),
    ];
    let mut output = vec![0u64; 8];
    fill_aes_1rx4_u64(&input, &mut output);
    let output_bytes: Vec<u8> = output.iter().flat_map(|word| word.to_le_bytes()).collect();
    assert_eq!(
        encode_hex(&output_bytes[..32]),
        "fa89397dd6ca422513aeadba3f124b5540324c4ad4b6db434394307a17c833ab"
    );
}

const CANONICAL_IMM32: u32 = 3_234_567_890;
const REGISTER_HIGH: u8 = 192;
const REGISTER_DST: u8 = 0;
const REGISTER_SRC: u8 = 1;

fn canonical_instruction(
    upper_bound: u16,
    dst: u8,
    src: u8,
    modifier: u8,
    immediate: u32,
    pc: i32,
    register_usage: &mut [i32; MAX_REG],
) -> Instr {
    let raw = (upper_bound as u64 - 1)
        | ((dst as u64) << 8)
        | ((src as u64) << 16)
        | ((modifier as u64) << 24)
        | ((immediate as u64) << 32);
    decode_instruction(raw as i64, pc, register_usage)
}

fn assert_register(store: &Store, expected: usize) {
    assert!(matches!(store, Store::R(index) if *index == expected));
}

fn assert_float_register(store: &Store, bank: char, expected: usize) {
    assert!(matches!(
        (bank, store),
        ('f', Store::F(index)) | ('e', Store::E(index)) | ('a', Store::A(index))
            if *index == expected
    ));
}

fn assert_memory_register(store: &Store, level: u8, expected: usize) {
    assert!(matches!(
        (level, store),
        (1, Store::L1(inner)) | (2, Store::L2(inner)) | (3, Store::L3(inner))
            if matches!(inner.as_ref(), Store::R(index) if *index == expected)
    ));
}

fn assert_memory_immediate(store: &Store) {
    assert!(matches!(store, Store::L3(inner) if matches!(inner.as_ref(), Store::Imm)));
}

fn signed_immediate(instruction: &Instr) -> u64 {
    instruction.imm.unwrap() as i64 as u64
}

fn vector_hex(value: m128d) -> String {
    let (high, low) = value.as_u64();
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&low.to_le_bytes());
    bytes[8..].copy_from_slice(&high.to_le_bytes());
    encode_hex(&bytes)
}

fn canonical_check(checks: &mut usize, check: impl FnOnce()) {
    check();
    *checks += 1;
}

#[test]
fn canonical_instruction_decode_and_execute_vectors() {
    let mut checks = 0usize;
    let mut usage = [0; MAX_REG];
    let memory = Arc::new(VmMemory::no_memory());
    let mut vm = crate::new_vm(memory);
    let high_dst = REGISTER_HIGH | REGISTER_DST;
    let high_src = REGISTER_HIGH | REGISTER_SRC;
    let imm64 = CANONICAL_IMM32 as i32 as i64 as u64;

    let iadd_rs = canonical_instruction(
        Opcode::IADD_RS as u16,
        high_dst,
        high_src,
        u8::MAX,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(iadd_rs.op == Opcode::IADD_RS);
        assert_register(&iadd_rs.dst, 0);
        assert_register(&iadd_rs.src, 1);
        assert!(iadd_rs.mode == Mode::Shft(3));
        assert!(iadd_rs.imm.is_none());
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 0x8000_0000_0000_0000;
        vm.reg.r[1] = 0x1000_0000_0000_0000;
        iadd_rs.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 0);
    });

    let iadd_rs_imm = canonical_instruction(
        Opcode::IADD_RS as u16,
        REGISTER_HIGH | 5,
        high_src,
        8,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(iadd_rs_imm.op == Opcode::IADD_RS);
        assert_register(&iadd_rs_imm.dst, 5);
        assert_register(&iadd_rs_imm.src, 1);
        assert!(iadd_rs_imm.mode == Mode::Shft(2));
        assert_eq!(signed_immediate(&iadd_rs_imm), imm64);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[5] = 0x8000_0000_0000_0000;
        vm.reg.r[1] = 0x2000_0000_0000_0000;
        iadd_rs_imm.execute(&mut vm);
        assert_eq!(vm.reg.r[5], imm64);
    });

    let iadd_m = canonical_instruction(
        Opcode::IADD_M as u16,
        high_dst,
        high_src,
        1,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(iadd_m.op == Opcode::IADD_M);
        assert_register(&iadd_m.dst, 0);
        assert_memory_register(&iadd_m.src, 1, 1);
        assert_eq!(signed_immediate(&iadd_m), imm64);
    });

    let isub_r = canonical_instruction(
        Opcode::ISUB_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(isub_r.op == Opcode::ISUB_R);
        assert_register(&isub_r.dst, 0);
        assert_register(&isub_r.src, 1);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 1;
        vm.reg.r[1] = 0xffff_ffff;
        isub_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 0xffff_ffff_0000_0002);
    });

    let isub_r_imm = canonical_instruction(
        Opcode::ISUB_R as u16,
        high_dst,
        high_dst,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(isub_r_imm.op == Opcode::ISUB_R);
        assert_register(&isub_r_imm.dst, 0);
        assert!(isub_r_imm.src == Store::NONE);
        assert_eq!(signed_immediate(&isub_r_imm), imm64);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 0;
        isub_r_imm.execute(&mut vm);
        assert_eq!(vm.reg.r[0], (!imm64).wrapping_add(1));
    });

    let isub_m = canonical_instruction(
        Opcode::ISUB_M as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(isub_m.op == Opcode::ISUB_M);
        assert_register(&isub_m.dst, 0);
        assert_memory_register(&isub_m.src, 2, 1);
        assert_eq!(signed_immediate(&isub_m), imm64);
    });

    let imul_r = canonical_instruction(
        Opcode::IMUL_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(imul_r.op == Opcode::IMUL_R);
        assert_register(&imul_r.dst, 0);
        assert_register(&imul_r.src, 1);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 0xbc55_0e96_ba88_a72b;
        vm.reg.r[1] = 0xf539_1fa9_f18d_6273;
        imul_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 0x2872_3424_a910_8e51);
    });

    let imul_r_imm = canonical_instruction(
        Opcode::IMUL_R as u16,
        high_dst,
        high_dst,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(imul_r_imm.op == Opcode::IMUL_R);
        assert_register(&imul_r_imm.dst, 0);
        assert!(imul_r_imm.src == Store::NONE);
        assert_eq!(signed_immediate(&imul_r_imm), imm64);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 1;
        imul_r_imm.execute(&mut vm);
        assert_eq!(vm.reg.r[0], imm64);
    });

    let imul_m = canonical_instruction(
        Opcode::IMUL_M as u16,
        high_dst,
        high_dst,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(imul_m.op == Opcode::IMUL_M);
        assert_register(&imul_m.dst, 0);
        assert_memory_immediate(&imul_m.src);
        assert_eq!(imul_m.imm, Some((CANONICAL_IMM32 & 0x1f_fff8) as i32));
    });

    let imulh_r = canonical_instruction(
        Opcode::IMULH_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(imulh_r.op == Opcode::IMULH_R);
        assert_register(&imulh_r.dst, 0);
        assert_register(&imulh_r.src, 1);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 0xbc55_0e96_ba88_a72b;
        vm.reg.r[1] = 0xf539_1fa9_f18d_6273;
        imulh_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 0xb467_6d31_d2b3_4883);
    });
    let imulh_squared = canonical_instruction(
        Opcode::IMULH_R as u16,
        high_dst,
        high_dst,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(imulh_squared.op == Opcode::IMULH_R);
        assert_register(&imulh_squared.dst, 0);
        assert_register(&imulh_squared.src, 0);
    });
    let imulh_m = canonical_instruction(
        Opcode::IMULH_M as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(imulh_m.op == Opcode::IMULH_M);
        assert_register(&imulh_m.dst, 0);
        assert_memory_register(&imulh_m.src, 2, 1);
        assert_eq!(signed_immediate(&imulh_m), imm64);
    });

    let ismulh_r = canonical_instruction(
        Opcode::ISMULH_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(ismulh_r.op == Opcode::ISMULH_R);
        assert_register(&ismulh_r.dst, 0);
        assert_register(&ismulh_r.src, 1);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 0xbc55_0e96_ba88_a72b;
        vm.reg.r[1] = 0xf539_1fa9_f18d_6273;
        ismulh_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 0x02d9_3ef1_269d_3ee5);
    });
    let ismulh_squared = canonical_instruction(
        Opcode::ISMULH_R as u16,
        high_dst,
        high_dst,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(ismulh_squared.op == Opcode::ISMULH_R);
        assert_register(&ismulh_squared.dst, 0);
        assert_register(&ismulh_squared.src, 0);
    });
    let ismulh_m = canonical_instruction(
        Opcode::ISMULH_M as u16,
        high_dst,
        high_src,
        3,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(ismulh_m.op == Opcode::ISMULH_M);
        assert_register(&ismulh_m.dst, 0);
        assert_memory_register(&ismulh_m.src, 1, 1);
        assert_eq!(signed_immediate(&ismulh_m), imm64);
    });

    let imul_rcp = canonical_instruction(
        Opcode::IMUL_RCP as u16,
        high_dst,
        0,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(imul_rcp.op == Opcode::IMUL_RCP);
        assert_register(&imul_rcp.dst, 0);
        assert_eq!(
            imul_rcp.reciprocal,
            randomx_reciprocal(CANONICAL_IMM32 as u64)
        );
    });
    let imul_rcp_zero =
        canonical_instruction(Opcode::IMUL_RCP as u16, high_dst, 0, 0, 0, 0, &mut usage);
    canonical_check(&mut checks, || {
        assert!(imul_rcp_zero.op == Opcode::IMUL_RCP);
        assert_eq!(imul_rcp_zero.reciprocal, 0);
        vm.reg.r[0] = 0x0123_4567_89ab_cdef;
        imul_rcp_zero.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 0x0123_4567_89ab_cdef);
    });

    let ineg_r = canonical_instruction(
        Opcode::INEG_R as u16,
        high_dst,
        0,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(ineg_r.op == Opcode::INEG_R);
        assert_register(&ineg_r.dst, 0);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = u64::MAX;
        ineg_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 1);
    });

    let ixor_r = canonical_instruction(
        Opcode::IXOR_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(ixor_r.op == Opcode::IXOR_R);
        assert_register(&ixor_r.dst, 0);
        assert_register(&ixor_r.src, 1);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 0x8888_8888_8888_8888;
        vm.reg.r[1] = 0xaaaa_aaaa_aaaa_aaaa;
        ixor_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 0x2222_2222_2222_2222);
    });
    let ixor_r_imm = canonical_instruction(
        Opcode::IXOR_R as u16,
        high_dst,
        high_dst,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(ixor_r_imm.op == Opcode::IXOR_R);
        assert_register(&ixor_r_imm.dst, 0);
        assert!(ixor_r_imm.src == Store::NONE);
        assert_eq!(signed_immediate(&ixor_r_imm), imm64);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = u64::MAX;
        ixor_r_imm.execute(&mut vm);
        assert_eq!(vm.reg.r[0], !imm64);
    });
    let ixor_m = canonical_instruction(
        Opcode::IXOR_M as u16,
        high_dst,
        high_dst,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(ixor_m.op == Opcode::IXOR_M);
        assert_register(&ixor_m.dst, 0);
        assert_memory_immediate(&ixor_m.src);
        assert_eq!(ixor_m.imm, Some((CANONICAL_IMM32 & 0x1f_fff8) as i32));
    });

    let iror_r = canonical_instruction(
        Opcode::IROR_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(iror_r.op == Opcode::IROR_R);
        assert_register(&iror_r.dst, 0);
        assert_register(&iror_r.src, 1);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 953_360_005_391_419_562;
        vm.reg.r[1] = 4_569_451_684_712_230_561;
        iror_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 0xd835_c455_069d_81ef);
    });

    let irol_r = canonical_instruction(
        Opcode::IROL_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(irol_r.op == Opcode::IROL_R);
        assert_register(&irol_r.dst, 0);
        assert_register(&irol_r.src, 1);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 953_360_005_391_419_562;
        vm.reg.r[1] = 4_569_451_684_712_230_561;
        irol_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 6_978_065_200_552_740_799);
    });

    let iswap_r = canonical_instruction(
        Opcode::ISWAP_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(iswap_r.op == Opcode::ISWAP_R);
        assert_register(&iswap_r.dst, 0);
        assert_register(&iswap_r.src, 1);
    });
    canonical_check(&mut checks, || {
        vm.reg.r[0] = 953_360_005_391_419_562;
        vm.reg.r[1] = 4_569_451_684_712_230_561;
        iswap_r.execute(&mut vm);
        assert_eq!(vm.reg.r[0], 4_569_451_684_712_230_561);
        assert_eq!(vm.reg.r[1], 953_360_005_391_419_562);
    });

    let fswap_r = canonical_instruction(
        Opcode::FSWAP_R as u16,
        high_dst,
        0,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fswap_r.op == Opcode::FSWAP_R);
        assert_float_register(&fswap_r.dst, 'f', 0);
    });
    canonical_check(&mut checks, || {
        vm.reg.f[0] = m128d::from_u64(953_360_005_391_419_562, 4_569_451_684_712_230_561);
        fswap_r.execute(&mut vm);
        assert_eq!(vector_hex(vm.reg.f[0]), "aa886bb0df033b0da12e95e518f4693f");
    });

    let fadd_r = canonical_instruction(
        Opcode::FADD_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fadd_r.op == Opcode::FADD_R);
        assert_float_register(&fadd_r.dst, 'f', 0);
        assert_float_register(&fadd_r.src, 'a', 1);
    });
    for (mode, expected) in [
        (0, "b932e048a730cec1fea6ea633bcc2d40"),
        (1, "b932e048a730cec1fda6ea633bcc2d40"),
        (2, "b832e048a730cec1fea6ea633bcc2d40"),
        (3, "b832e048a730cec1fda6ea633bcc2d40"),
    ] {
        canonical_check(&mut checks, || {
            vm.reg.f[0] = m128d::from_u64(0x3ffd_2c97_cc4e_f015, 0xc1ce_30b3_c422_3576);
            vm.reg.a[1] = m128d::from_u64(0x402a_26a8_6a60_c8fb, 0x40b8_f684_057a_59e1);
            vm.set_rounding_mode(mode);
            fadd_r.execute(&mut vm);
            assert_eq!(vector_hex(vm.reg.f[0]), expected);
        });
    }

    let fadd_m = canonical_instruction(
        Opcode::FADD_M as u16,
        high_dst,
        high_src,
        1,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fadd_m.op == Opcode::FADD_M);
        assert_float_register(&fadd_m.dst, 'f', 0);
        assert_memory_register(&fadd_m.src, 1, 1);
        assert_eq!(signed_immediate(&fadd_m), imm64);
    });
    canonical_check(&mut checks, || {
        vm.scratchpad[0] = 0x1234_5678_90ab_cdef;
        vm.reg.f[0] = m128d::from_u64(0, 0);
        vm.reg.r[1] = 0xffff_ffff_ffff_e930;
        vm.set_rounding_mode(0);
        fadd_m.execute(&mut vm);
        assert_eq!(vector_hex(vm.reg.f[0]), "000040840cd5dbc1000000785634b241");
    });

    let fsub_r = canonical_instruction(
        Opcode::FSUB_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fsub_r.op == Opcode::FSUB_R);
        assert_float_register(&fsub_r.dst, 'f', 0);
        assert_float_register(&fsub_r.src, 'a', 1);
    });
    let fsub_m = canonical_instruction(
        Opcode::FSUB_M as u16,
        high_dst,
        high_src,
        2,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fsub_m.op == Opcode::FSUB_M);
        assert_float_register(&fsub_m.dst, 'f', 0);
        assert_memory_register(&fsub_m.src, 1, 1);
        assert_eq!(signed_immediate(&fsub_m), imm64);
    });

    let fscal_r = canonical_instruction(
        Opcode::FSCAL_R as u16,
        high_dst,
        0,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fscal_r.op == Opcode::FSCAL_R);
        assert_float_register(&fscal_r.dst, 'f', 0);
    });
    canonical_check(&mut checks, || {
        vm.reg.f[0] = m128d::from_u64(0x41db_c35c_ef24_8783, 0x40fd_fdab_b617_3d07);
        fscal_r.execute(&mut vm);
        assert_eq!(vector_hex(vm.reg.f[0]), "073d17b6abfd0dc0838724ef5cc32bc1");
    });

    let fmul_r = canonical_instruction(
        Opcode::FMUL_R as u16,
        high_dst,
        high_src,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fmul_r.op == Opcode::FMUL_R);
        assert_float_register(&fmul_r.dst, 'e', 0);
        assert_float_register(&fmul_r.src, 'a', 1);
    });
    for (mode, expected) in [
        (0, "69697aff350fd3422f1589cdecfed742"),
        (1, "69697aff350fd3422e1589cdecfed742"),
        (2, "6a697aff350fd3422f1589cdecfed742"),
    ] {
        canonical_check(&mut checks, || {
            vm.reg.e[0] = m128d::from_u64(0x41db_c35c_ef24_8783, 0x40fd_fdab_b617_3d07);
            vm.reg.a[1] = m128d::from_u64(0x40eb_a861_aa31_c7c0, 0x41c4_5612_12ae_2d50);
            vm.set_rounding_mode(mode);
            fmul_r.execute(&mut vm);
            assert_eq!(vector_hex(vm.reg.e[0]), expected);
        });
    }

    let fdiv_m = canonical_instruction(
        Opcode::FDIV_M as u16,
        high_dst,
        high_src,
        3,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fdiv_m.op == Opcode::FDIV_M);
        assert_float_register(&fdiv_m.dst, 'e', 0);
        assert_memory_register(&fdiv_m.src, 1, 1);
        assert_eq!(signed_immediate(&fdiv_m), imm64);
    });
    for (mode, expected) in [
        (0, "e7b269639484434632474a66635ba547"),
        (1, "e6b269639484434632474a66635ba547"),
        (2, "e7b269639484434633474a66635ba547"),
    ] {
        canonical_check(&mut checks, || {
            vm.scratchpad[0] = 0x8b24_60d9_d350_a1b6;
            vm.config.e_mask = [0x3a00_0000_0005_d11a, 0x3900_0000_001b_a31e];
            vm.reg.e[0] = m128d::from_u64(0x4193_7f76_fede_16ee, 0x411b_4142_96ce_93b6);
            vm.reg.r[1] = 0xffff_ffff_ffff_e930;
            vm.set_rounding_mode(mode);
            fdiv_m.execute(&mut vm);
            assert_eq!(vector_hex(vm.reg.e[0]), expected);
        });
    }

    let fsqrt_r = canonical_instruction(
        Opcode::FSQRT_R as u16,
        high_dst,
        0,
        0,
        CANONICAL_IMM32,
        0,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(fsqrt_r.op == Opcode::FSQRT_R);
        assert_float_register(&fsqrt_r.dst, 'e', 0);
    });
    for (mode, expected) in [
        (0, "e81f300b612a21408dbaa33f570ed340"),
        (1, "e81f300b612a21408cbaa33f570ed340"),
        (2, "e91f300b612a21408dbaa33f570ed340"),
    ] {
        canonical_check(&mut checks, || {
            vm.reg.e[0] = m128d::from_u64(0x41b6_b21c_11af_fea7, 0x4052_6a7e_778d_9824);
            vm.set_rounding_mode(mode);
            fsqrt_r.execute(&mut vm);
            assert_eq!(vector_hex(vm.reg.e[0]), expected);
        });
    }

    let mut branch_usage = [0; MAX_REG];
    let first_branch = canonical_instruction(
        Opcode::CBRANCH as u16,
        high_dst,
        0,
        48,
        CANONICAL_IMM32,
        100,
        &mut branch_usage,
    );
    canonical_check(&mut checks, || {
        assert!(first_branch.op == Opcode::CBRANCH);
        assert_register(&first_branch.dst, 0);
        assert_eq!(first_branch.imm, Some(CANONICAL_IMM32 as i32));
        assert!(first_branch.mode == Mode::Cond(3));
        assert_eq!(first_branch.target, Some(0));
        let shift = 3 + 8;
        let branch_imm = (imm64 | (1 << shift)) & !(1 << (shift - 1));
        assert_eq!(branch_imm, 0xffff_ffff_c0cb_9ad2);
        assert_eq!(0xffu64 << shift, 0x7f800);
    });
    let second_branch = canonical_instruction(
        Opcode::CBRANCH as u16,
        high_dst,
        0,
        48,
        CANONICAL_IMM32,
        200,
        &mut branch_usage,
    );
    canonical_check(&mut checks, || {
        assert!(second_branch.op == Opcode::CBRANCH);
        assert_register(&second_branch.dst, 0);
        assert_eq!(second_branch.target, Some(100));
    });
    canonical_check(&mut checks, || {
        vm.pc = 200;
        vm.reg.r[0] = 0;
        second_branch.execute(&mut vm);
        assert_eq!(vm.pc, 200);
    });
    canonical_check(&mut checks, || {
        vm.pc = 200;
        vm.reg.r[0] = 0xffff_ffff_fffc_6800;
        second_branch.execute(&mut vm);
        assert_eq!(vm.pc, 100);
    });

    let cfround = canonical_instruction(
        Opcode::CFROUND as u16,
        0,
        high_src,
        0,
        CANONICAL_IMM32,
        100,
        &mut usage,
    );
    canonical_check(&mut checks, || {
        assert!(cfround.op == Opcode::CFROUND);
        assert_register(&cfround.src, 1);
        assert_eq!(cfround.imm, Some(18));
    });

    for (modifier, expected_level) in [(1, 1), (0, 2), (224, 3)] {
        let istore = canonical_instruction(
            Opcode::ISTORE as u16,
            high_dst,
            high_src,
            modifier,
            CANONICAL_IMM32,
            200,
            &mut usage,
        );
        canonical_check(&mut checks, || {
            assert!(istore.op == Opcode::ISTORE);
            assert_memory_register(&istore.dst, expected_level, 0);
            assert_register(&istore.src, 1);
            assert_eq!(signed_immediate(&istore), imm64);
        });
    }

    vm.reset_rounding_mode();
    assert_eq!(
        checks, 71,
        "all canonical instruction checks must be ported"
    );
}
