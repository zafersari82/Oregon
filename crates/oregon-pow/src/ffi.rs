use std::ffi::c_void;

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

pub(crate) const RANDOMX_FLAG_V2: u32 = 128;

unsafe extern "C" {
    pub(crate) fn randomx_alloc_cache(flags: u32) -> *mut RandomxCache;
    pub(crate) fn randomx_init_cache(cache: *mut RandomxCache, key: *const c_void, key_size: usize);
    pub(crate) fn randomx_release_cache(cache: *mut RandomxCache);
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
