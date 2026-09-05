use std::{
    error::Error,
    ffi::c_void,
    fmt,
    ptr::{self, NonNull},
};

use crate::ffi::{
    RANDOMX_FLAG_ARGON2, RANDOMX_FLAG_DEFAULT, RANDOMX_FLAG_FULL_MEM, RANDOMX_FLAG_V2,
    RandomxCache, RandomxDataset, RandomxVm, randomx_alloc_cache, randomx_alloc_dataset,
    randomx_calculate_hash, randomx_create_vm, randomx_dataset_item_count, randomx_destroy_vm,
    randomx_get_flags, randomx_init_cache, randomx_init_dataset, randomx_release_cache,
    randomx_release_dataset,
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

pub trait PowEngine {
    fn key(&self) -> [u8; 32];

    fn hash(&mut self, input: &[u8]) -> [u8; 32];
}

pub struct LightEngine {
    key: [u8; 32],
    cache: NonNull<RandomxCache>,
    vm: NonNull<RandomxVm>,
}

impl LightEngine {
    pub fn new(key: [u8; 32]) -> Result<Self, PowError> {
        // SAFETY: RandomX accepts the frozen V2 flag set without pointer arguments. A null
        // return represents allocation failure; a non-null return is owned by this constructor.
        let cache = unsafe { randomx_alloc_cache(RANDOMX_FLAG_V2) };
        let cache = NonNull::new(cache).ok_or(PowError::CacheAllocationFailed)?;

        // SAFETY: `cache` is a live allocation returned by RandomX and uniquely owned here.
        // `key.as_ptr()` addresses exactly `key.len()` initialized bytes for the duration of
        // this synchronous initialization call.
        unsafe {
            randomx_init_cache(cache.as_ptr(), key.as_ptr().cast::<c_void>(), key.len());
        }

        // SAFETY: the cache is initialized and remains live for the VM lifetime. Light mode
        // requires no dataset, so the dataset argument is null. A non-null VM is owned here.
        let vm = unsafe { randomx_create_vm(RANDOMX_FLAG_V2, cache.as_ptr(), ptr::null_mut()) };
        let Some(vm) = NonNull::new(vm) else {
            // SAFETY: VM creation failed, so no VM owns or references the cache. `cache` is the
            // live allocation still uniquely owned by this constructor and must be released once.
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
        // SAFETY: both pointers are non-null RandomX allocations uniquely owned by this engine.
        // The VM may reference the cache, so the VM is destroyed before the cache is released.
        unsafe {
            randomx_destroy_vm(self.vm.as_ptr());
            randomx_release_cache(self.cache.as_ptr());
        }
    }
}

impl PowEngine for LightEngine {
    fn key(&self) -> [u8; 32] {
        LightEngine::key(self)
    }

    fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        LightEngine::hash(self, input)
    }
}

pub struct FullEngine {
    key: [u8; 32],
    dataset: NonNull<RandomxDataset>,
    vm: NonNull<RandomxVm>,
}

impl FullEngine {
    pub fn new(key: [u8; 32]) -> Result<Self, PowError> {
        // SAFETY: this RandomX query has no pointer arguments and returns a flag bitset owned by
        // value. Oregon only masks/adds documented flags before passing it back to RandomX.
        let recommended = unsafe { randomx_get_flags() };
        let cache_flags = recommended | RANDOMX_FLAG_V2;
        // SAFETY: RandomX accepts the computed flag set without pointer arguments. A null return
        // is handled as allocation failure; a non-null cache is uniquely owned here.
        let cache = unsafe { randomx_alloc_cache(cache_flags) };
        let cache = NonNull::new(cache).ok_or(PowError::CacheAllocationFailed)?;

        // SAFETY: `cache` is live and uniquely owned. The key pointer covers exactly `key.len()`
        // initialized bytes and remains valid for this synchronous initialization call.
        unsafe {
            randomx_init_cache(cache.as_ptr(), key.as_ptr().cast::<c_void>(), key.len());
        }

        // SAFETY: dataset allocation takes only the documented default flag set. A null result is
        // handled; a non-null result is a live dataset allocation uniquely owned here.
        let dataset = unsafe { randomx_alloc_dataset(RANDOMX_FLAG_DEFAULT) };
        let Some(dataset) = NonNull::new(dataset) else {
            // SAFETY: dataset allocation failed and no VM exists. The initialized cache is still
            // uniquely owned by this constructor and is released exactly once.
            unsafe { randomx_release_cache(cache.as_ptr()) };
            return Err(PowError::DatasetAllocationFailed);
        };

        // SAFETY: `dataset` and `cache` are live allocations owned by this constructor. The item
        // count comes from the same RandomX library, so the `[0, item_count)` initialization range
        // is the library-defined complete dataset. RandomX permits releasing the cache after full
        // dataset initialization; no VM has been created yet.
        unsafe {
            let item_count = randomx_dataset_item_count();
            randomx_init_dataset(dataset.as_ptr(), cache.as_ptr(), 0, item_count);
            randomx_release_cache(cache.as_ptr());
        }

        let vm_flags =
            (recommended & !RANDOMX_FLAG_ARGON2) | RANDOMX_FLAG_V2 | RANDOMX_FLAG_FULL_MEM;
        // SAFETY: the full dataset is initialized and remains live for the VM lifetime. FULL_MEM
        // mode uses the dataset and therefore receives a null cache. A non-null VM is owned here.
        let vm = unsafe { randomx_create_vm(vm_flags, ptr::null_mut(), dataset.as_ptr()) };
        let Some(vm) = NonNull::new(vm) else {
            // SAFETY: VM creation failed, so the initialized dataset remains solely owned by this
            // constructor and must be released exactly once.
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
        // SAFETY: both pointers are non-null RandomX allocations uniquely owned by this engine.
        // The VM references the dataset, so it is destroyed before the dataset is released.
        unsafe {
            randomx_destroy_vm(self.vm.as_ptr());
            randomx_release_dataset(self.dataset.as_ptr());
        }
    }
}

impl PowEngine for FullEngine {
    fn key(&self) -> [u8; 32] {
        FullEngine::key(self)
    }

    fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        FullEngine::hash(self, input)
    }
}

fn calculate_hash(vm: NonNull<RandomxVm>, input: &[u8]) -> [u8; 32] {
    let mut output = [0u8; 32];
    // SAFETY: `vm` is a live RandomX VM supplied only by an owning engine. `input` exposes
    // `input.len()` readable bytes for the duration of the call, and `output` exposes exactly
    // 32 writable bytes, which is the RandomX hash output size.
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
