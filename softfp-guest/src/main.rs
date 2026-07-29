#![no_main]

use randomx_softfp::{add2, div2, mul2, sqrt2, sub2, RoundingMode};

sp1_zkvm::entrypoint!(main);

const ITERATIONS: u64 = 1_024;
const FRAC_MASK: u64 = 0x000f_ffff_ffff_ffff;
const SIGN_MASK: u64 = 1 << 63;
const SUCCESS_TAG: [u8; 24] = *b"softfp-all-vectors-pass!";

fn start(label: &str) {
    println!("cycle-tracker-report-start:{label}");
}

fn end(label: &str) {
    println!("cycle-tracker-report-end:{label}");
}

#[inline(always)]
fn mix(state: u64, values: [u64; 2]) -> u64 {
    state
        .rotate_left(13)
        .wrapping_add(values[0])
        .wrapping_mul(0xd134_2543_de82_ef95)
        ^ values[1].rotate_right(17)
}

fn check_vectors() {
    let modes = [
        RoundingMode::Nearest,
        RoundingMode::Down,
        RoundingMode::Up,
        RoundingMode::TowardZero,
    ];
    let add_a = [0xc1ce_30b3_c422_3576, 0x3ffd_2c97_cc4e_f015];
    let add_b = [0x40b8_f684_057a_59e1, 0x402a_26a8_6a60_c8fb];
    let add_expected = [
        [0xc1ce_30a7_48e0_32b9, 0x402d_cc3b_63ea_a6fe],
        [0xc1ce_30a7_48e0_32b9, 0x402d_cc3b_63ea_a6fd],
        [0xc1ce_30a7_48e0_32b8, 0x402d_cc3b_63ea_a6fe],
        [0xc1ce_30a7_48e0_32b8, 0x402d_cc3b_63ea_a6fd],
    ];
    let mul_a = [0x40fd_fdab_b617_3d07, 0x41db_c35c_ef24_8783];
    let mul_b = [0x41c4_5612_12ae_2d50, 0x40eb_a861_aa31_c7c0];
    let mul_expected = [
        [0x42d3_0f35_ff7a_6969, 0x42d7_feec_cd89_152f],
        [0x42d3_0f35_ff7a_6969, 0x42d7_feec_cd89_152e],
        [0x42d3_0f35_ff7a_696a, 0x42d7_feec_cd89_152f],
        [0x42d3_0f35_ff7a_6969, 0x42d7_feec_cd89_152e],
    ];
    let div_a = [0x411b_4142_96ce_93b6, 0x4193_7f76_fede_16ee];
    let div_b = [0x3ac6_57af_2505_d11a, 0x39dd_36e7_c9db_a31e];
    let div_expected = [
        [0x4643_8494_6369_b2e7, 0x47a5_5b63_664a_4732],
        [0x4643_8494_6369_b2e6, 0x47a5_5b63_664a_4732],
        [0x4643_8494_6369_b2e7, 0x47a5_5b63_664a_4733],
        [0x4643_8494_6369_b2e6, 0x47a5_5b63_664a_4732],
    ];
    let sqrt_a = [0x4052_6a7e_778d_9824, 0x41b6_b21c_11af_fea7];
    let sqrt_expected = [
        [0x4021_2a61_0b30_1fe8, 0x40d3_0e57_3fa3_ba8d],
        [0x4021_2a61_0b30_1fe8, 0x40d3_0e57_3fa3_ba8c],
        [0x4021_2a61_0b30_1fe9, 0x40d3_0e57_3fa3_ba8d],
        [0x4021_2a61_0b30_1fe8, 0x40d3_0e57_3fa3_ba8c],
    ];

    for i in 0..4 {
        let mode = modes[i];
        assert!(add2(add_a, add_b, mode) == add_expected[i]);
        let negated_b = [add_b[0] ^ SIGN_MASK, add_b[1] ^ SIGN_MASK];
        assert!(sub2(add_a, negated_b, mode) == add_expected[i]);
        assert!(mul2(mul_a, mul_b, mode) == mul_expected[i]);
        assert!(div2(div_a, div_b, mode) == div_expected[i]);
        assert!(sqrt2(sqrt_a, mode) == sqrt_expected[i]);

        let cancellation_sign = if mode == RoundingMode::Down {
            SIGN_MASK
        } else {
            0
        };
        assert!(
            add2([1.0f64.to_bits(); 2], [(-1.0f64).to_bits(); 2], mode) == [cancellation_sign; 2]
        );
        assert!(sub2([1.0f64.to_bits(); 2], [1.0f64.to_bits(); 2], mode) == [cancellation_sign; 2]);
        assert!(sqrt2([0, SIGN_MASK], mode) == [0, SIGN_MASK]);
        assert!(mul2([0, SIGN_MASK], [1.0f64.to_bits(); 2], mode) == [0, SIGN_MASK]);
        assert!(div2([0, SIGN_MASK], [1.0f64.to_bits(); 2], mode) == [0, SIGN_MASK]);
    }
}

#[inline(never)]
fn bench_add(mode: RoundingMode, mut state: u64) -> u64 {
    for i in 0..ITERATIONS {
        let a = [
            0x4200_0000_0000_0000 | (state & FRAC_MASK),
            0x41d0_0000_0000_0000 | (state.rotate_left(19) & FRAC_MASK),
        ];
        let b = [
            0x40f0_0000_0000_0000 | (state.rotate_right(7) & FRAC_MASK),
            0x4100_0000_0000_0000 | (i.wrapping_mul(state) & FRAC_MASK),
        ];
        state = mix(state, add2(a, b, mode));
    }
    state
}

#[inline(never)]
fn bench_sub(mode: RoundingMode, mut state: u64) -> u64 {
    for i in 0..ITERATIONS {
        let a = [
            0x4200_0000_0000_0000 | (state & FRAC_MASK),
            0x41d0_0000_0000_0000 | (state.rotate_left(19) & FRAC_MASK),
        ];
        let b = [
            0x40f0_0000_0000_0000 | (state.rotate_right(7) & FRAC_MASK),
            0x4100_0000_0000_0000 | (i.wrapping_mul(state) & FRAC_MASK),
        ];
        state = mix(state, sub2(a, b, mode));
    }
    state
}

#[inline(never)]
fn bench_mul(mode: RoundingMode, mut state: u64) -> u64 {
    for i in 0..ITERATIONS {
        let a = [
            0x4100_0000_0000_0000 | (state & FRAC_MASK),
            0x4120_0000_0000_0000 | (state.rotate_left(23) & FRAC_MASK),
        ];
        let b = [
            0x3f00_0000_0000_0000 | (state.rotate_right(11) & FRAC_MASK),
            0x3ee0_0000_0000_0000 | (i.wrapping_mul(state) & FRAC_MASK),
        ];
        state = mix(state, mul2(a, b, mode));
    }
    state
}

#[inline(never)]
fn bench_div(mode: RoundingMode, mut state: u64) -> u64 {
    for i in 0..ITERATIONS {
        let a = [
            0x4300_0000_0000_0000 | (state & FRAC_MASK),
            0x42d0_0000_0000_0000 | (state.rotate_left(29) & FRAC_MASK),
        ];
        let b = [
            0x3f00_0000_0000_0000 | (state.rotate_right(5) & FRAC_MASK),
            0x3f20_0000_0000_0000 | (i.wrapping_mul(state) & FRAC_MASK),
        ];
        state = mix(state, div2(a, b, mode));
    }
    state
}

#[inline(never)]
fn bench_sqrt(mode: RoundingMode, mut state: u64) -> u64 {
    for i in 0..ITERATIONS {
        let a = [
            0x4500_0000_0000_0000 | (state & FRAC_MASK),
            0x3a00_0000_0000_0000 | (i.wrapping_mul(state) & FRAC_MASK),
        ];
        state = mix(state, sqrt2(a, mode));
    }
    state
}

pub fn main() {
    check_vectors();
    let modes = [
        ("nearest", RoundingMode::Nearest),
        ("down", RoundingMode::Down),
        ("up", RoundingMode::Up),
        ("zero", RoundingMode::TowardZero),
    ];
    let mut checksum = 0x9e37_79b9_7f4a_7c15u64;

    for (name, mode) in modes {
        let label = match name {
            "nearest" => "add-nearest",
            "down" => "add-down",
            "up" => "add-up",
            _ => "add-zero",
        };
        start(label);
        checksum = bench_add(mode, checksum);
        end(label);

        let label = match name {
            "nearest" => "sub-nearest",
            "down" => "sub-down",
            "up" => "sub-up",
            _ => "sub-zero",
        };
        start(label);
        checksum = bench_sub(mode, checksum);
        end(label);

        let label = match name {
            "nearest" => "mul-nearest",
            "down" => "mul-down",
            "up" => "mul-up",
            _ => "mul-zero",
        };
        start(label);
        checksum = bench_mul(mode, checksum);
        end(label);

        let label = match name {
            "nearest" => "div-nearest",
            "down" => "div-down",
            "up" => "div-up",
            _ => "div-zero",
        };
        start(label);
        checksum = bench_div(mode, checksum);
        end(label);

        let label = match name {
            "nearest" => "sqrt-nearest",
            "down" => "sqrt-down",
            "up" => "sqrt-up",
            _ => "sqrt-zero",
        };
        start(label);
        checksum = bench_sqrt(mode, checksum);
        end(label);
    }

    let mut public = [0u8; 32];
    public[..8].copy_from_slice(&checksum.to_le_bytes());
    public[8..].copy_from_slice(&SUCCESS_TAG);
    sp1_zkvm::io::commit_slice(&public);
}
