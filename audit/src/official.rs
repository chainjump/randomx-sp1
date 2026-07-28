use std::ffi::c_void;
use std::ptr;

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

/// Portable light-mode VM from the pinned official RandomX v1.2.3 library.
pub struct OfficialVm {
    vm: *mut RandomxVm,
    cache: *mut RandomxCache,
}

impl OfficialVm {
    pub fn new(key: &[u8]) -> Self {
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

    pub fn hash(&mut self, input: &[u8]) -> [u8; HASH_SIZE] {
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
