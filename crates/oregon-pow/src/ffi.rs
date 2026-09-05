use std::{ffi::c_void, os::raw::c_ulong};

#[repr(C)]
pub(crate) struct RandomxCache {
    _private: [u8; 0],
}

#[repr(C)]
pub(crate) struct RandomxDataset {
    _private: [u8; 0],
}

#[repr(C)]
pub(crate) struct RandomxVm {
    _private: [u8; 0],
}

pub(crate) const RANDOMX_FLAG_DEFAULT: u32 = 0;
pub(crate) const RANDOMX_FLAG_FULL_MEM: u32 = 4;
pub(crate) const RANDOMX_FLAG_ARGON2: u32 = 96;
pub(crate) const RANDOMX_FLAG_V2: u32 = 128;

// SAFETY: these declarations mirror the vendored RandomX C API. Callers must uphold the native
// ownership and pointer contracts: allocations are checked for null, cache/dataset/VM lifetimes
// are nested correctly, input/output buffers cover the lengths passed to C, and each native
// allocation is released exactly once by the safe engine wrappers in `engine.rs`.
unsafe extern "C" {
    pub(crate) fn randomx_get_flags() -> u32;
    pub(crate) fn randomx_alloc_cache(flags: u32) -> *mut RandomxCache;
    pub(crate) fn randomx_init_cache(cache: *mut RandomxCache, key: *const c_void, key_size: usize);
    pub(crate) fn randomx_release_cache(cache: *mut RandomxCache);
    pub(crate) fn randomx_alloc_dataset(flags: u32) -> *mut RandomxDataset;
    pub(crate) fn randomx_dataset_item_count() -> c_ulong;
    pub(crate) fn randomx_init_dataset(
        dataset: *mut RandomxDataset,
        cache: *mut RandomxCache,
        start_item: c_ulong,
        item_count: c_ulong,
    );
    pub(crate) fn randomx_release_dataset(dataset: *mut RandomxDataset);
    pub(crate) fn randomx_create_vm(
        flags: u32,
        cache: *mut RandomxCache,
        dataset: *mut RandomxDataset,
    ) -> *mut RandomxVm;
    pub(crate) fn randomx_destroy_vm(machine: *mut RandomxVm);
    pub(crate) fn randomx_calculate_hash(
        machine: *mut RandomxVm,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
    );
}
