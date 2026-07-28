use std::env;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;
use std::time::Instant;

use rustdom_x::{new_vm, VmMemory};
use rustdom_x_compact_vm::calculate_hash;

const RANDOMX_FLAG_DEFAULT: u32 = 0;
const HASH_SIZE: usize = 32;

#[repr(C)]
struct RandomxCache {
    _private: [u8; 0],
}

#[repr(C)]
struct RandomxVm {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn randomx_alloc_cache(flags: u32) -> *mut RandomxCache;
    fn randomx_init_cache(cache: *mut RandomxCache, key: *const c_void, key_size: usize);
    fn randomx_release_cache(cache: *mut RandomxCache);
    fn randomx_create_vm(
        flags: u32,
        cache: *mut RandomxCache,
        dataset: *mut c_void,
    ) -> *mut RandomxVm;
    fn randomx_destroy_vm(vm: *mut RandomxVm);
    fn randomx_calculate_hash(
        vm: *mut RandomxVm,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
    );
}

struct OfficialVm {
    vm: *mut RandomxVm,
    cache: *mut RandomxCache,
}

impl OfficialVm {
    fn new(key: &[u8]) -> Self {
        // SAFETY: the v1.2.3 C API accepts the portable zero flag, copies the
        // key during initialization, and owns both returned opaque objects.
        // Rust's empty-slice pointer is non-null even when `key.len()` is zero.
        unsafe {
            let cache = randomx_alloc_cache(RANDOMX_FLAG_DEFAULT);
            assert!(!cache.is_null(), "official RandomX cache allocation failed");
            randomx_init_cache(cache, key.as_ptr().cast(), key.len());
            let vm = randomx_create_vm(RANDOMX_FLAG_DEFAULT, cache, ptr::null_mut());
            assert!(!vm.is_null(), "official RandomX VM allocation failed");
            Self { vm, cache }
        }
    }

    fn hash(&mut self, input: &[u8]) -> [u8; HASH_SIZE] {
        let mut output = [0u8; HASH_SIZE];
        // SAFETY: `self.vm` remains alive, and both byte ranges are valid for
        // the exact lengths supplied for the duration of the synchronous call.
        unsafe {
            randomx_calculate_hash(
                self.vm,
                input.as_ptr().cast(),
                input.len(),
                output.as_mut_ptr().cast(),
            );
        }
        output
    }
}

impl Drop for OfficialVm {
    fn drop(&mut self) {
        // SAFETY: each object was returned by the matching allocation call;
        // destroying the VM first releases its borrow of the cache.
        unsafe {
            randomx_destroy_vm(self.vm);
            randomx_release_cache(self.cache);
        }
    }
}

fn pattern(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    let mut bytes = vec![0; length];
    for byte in &mut bytes {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
        *byte = (state >> 56) as u8;
    }
    bytes
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn main() {
    let requested_key = env::args().nth(1).expect(
        "usage: official_randomx <empty|one-byte|test-key|zero-32|monero|pattern-64|pattern-257>",
    );
    let monero_seed = vec![
        0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54, 0x16, 0xe5,
        0xb6, 0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74, 0x6b, 0xb7, 0x8e, 0xb4,
        0xbe, 0x8e,
    ];
    let keys = [
        ("empty", Vec::new()),
        ("one-byte", vec![0xa5]),
        ("test-key", b"test key 000".to_vec()),
        ("zero-32", vec![0; 32]),
        ("monero", monero_seed),
        ("pattern-64", pattern(64, 0x243f_6a88_85a3_08d3)),
        ("pattern-257", pattern(257, 0x1319_8a2e_0370_7344)),
    ];
    let inputs = [
        ("empty", Vec::new()),
        ("one-byte", vec![0]),
        ("text", b"RandomX differential audit".to_vec()),
        ("blob-76", pattern(76, 0xa409_3822_299f_31d0)),
        ("blob-257", pattern(257, 0x082e_fa98_ec4e_6c89)),
        ("blob-4096", pattern(4096, 0x4528_21e6_38d0_1377)),
    ];

    let (key_name, key) = keys
        .into_iter()
        .find(|(name, _)| *name == requested_key)
        .unwrap_or_else(|| panic!("unknown audit key: {requested_key}"));

    let started = Instant::now();
    let mut comparisons = 0usize;
    let memory = Arc::new(VmMemory::light(&key));
    let mut rich = new_vm(Arc::clone(&memory));
    let mut compact = new_vm(memory);
    let mut official = OfficialVm::new(&key);

    for (input_name, input) in &inputs {
        let expected = official.hash(input);
        let rich_hash = rich.calculate_hash(input);
        let compact_hash = calculate_hash(&mut compact, input);

        assert_eq!(
            rich_hash.as_bytes(),
            &expected,
            "rich mismatch for key {key_name}, input {input_name}"
        );
        assert_eq!(
            compact_hash.as_bytes(),
            &expected,
            "compact mismatch for key {key_name}, input {input_name}"
        );
        assert_eq!(
            rich.reg.to_bytes(),
            compact.reg.to_bytes(),
            "register mismatch for key {key_name}, input {input_name}"
        );
        assert_eq!(
            rich.scratchpad, compact.scratchpad,
            "scratchpad mismatch for key {key_name}, input {input_name}"
        );
        compact.reset_rounding_mode();
        comparisons += 1;

        println!("{key_name}/{input_name}: {}", hex(&expected));
    }

    println!(
        "official/rich/compact agreement: {comparisons} complete light-mode hashes for {key_name} in {:.3?}",
        started.elapsed()
    );
}
