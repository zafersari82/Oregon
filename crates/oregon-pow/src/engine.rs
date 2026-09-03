use std::{
    error::Error,
    ffi::c_void,
    fmt,
    ptr::{self, NonNull},
};

use crate::ffi::{
    RANDOMX_FLAG_V2, RandomxCache, RandomxVm, randomx_alloc_cache, randomx_calculate_hash,
    randomx_create_vm, randomx_destroy_vm, randomx_init_cache, randomx_release_cache,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowError {
    CacheAllocationFailed,
    VmAllocationFailed,
}

impl fmt::Display for PowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CacheAllocationFailed => formatter.write_str("RandomX cache allocation failed"),
            Self::VmAllocationFailed => formatter.write_str("RandomX VM allocation failed"),
        }
    }
}

impl Error for PowError {}

pub struct LightEngine {
    cache: NonNull<RandomxCache>,
    vm: NonNull<RandomxVm>,
}

impl LightEngine {
    pub fn new(key: [u8; 32]) -> Result<Self, PowError> {
        let cache = unsafe { randomx_alloc_cache(RANDOMX_FLAG_V2) };
        let cache = NonNull::new(cache).ok_or(PowError::CacheAllocationFailed)?;

        unsafe {
            randomx_init_cache(
                cache.as_ptr(),
                key.as_ptr().cast::<c_void>(),
                key.len(),
            );
        }

        let vm = unsafe { randomx_create_vm(RANDOMX_FLAG_V2, cache.as_ptr(), ptr::null_mut()) };
        let Some(vm) = NonNull::new(vm) else {
            unsafe { randomx_release_cache(cache.as_ptr()) };
            return Err(PowError::VmAllocationFailed);
        };

        Ok(Self { cache, vm })
    }

    pub fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        let mut output = [0u8; 32];
        unsafe {
            randomx_calculate_hash(
                self.vm.as_ptr(),
                input.as_ptr().cast::<c_void>(),
                input.len(),
                output.as_mut_ptr().cast::<c_void>(),
            );
        }
        output
    }
}

impl Drop for LightEngine {
    fn drop(&mut self) {
        unsafe {
            randomx_destroy_vm(self.vm.as_ptr());
            randomx_release_cache(self.cache.as_ptr());
        }
    }
}
