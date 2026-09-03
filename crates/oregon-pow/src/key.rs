pub const RANDOMX_KEY_EPOCH: u64 = 864;
pub const RANDOMX_KEY_DELAY: u64 = 24;

pub fn key_block_height(candidate_height: u64) -> u64 {
    if candidate_height < RANDOMX_KEY_EPOCH + RANDOMX_KEY_DELAY {
        0
    } else {
        ((candidate_height - RANDOMX_KEY_DELAY) / RANDOMX_KEY_EPOCH) * RANDOMX_KEY_EPOCH
    }
}
