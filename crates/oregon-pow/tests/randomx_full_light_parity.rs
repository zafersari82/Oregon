use oregon_pow::{FullEngine, LightEngine, derive_randomx_key, pow_input};
use oregon_primitives::{BlockHeader, Hash256};

#[test]
#[ignore = "allocates and initializes the full RandomX dataset"]
fn full_and_light_engines_match_for_frozen_vector() {
    let key_block_id = Hash256::from_bytes([0x44; 32]);
    let key = derive_randomx_key(key_block_id);
    let header = BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0x11; 32]),
        transaction_root: Hash256::from_bytes([0x22; 32]),
        timestamp: 1_800_000_000,
        difficulty_commitment: [0xff; 32],
        nonce: 7,
    };
    let input = pow_input(&header);

    let mut light = LightEngine::new(key).expect("RandomX light engine");
    let light_hash = light.hash(&input);

    let mut full = FullEngine::new(key).expect("RandomX full engine");
    assert_eq!(full.key(), key);
    let full_hash = full.hash(&input);

    assert_eq!(full_hash, light_hash);
}
