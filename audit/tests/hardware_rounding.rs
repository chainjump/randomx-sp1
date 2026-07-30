use std::sync::Arc;

use randomx_softfp::{add2, div2, mul2, sqrt2, sub2, RoundingMode};
use randomx_sp1_core::m128::m128d;
use randomx_sp1_core::{new_vm, VmMemory};

const CASES_PER_MODE: usize = 20_000;
const FRAC_MASK: u64 = 0x000f_ffff_ffff_ffff;
const SIGN_MASK: u64 = 1 << 63;

fn next(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    *state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
    *state
}

fn finite_normal(state: &mut u64, min_exponent: u64, exponent_span: u64) -> u64 {
    let random = next(state);
    let exponent = min_exponent + ((random >> 52) % exponent_span);
    (random & (SIGN_MASK | FRAC_MASK)) | (exponent << 52)
}

fn lanes(value: m128d) -> [u64; 2] {
    let (high, low) = value.as_u64();
    [low, high]
}

fn packed(value: [u64; 2]) -> m128d {
    m128d::from_u64(value[1], value[0])
}

#[test]
fn hardware_and_software_rounding_agree() {
    let mut vm = new_vm(Arc::new(VmMemory::no_memory()));
    let modes = [
        RoundingMode::Nearest,
        RoundingMode::Down,
        RoundingMode::Up,
        RoundingMode::TowardZero,
    ];
    let mut state = 0x243f_6a88_85a3_08d3;

    for mode in modes {
        vm.set_rounding_mode(mode as u32);
        for case in 0..CASES_PER_MODE {
            // Keep products, quotients, and sums well inside the finite-normal
            // RandomX domain while retaining randomized signs and mantissas.
            let a = [
                finite_normal(&mut state, 800, 180),
                finite_normal(&mut state, 800, 180),
            ];
            let b = [
                finite_normal(&mut state, 800, 180),
                finite_normal(&mut state, 800, 180),
            ];
            let positive = [a[0] & !SIGN_MASK, a[1] & !SIGN_MASK];

            let hardware_add = lanes(packed(a) + packed(b));
            assert_eq!(
                hardware_add,
                add2(a, b, mode),
                "add mode {mode:?} case {case}"
            );
            let hardware_sub = lanes(packed(a) - packed(b));
            assert_eq!(
                hardware_sub,
                sub2(a, b, mode),
                "sub mode {mode:?} case {case}"
            );
            let hardware_mul = lanes(packed(a) * packed(b));
            assert_eq!(
                hardware_mul,
                mul2(a, b, mode),
                "mul mode {mode:?} case {case}"
            );
            let hardware_div = lanes(packed(a) / packed(b));
            assert_eq!(
                hardware_div,
                div2(a, b, mode),
                "div mode {mode:?} case {case}"
            );
            let hardware_sqrt = lanes(packed(positive).sqrt());
            assert_eq!(
                hardware_sqrt,
                sqrt2(positive, mode),
                "sqrt mode {mode:?} case {case}"
            );
        }

        let max = 0x7fef_ffff_ffff_ffff;
        let negative_max = max | SIGN_MASK;
        let two = 2.0f64.to_bits();
        let half = 0.5f64.to_bits();
        let overflow_pairs = [
            ([max, negative_max], [max, negative_max], '+'),
            ([max, negative_max], [negative_max, max], '-'),
            ([max, negative_max], [two, two], '*'),
            ([max, negative_max], [half, half], '/'),
        ];
        for (a, b, operation) in overflow_pairs {
            let hardware = match operation {
                '+' => lanes(packed(a) + packed(b)),
                '-' => lanes(packed(a) - packed(b)),
                '*' => lanes(packed(a) * packed(b)),
                '/' => lanes(packed(a) / packed(b)),
                _ => unreachable!(),
            };
            let software = match operation {
                '+' => add2(a, b, mode),
                '-' => sub2(a, b, mode),
                '*' => mul2(a, b, mode),
                '/' => div2(a, b, mode),
                _ => unreachable!(),
            };
            assert_eq!(hardware, software, "overflow {operation} mode {mode:?}");
        }
    }

    vm.reset_rounding_mode();
}
