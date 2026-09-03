use oregon_pow::{LightEngine, derive_randomx_key, pow_input};
use oregon_primitives::{BlockHeader, Hash256};

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use core::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn main() {
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
    let mut engine = LightEngine::new(key).expect("RandomX light engine");
    let hash = engine.hash(&input);

    println!("key_block_id={key_block_id}");
    println!("derived_key={}", hex(&key));
    println!("pow_input_len={}", input.len());
    println!("pow_input={}", hex(&input));
    println!("randomx_hash={}", hex(&hash));
}
