//! Exact directed binary64 arithmetic for the RandomX floating-point domain.
//!
//! RandomX supplies finite inputs and excludes NaN and subnormal/underflow
//! results. Mode zero uses the target's correctly rounded nearest-even
//! soft-float helpers. Directed modes identify which neighbor contains the
//! exact result and adjust the nearest result by at most one ULP, including
//! saturation to the signed maximum finite value when overflow requires it.

use core::cmp::Ordering;

const SIGN_MASK: u64 = 1 << 63;
const ABS_MASK: u64 = SIGN_MASK - 1;
const EXP_MASK: u64 = 0x7ff0_0000_0000_0000;
const FRAC_MASK: u64 = 0x000f_ffff_ffff_ffff;
const IMPLICIT_BIT: u64 = 1 << 52;

/// RandomX `fprc` values, in their consensus-specified encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RoundingMode {
    Nearest = 0,
    Down = 1,
    Up = 2,
    TowardZero = 3,
}

impl RoundingMode {
    #[inline(always)]
    pub const fn from_fprc(value: u32) -> Self {
        match value & 3 {
            0 => Self::Nearest,
            1 => Self::Down,
            2 => Self::Up,
            _ => Self::TowardZero,
        }
    }
}

#[derive(Clone, Copy)]
struct Magnitude {
    coefficient: u128,
    scale: i32,
}

#[inline(always)]
fn magnitude(bits: u64) -> Magnitude {
    let exponent = ((bits & EXP_MASK) >> 52) as i32;
    let fraction = bits & FRAC_MASK;
    if exponent == 0 {
        Magnitude {
            coefficient: fraction as u128,
            scale: -1074,
        }
    } else {
        debug_assert!(exponent != 0x7ff, "RandomX excludes NaN and infinity");
        Magnitude {
            coefficient: (fraction | IMPLICIT_BIT) as u128,
            scale: exponent - 1023 - 52,
        }
    }
}

/// Compare two values already known to be within one rounding binade. This is
/// the hot path for exact-vs-RN checks and avoids leading-zero normalization.
#[inline(always)]
fn compare_nearby_scaled(a: Magnitude, b: Magnitude) -> Ordering {
    if a.coefficient == 0 || b.coefficient == 0 {
        return a.coefficient.cmp(&b.coefficient);
    }
    match a.scale.cmp(&b.scale) {
        Ordering::Equal => a.coefficient.cmp(&b.coefficient),
        Ordering::Greater => {
            let shift = (a.scale - b.scale) as u32;
            debug_assert!(shift < 128);
            (a.coefficient << shift).cmp(&b.coefficient)
        }
        Ordering::Less => {
            let shift = (b.scale - a.scale) as u32;
            debug_assert!(shift < 128);
            a.coefficient.cmp(&(b.coefficient << shift))
        }
    }
}

#[inline(always)]
fn multiply_magnitudes(a: Magnitude, b: Magnitude) -> Magnitude {
    Magnitude {
        coefficient: a.coefficient * b.coefficient,
        scale: a.scale + b.scale,
    }
}

#[inline(always)]
fn reverse_if_negative(ordering: Ordering, negative: bool) -> Ordering {
    if negative {
        ordering.reverse()
    } else {
        ordering
    }
}

#[inline(always)]
fn next_up(bits: u64) -> u64 {
    if bits & ABS_MASK == 0 {
        return 1;
    }
    if bits & SIGN_MASK == 0 {
        bits + 1
    } else {
        bits - 1
    }
}

#[inline(always)]
fn next_down(bits: u64) -> u64 {
    if bits & ABS_MASK == 0 {
        return SIGN_MASK | 1;
    }
    if bits & SIGN_MASK == 0 {
        bits - 1
    } else {
        bits + 1
    }
}

/// `relation` compares the exact result to `nearest`.
#[inline(always)]
fn apply_mode(
    nearest: u64,
    relation: Ordering,
    exact_negative: bool,
    mode: RoundingMode,
) -> u64 {
    match mode {
        RoundingMode::Nearest => nearest,
        RoundingMode::Down if relation == Ordering::Less => next_down(nearest),
        RoundingMode::Up if relation == Ordering::Greater => next_up(nearest),
        RoundingMode::TowardZero if exact_negative && relation == Ordering::Greater => {
            next_up(nearest)
        }
        RoundingMode::TowardZero if !exact_negative && relation == Ordering::Less => {
            next_down(nearest)
        }
        _ => nearest,
    }
}

/// Adjust a nearest-even infinity produced by finite-input overflow.
#[inline(always)]
fn directed_overflow(nearest: u64, mode: RoundingMode) -> Option<u64> {
    if nearest & ABS_MASK != EXP_MASK {
        return None;
    }
    let negative = nearest & SIGN_MASK != 0;
    // A positive finite exact result lies below +infinity; a negative finite
    // exact result lies above -infinity. `apply_mode` therefore selects either
    // infinity or its adjacent maximum-finite value for every directed mode.
    let relation = if negative {
        Ordering::Greater
    } else {
        Ordering::Less
    };
    Some(apply_mode(nearest, relation, negative, mode))
}

#[inline(always)]
fn is_infinite(value: u64) -> bool {
    value & ABS_MASK == EXP_MASK
}

#[cfg(target_arch = "riscv64")]
mod nearest {
    #[inline(always)]
    pub fn add(a: u64, b: u64) -> u64 {
        (f64::from_bits(a) + f64::from_bits(b)).to_bits()
    }

    #[inline(always)]
    pub fn sub(a: u64, b: u64) -> u64 {
        (f64::from_bits(a) - f64::from_bits(b)).to_bits()
    }

    #[inline(always)]
    pub fn mul(a: u64, b: u64) -> u64 {
        (f64::from_bits(a) * f64::from_bits(b)).to_bits()
    }

    #[inline(always)]
    pub fn div(a: u64, b: u64) -> u64 {
        (f64::from_bits(a) / f64::from_bits(b)).to_bits()
    }

    #[inline(always)]
    pub fn sqrt(a: u64) -> u64 {
        f64::from_bits(a).sqrt().to_bits()
    }
}

#[cfg(not(target_arch = "riscv64"))]
mod nearest {
    use softfloat::F64;

    #[inline(always)]
    pub fn add(a: u64, b: u64) -> u64 {
        F64::from_bits(a).add(F64::from_bits(b)).to_bits()
    }

    #[inline(always)]
    pub fn sub(a: u64, b: u64) -> u64 {
        F64::from_bits(a).sub(F64::from_bits(b)).to_bits()
    }

    #[inline(always)]
    pub fn mul(a: u64, b: u64) -> u64 {
        F64::from_bits(a).mul(F64::from_bits(b)).to_bits()
    }

    #[inline(always)]
    pub fn div(a: u64, b: u64) -> u64 {
        F64::from_bits(a).div(F64::from_bits(b)).to_bits()
    }

    #[inline(always)]
    pub fn sqrt(a: u64) -> u64 {
        F64::from_bits(a).sqrt().to_bits()
    }
}

#[inline]
fn add_inner(a: u64, b: u64, mode: RoundingMode) -> u64 {
    let nearest = nearest::add(a, b);
    if is_infinite(a) || is_infinite(b) {
        // RandomX can propagate an infinity produced by an earlier overflow.
        // Its other operand remains finite, so the result is the same infinity
        // in every rounding mode.
        debug_assert!(nearest & ABS_MASK == EXP_MASK);
        return nearest;
    }
    if let Some(overflow) = directed_overflow(nearest, mode) {
        return overflow;
    }
    let a_magnitude = magnitude(a);
    let b_magnitude = magnitude(b);
    let a_negative = a & SIGN_MASK != 0;
    let b_negative = b & SIGN_MASK != 0;

    let (relation, negative) = if a_magnitude.coefficient == 0
        || b_magnitude.coefficient == 0
    {
        // Addition by zero is exact. The nearest helper supplies the IEEE
        // sign except for opposite-sign zero under roundTowardNegative below.
        (Ordering::Equal, nearest & SIGN_MASK != 0)
    } else if a_negative == b_negative {
        let (big, small) = if a_magnitude.scale >= b_magnitude.scale {
            (a_magnitude, b_magnitude)
        } else {
            (b_magnitude, a_magnitude)
        };
        let distance = (big.scale - small.scale) as u32;
        let magnitude_relation = if distance <= 75 {
            let exact = Magnitude {
                coefficient: (big.coefficient << distance) + small.coefficient,
                scale: small.scale,
            };
            compare_nearby_scaled(exact, magnitude(nearest))
        } else {
            // The smaller operand is far below half an ULP of `big`, so RN is
            // exactly `big`; its nonzero tail still determines directed mode.
            debug_assert_eq!(magnitude(nearest).coefficient, big.coefficient);
            debug_assert_eq!(magnitude(nearest).scale, big.scale);
            Ordering::Greater
        };
        (
            reverse_if_negative(magnitude_relation, a_negative),
            a_negative,
        )
    } else {
        match (a & ABS_MASK).cmp(&(b & ABS_MASK)) {
            Ordering::Equal => (Ordering::Equal, false),
            magnitude_order => {
                let (big, small, negative) = if magnitude_order == Ordering::Greater {
                    (a_magnitude, b_magnitude, a_negative)
                } else {
                    (b_magnitude, a_magnitude, b_negative)
                };
                debug_assert!(big.scale >= small.scale);
                let distance = (big.scale - small.scale) as u32;
                let magnitude_relation = if distance <= 75 {
                    let exact = Magnitude {
                        coefficient: (big.coefficient << distance) - small.coefficient,
                        scale: small.scale,
                    };
                    compare_nearby_scaled(exact, magnitude(nearest))
                } else {
                    debug_assert_eq!(magnitude(nearest).coefficient, big.coefficient);
                    debug_assert_eq!(magnitude(nearest).scale, big.scale);
                    Ordering::Less
                };
                (
                    reverse_if_negative(magnitude_relation, negative),
                    negative,
                )
            }
        }
    };

    // IEEE-754 gives an exact cancellation the sign of roundTowardNegative;
    // otherwise opposite-sign exact zero is positive zero.
    if nearest & ABS_MASK == 0
        && relation == Ordering::Equal
        && (a ^ b) & SIGN_MASK != 0
        && mode == RoundingMode::Down
    {
        return SIGN_MASK;
    }

    apply_mode(nearest, relation, negative, mode)
}

/// Exact RandomX binary64 addition under the selected `fprc` mode.
#[inline(always)]
pub fn add(a: u64, b: u64, mode: RoundingMode) -> u64 {
    if mode == RoundingMode::Nearest {
        nearest::add(a, b)
    } else {
        add_inner(a, b, mode)
    }
}

/// Exact RandomX binary64 subtraction under the selected `fprc` mode.
#[inline(always)]
pub fn sub(a: u64, b: u64, mode: RoundingMode) -> u64 {
    if mode == RoundingMode::Nearest {
        nearest::sub(a, b)
    } else {
        add_inner(a, b ^ SIGN_MASK, mode)
    }
}

/// Exact RandomX binary64 multiplication under the selected `fprc` mode.
#[inline]
pub fn mul(a: u64, b: u64, mode: RoundingMode) -> u64 {
    if mode == RoundingMode::Nearest {
        return nearest::mul(a, b);
    }
    mul_inner(a, b, mode)
}

#[inline(always)]
fn mul_inner(a: u64, b: u64, mode: RoundingMode) -> u64 {
    debug_assert!(mode != RoundingMode::Nearest);
    let nearest = nearest::mul(a, b);
    if is_infinite(a) || is_infinite(b) {
        debug_assert!(nearest & ABS_MASK == EXP_MASK);
        return nearest;
    }
    if let Some(overflow) = directed_overflow(nearest, mode) {
        return overflow;
    }
    let negative = (a ^ b) & SIGN_MASK != 0;
    let exact = multiply_magnitudes(magnitude(a), magnitude(b));
    let relation = reverse_if_negative(compare_nearby_scaled(exact, magnitude(nearest)), negative);
    apply_mode(nearest, relation, negative, mode)
}

/// Exact RandomX binary64 division under the selected `fprc` mode.
#[inline]
pub fn div(a: u64, b: u64, mode: RoundingMode) -> u64 {
    if mode == RoundingMode::Nearest {
        return nearest::div(a, b);
    }
    div_inner(a, b, mode)
}

#[inline(always)]
fn div_inner(a: u64, b: u64, mode: RoundingMode) -> u64 {
    debug_assert!(mode != RoundingMode::Nearest);
    debug_assert!(b & ABS_MASK != 0, "RandomX excludes a zero divisor");
    let nearest = nearest::div(a, b);
    if is_infinite(a) || is_infinite(b) {
        return nearest;
    }
    if let Some(overflow) = directed_overflow(nearest, mode) {
        return overflow;
    }
    let negative = (a ^ b) & SIGN_MASK != 0;
    let rhs = multiply_magnitudes(magnitude(nearest), magnitude(b));
    let relation = reverse_if_negative(compare_nearby_scaled(magnitude(a), rhs), negative);
    apply_mode(nearest, relation, negative, mode)
}

/// Exact RandomX square root.  RandomX guarantees a positive finite operand.
#[inline]
pub fn sqrt(a: u64, mode: RoundingMode) -> u64 {
    debug_assert!(
        a & SIGN_MASK == 0 || a & ABS_MASK == 0,
        "RandomX square-root operands are nonnegative"
    );
    if mode == RoundingMode::Nearest {
        return nearest::sqrt(a);
    }
    sqrt_inner(a, mode)
}

#[inline(always)]
fn sqrt_inner(a: u64, mode: RoundingMode) -> u64 {
    debug_assert!(mode != RoundingMode::Nearest);
    let nearest = nearest::sqrt(a);
    if is_infinite(a) {
        return nearest;
    }
    let squared = multiply_magnitudes(magnitude(nearest), magnitude(nearest));
    let relation = compare_nearby_scaled(magnitude(a), squared);
    apply_mode(nearest, relation, false, mode)
}

#[inline(always)]
pub fn add2(a: [u64; 2], b: [u64; 2], mode: RoundingMode) -> [u64; 2] {
    if mode == RoundingMode::Nearest {
        [nearest::add(a[0], b[0]), nearest::add(a[1], b[1])]
    } else {
        [add_inner(a[0], b[0], mode), add_inner(a[1], b[1], mode)]
    }
}

#[inline(always)]
pub fn sub2(a: [u64; 2], b: [u64; 2], mode: RoundingMode) -> [u64; 2] {
    if mode == RoundingMode::Nearest {
        [nearest::sub(a[0], b[0]), nearest::sub(a[1], b[1])]
    } else {
        [
            add_inner(a[0], b[0] ^ SIGN_MASK, mode),
            add_inner(a[1], b[1] ^ SIGN_MASK, mode),
        ]
    }
}

#[inline(always)]
pub fn mul2(a: [u64; 2], b: [u64; 2], mode: RoundingMode) -> [u64; 2] {
    if mode == RoundingMode::Nearest {
        [nearest::mul(a[0], b[0]), nearest::mul(a[1], b[1])]
    } else {
        [mul_inner(a[0], b[0], mode), mul_inner(a[1], b[1], mode)]
    }
}

#[inline(always)]
pub fn div2(a: [u64; 2], b: [u64; 2], mode: RoundingMode) -> [u64; 2] {
    if mode == RoundingMode::Nearest {
        [nearest::div(a[0], b[0]), nearest::div(a[1], b[1])]
    } else {
        [div_inner(a[0], b[0], mode), div_inner(a[1], b[1], mode)]
    }
}

#[inline(always)]
pub fn sqrt2(a: [u64; 2], mode: RoundingMode) -> [u64; 2] {
    if mode == RoundingMode::Nearest {
        [nearest::sqrt(a[0]), nearest::sqrt(a[1])]
    } else {
        [sqrt_inner(a[0], mode), sqrt_inner(a[1], mode)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use softfloat_wrapper::{Float, RoundingMode as OracleMode, F64};

    fn oracle_mode(mode: RoundingMode) -> OracleMode {
        match mode {
            RoundingMode::Nearest => OracleMode::TiesToEven,
            RoundingMode::Down => OracleMode::TowardNegative,
            RoundingMode::Up => OracleMode::TowardPositive,
            RoundingMode::TowardZero => OracleMode::TowardZero,
        }
    }

    fn oracle2(op: char, a: u64, b: u64, mode: RoundingMode) -> u64 {
        let a = F64::from_bits(a);
        let b = F64::from_bits(b);
        let value = match op {
            '+' => a.add(b, oracle_mode(mode)),
            '-' => a.sub(b, oracle_mode(mode)),
            '*' => a.mul(b, oracle_mode(mode)),
            '/' => a.div(b, oracle_mode(mode)),
            _ => unreachable!(),
        };
        value.to_bits()
    }

    fn oracle_sqrt(a: u64, mode: RoundingMode) -> u64 {
        F64::from_bits(a).sqrt(oracle_mode(mode)).to_bits()
    }

    #[test]
    fn official_randomx_vectors() {
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
            [0x42d3_0f35_ff7a_6969, 0x42d7_feeccd89_152f],
            [0x42d3_0f35_ff7a_6969, 0x42d7_feeccd89_152e],
            [0x42d3_0f35_ff7a_696a, 0x42d7_feeccd89_152f],
            [0x42d3_0f35_ff7a_6969, 0x42d7_feeccd89_152e],
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

        for (index, mode) in modes.into_iter().enumerate() {
            assert_eq!(add2(add_a, add_b, mode), add_expected[index]);
            assert_eq!(mul2(mul_a, mul_b, mode), mul_expected[index]);
            assert_eq!(div2(div_a, div_b, mode), div_expected[index]);
            assert_eq!(sqrt2(sqrt_a, mode), sqrt_expected[index]);
        }
    }

    #[test]
    fn randomized_randomx_domain_matches_berkeley_softfloat() {
        let modes = [
            RoundingMode::Nearest,
            RoundingMode::Down,
            RoundingMode::Up,
            RoundingMode::TowardZero,
        ];
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
            state
        };

        for case in 0..20_000 {
            // Broad finite-normal values containing the RandomX A/F/E ranges.
            let exponent_a = 700 + (next() % 600);
            let exponent_b = 700 + (next() % 600);
            let a = (next() & (SIGN_MASK | FRAC_MASK)) | (exponent_a << 52);
            let b = (next() & (SIGN_MASK | FRAC_MASK)) | (exponent_b << 52);
            let positive_a = a & ABS_MASK;
            let positive_b = b & ABS_MASK;

            for mode in modes {
                let expected_add = oracle2('+', a, b, mode);
                if expected_add & EXP_MASK != 0 {
                    assert_eq!(add(a, b, mode), expected_add, "add case {case} {mode:?}");
                }

                let expected_sub = oracle2('-', a, b, mode);
                if expected_sub & EXP_MASK != 0 {
                    assert_eq!(sub(a, b, mode), expected_sub, "sub case {case} {mode:?}");
                }

                let expected_mul = oracle2('*', a, b, mode);
                if expected_mul & EXP_MASK != 0 {
                    assert_eq!(mul(a, b, mode), expected_mul, "mul case {case} {mode:?}");
                }

                let expected_div = oracle2('/', a, b, mode);
                if expected_div & EXP_MASK != 0 {
                    assert_eq!(div(a, b, mode), expected_div, "div case {case} {mode:?}");
                }

                let expected_sqrt = oracle_sqrt(positive_a, mode);
                assert_eq!(
                    sqrt(positive_a, mode),
                    expected_sqrt,
                    "sqrt case {case} {mode:?}"
                );

                // Keep the second positive value live to vary denominator and
                // to catch exact powers in subsequent generated states.
                core::hint::black_box(positive_b);
            }
        }
    }

    #[test]
    fn exact_cancellation_has_ieee_sign() {
        let one = 1.0f64.to_bits();
        let minus_one = (-1.0f64).to_bits();
        assert_eq!(add(one, minus_one, RoundingMode::Down), SIGN_MASK);
        assert_eq!(add(one, minus_one, RoundingMode::Up), 0);
        assert_eq!(sub(one, one, RoundingMode::Down), SIGN_MASK);
        assert_eq!(sub(one, one, RoundingMode::TowardZero), 0);
    }

    #[test]
    fn finite_input_overflow_matches_berkeley_softfloat() {
        let modes = [
            RoundingMode::Nearest,
            RoundingMode::Down,
            RoundingMode::Up,
            RoundingMode::TowardZero,
        ];
        let max = 0x7fef_ffff_ffff_ffff;
        let negative_max = max | SIGN_MASK;
        let two = 2.0f64.to_bits();
        let half = 0.5f64.to_bits();
        let cases = [
            ('+', max, max),
            ('+', negative_max, negative_max),
            ('-', max, negative_max),
            ('-', negative_max, max),
            ('*', max, two),
            ('*', negative_max, two),
            ('/', max, half),
            ('/', negative_max, half),
        ];

        for mode in modes {
            for (op, a, b) in cases {
                let expected = oracle2(op, a, b, mode);
                let actual = match op {
                    '+' => add(a, b, mode),
                    '-' => sub(a, b, mode),
                    '*' => mul(a, b, mode),
                    '/' => div(a, b, mode),
                    _ => unreachable!(),
                };
                assert_eq!(actual, expected, "{op} {a:016x} {b:016x} {mode:?}");
            }
        }
    }

    #[test]
    fn infinity_propagation_matches_berkeley_softfloat() {
        let modes = [
            RoundingMode::Nearest,
            RoundingMode::Down,
            RoundingMode::Up,
            RoundingMode::TowardZero,
        ];
        let infinity = EXP_MASK;
        let negative_infinity = SIGN_MASK | EXP_MASK;
        let finite = 1.5f64.to_bits();
        let negative_finite = (-1.5f64).to_bits();

        for mode in modes {
            for infinite in [infinity, negative_infinity] {
                assert_eq!(add(infinite, finite, mode), oracle2('+', infinite, finite, mode));
                assert_eq!(sub(infinite, finite, mode), oracle2('-', infinite, finite, mode));
                assert_eq!(mul(infinite, finite, mode), oracle2('*', infinite, finite, mode));
                assert_eq!(
                    mul(infinite, negative_finite, mode),
                    oracle2('*', infinite, negative_finite, mode)
                );
                assert_eq!(div(infinite, finite, mode), oracle2('/', infinite, finite, mode));
                assert_eq!(div(finite, infinite, mode), oracle2('/', finite, infinite, mode));
            }
            assert_eq!(sqrt(infinity, mode), oracle_sqrt(infinity, mode));
        }
    }

    #[test]
    fn signed_zero_operands_match_berkeley_softfloat() {
        let modes = [
            RoundingMode::Nearest,
            RoundingMode::Down,
            RoundingMode::Up,
            RoundingMode::TowardZero,
        ];
        let zeros = [0, SIGN_MASK];
        let finite = [
            0,
            SIGN_MASK,
            1.0f64.to_bits(),
            (-1.0f64).to_bits(),
            0x0010_0000_0000_0000,
            0x8010_0000_0000_0000,
        ];

        for mode in modes {
            for a in finite {
                for z in zeros {
                    assert_eq!(add(a, z, mode), oracle2('+', a, z, mode));
                    assert_eq!(add(z, a, mode), oracle2('+', z, a, mode));
                    assert_eq!(sub(a, z, mode), oracle2('-', a, z, mode));
                    assert_eq!(sub(z, a, mode), oracle2('-', z, a, mode));
                    assert_eq!(mul(a, z, mode), oracle2('*', a, z, mode));
                    if a & ABS_MASK != 0 {
                        assert_eq!(div(z, a, mode), oracle2('/', z, a, mode));
                    }
                }
            }
            for z in zeros {
                assert_eq!(sqrt(z, mode), oracle_sqrt(z, mode));
            }
        }
    }
}
