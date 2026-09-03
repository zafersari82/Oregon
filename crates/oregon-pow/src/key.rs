use oregon_primitives::Hash256;

pub const RANDOMX_KEY_EPOCH: u64 = 864;
pub const RANDOMX_KEY_DELAY: u64 = 24;
pub const RANDOMX_KEY_DOMAIN: &[u8] = b"OREGON/RANDOMX-KEY/V1\0";

pub fn key_block_height(candidate_height: u64) -> u64 {
    if candidate_height < RANDOMX_KEY_EPOCH + RANDOMX_KEY_DELAY {
        0
    } else {
        ((candidate_height - RANDOMX_KEY_DELAY) / RANDOMX_KEY_EPOCH) * RANDOMX_KEY_EPOCH
    }
}

pub fn derive_randomx_key(key_block_id: Hash256) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RANDOMX_KEY_DOMAIN);
    hasher.update(key_block_id.as_bytes());
    *hasher.finalize().as_bytes()
}
