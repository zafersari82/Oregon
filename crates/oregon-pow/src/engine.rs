use std::{
    error::Error,
    ffi::c_void,
    fmt,
    ptr::{self, NonNull},
};

use crate::ffi::{
    RANDOMX_FLAG_DEFAULT, RANDOMX_FLAG_FULL_MEM, RANDOMX_FLAG_V2, RandomxCache, RandomxDataset,
    RandomxVm, randomx_alloc_cache, randomx_alloc_dataset, randomx_calculate_hash,
    randomx_create_vm, randomx_dataset_item_count, randomx_destroy_vm, randomx_init_cache,
    randomx_init_dataset, randomx_release_cache, randomx_release_dataset,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowError {
    CacheAllocationFailed,
    DatasetAllocationFailed,
    VmAllocationFailed,
}

impl fmt::Display for PowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CacheAllocationFailed => formatter.write_str("RandomX cache allocation failed"),
            Self::DatasetAllocationFailed => {
                formatter.write_str("RandomX dataset allocation failed")
            }
            Self::VmAllocationFailed => formatter.write_str("RandomX VM allocation failed"),
        }
    }
}

impl Error for PowError {}

pub struct LightEngine {
    key: [u8; 32],
    cache: NonNull<RandomxCache>,
    vm: NonNull<RandomxVm>,
}

impl LightEngine {
    pub fn new(key: [u8; 32]) -> Result<Self, PowError> {
        let cache = unsafe { randomx_alloc_cache(RANDOMX_FLAG_V2) };
        let cache = NonNull::new(cache).ok_or(PowError::CacheAllocationFailed)?;

        unsafe {
            randomx_init_cache(cache.as_ptr(), key.as_ptr().cast::<c_void>(), key.len());
        }

        let vm = unsafe { randomx_create_vm(RANDOMX_FLAG_V2, cache.as_ptr(), ptr::null_mut()) };
        let Some(vm) = NonNull::new(vm) else {
            unsafe { randomx_release_cache(cache.as_ptr()) };
            return Err(PowError::VmAllocationFailed);
        };

        Ok(Self { key, cache, vm })
    }

    pub const fn key(&self) -> [u8; 32] {
        self.key
    }

    pub fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        calculate_hash(self.vm, input)
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

pub struct FullEngine {
    key: [u8; 32],
    dataset: NonNull<RandomxDataset>,
    vm: NonNull<RandomxVm>,
}

impl FullEngine {
    pub fn new(key: [u8; 32]) -> Result<Self, PowError> {
        let cache = unsafe { randomx_alloc_cache(RANDOMX_FLAG_V2) };
        let cache = NonNull::new(cache).ok_or(PowError::CacheAllocationFailed)?;

        unsafe {
            randomx_init_cache(cache.as_ptr(), key.as_ptr().cast::<c_void>(), key.len());
        }

        let dataset = unsafe { randomx_alloc_dataset(RANDOMX_FLAG_DEFAULT) };
        let Some(dataset) = NonNull::new(dataset) else {
            unsafe { randomx_release_cache(cache.as_ptr()) };
            return Err(PowError::DatasetAllocationFailed);
        };

        unsafe {
            let item_count = randomx_dataset_item_count();
            randomx_init_dataset(dataset.as_ptr(), cache.as_ptr(), 0, item_count);
            randomx_release_cache(cache.as_ptr());
        }

        let vm_flags = RANDOMX_FLAG_V2 | RANDOMX_FLAG_FULL_MEM;
        let vm = unsafe { randomx_create_vm(vm_flags, ptr::null_mut(), dataset.as_ptr()) };
        let Some(vm) = NonNull::new(vm) else {
            unsafe { randomx_release_dataset(dataset.as_ptr()) };
            return Err(PowError::VmAllocationFailed);
        };

        Ok(Self { key, dataset, vm })
    }

    pub const fn key(&self) -> [u8; 32] {
        self.key
    }

    pub fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        calculate_hash(self.vm, input)
    }
}

impl Drop for FullEngine {
    fn drop(&mut self) {
        unsafe {
            randomx_destroy_vm(self.vm.as_ptr());
            randomx_release_dataset(self.dataset.as_ptr());
        }
    }
}

fn calculate_hash(vm: NonNull<RandomxVm>, input: &[u8]) -> [u8; 32] {
    let mut output = [0u8; 32];
    unsafe {
        randomx_calculate_hash(
            vm.as_ptr(),
            input.as_ptr().cast::<c_void>(),
            input.len(),
            output.as_mut_ptr().cast::<c_void>(),
        );
    }
    output
}
