use oregon_pow::{
    LightEngine, OREGON_RANDOMX_ARGON_SALT, RANDOMX_UPSTREAM_COMMIT, derive_randomx_key, pow_input,
};
use oregon_primitives::{BlockHeader, Hash256};

const VECTOR: &str = include_str!("vectors/randomx-v1.expected");

fn field(name: &str) -> &str {
    VECTOR
        .lines()
        .find_map(|line| line.strip_prefix(name).and_then(|rest| rest.strip_prefix('=')))
        .unwrap_or_else(|| panic!("missing vector field: {name}"))
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use core::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[test]
fn frozen_randomx_vector_matches_this_architecture() {
    assert_eq!(field("upstream_commit"), RANDOMX_UPSTREAM_COMMIT);
    assert_eq!(field("argon_salt"), OREGON_RANDOMX_ARGON_SALT);

    let key_block_id = Hash256::from_bytes([0x44; 32]);
    assert_eq!(field("key_block_id"), key_block_id.to_string());

    let key = derive_randomx_key(key_block_id);
    assert_eq!(field("derived_key"), hex(&key));

    let header = BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0x11; 32]),
        transaction_root: Hash256::from_bytes([0x22; 32]),
        timestamp: 1_800_000_000,
        difficulty_commitment: [0xff; 32],
        nonce: 7,
    };
    let input = pow_input(&header);
    assert_eq!(field("pow_input_len"), input.len().to_string());
    assert_eq!(field("pow_input"), hex(&input));

    let mut engine = LightEngine::new(key).expect("RandomX light engine");
    let hash = engine.hash(&input);
    assert_eq!(field("randomx_hash"), hex(&hash));
}
