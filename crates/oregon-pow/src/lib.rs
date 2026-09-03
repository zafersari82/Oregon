mod engine;
mod ffi;
mod input;
mod key;
mod target;

pub use engine::{FullEngine, LightEngine, PowError};
pub use input::{POW_INPUT_DOMAIN, pow_input};
pub use key::{
    RANDOMX_KEY_DELAY, RANDOMX_KEY_DOMAIN, RANDOMX_KEY_EPOCH, derive_randomx_key, key_block_height,
};
pub use target::hash_meets_target;

pub const RANDOMX_UPSTREAM_COMMIT: &str = "aaafe71322df6602c21a5c72937ac284724ae561";
pub const OREGON_RANDOMX_ARGON_SALT: &str = "OREGON-RANDOMX-V1";

#[cfg(test)]
mod tests {
    use oregon_primitives::{BlockHeader, Hash256};

    use super::{
        LightEngine, OREGON_RANDOMX_ARGON_SALT, RANDOMX_UPSTREAM_COMMIT, derive_randomx_key,
        hash_meets_target, key_block_height, pow_input,
    };

    #[test]
    fn randomx_provenance_is_frozen() {
        assert_eq!(
            RANDOMX_UPSTREAM_COMMIT,
            "aaafe71322df6602c21a5c72937ac284724ae561"
        );
        assert_eq!(OREGON_RANDOMX_ARGON_SALT, "OREGON-RANDOMX-V1");
    }

    #[test]
    fn light_engine_is_deterministic_and_sensitive_to_key_and_input() {
        let key_a = [0x11; 32];
        let mut key_b = key_a;
        key_b[31] ^= 1;

        let input_a = b"oregon-randomx-light-engine-test";
        let mut input_b = input_a.to_vec();
        input_b[0] ^= 1;

        let mut engine_a = LightEngine::new(key_a).expect("light engine A");
        assert_eq!(engine_a.key(), key_a);
        let first = engine_a.hash(input_a);
        let second = engine_a.hash(input_a);
        assert_eq!(first, second);

        let changed_input = engine_a.hash(&input_b);
        assert_ne!(first, changed_input);
        drop(engine_a);

        let mut engine_b = LightEngine::new(key_b).expect("light engine B");
        assert_eq!(engine_b.key(), key_b);
        let changed_key = engine_b.hash(input_a);
        assert_ne!(first, changed_key);
    }

    #[test]
    fn randomx_key_schedule_boundaries_are_frozen() {
        assert_eq!(key_block_height(0), 0);
        assert_eq!(key_block_height(1), 0);
        assert_eq!(key_block_height(887), 0);
        assert_eq!(key_block_height(888), 864);
        assert_eq!(key_block_height(1_751), 864);
        assert_eq!(key_block_height(1_752), 1_728);
    }

    #[test]
    fn randomx_key_derivation_is_deterministic_and_block_bound() {
        let first_id = Hash256::from_bytes([0x22; 32]);
        let mut second_bytes = [0x22; 32];
        second_bytes[31] ^= 1;
        let second_id = Hash256::from_bytes(second_bytes);

        let first = derive_randomx_key(first_id);
        assert_eq!(first, derive_randomx_key(first_id));
        assert_ne!(first, derive_randomx_key(second_id));
    }

    #[test]
    fn pow_input_is_domain_plus_exact_canonical_header() {
        let header = BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x11; 32]),
            transaction_root: Hash256::from_bytes([0x22; 32]),
            timestamp: 1_800_000_000,
            difficulty_commitment: [0x33; 32],
            nonce: 7,
        };

        let input = pow_input(&header);
        let domain = b"OREGON/POW/V1\0";
        assert_eq!(header.encode().len(), 114);
        assert_eq!(input.len(), domain.len() + 114);
        assert_eq!(&input[..domain.len()], domain);
        assert_eq!(&input[domain.len()..], header.encode());

        let mut changed = header.clone();
        changed.nonce += 1;
        assert_ne!(input, pow_input(&changed));
    }

    #[test]
    fn randomx_hash_target_comparison_is_little_endian() {
        let mut target = [0u8; 32];
        target[0] = 0xff;

        let equal = target;
        let mut below = [0u8; 32];
        below[0] = 0xfe;
        let mut above = [0u8; 32];
        above[1] = 0x01;

        assert!(hash_meets_target(equal, target));
        assert!(hash_meets_target(below, target));
        assert!(!hash_meets_target(above, target));

        let mut high_byte = [0u8; 32];
        high_byte[31] = 1;
        assert!(!hash_meets_target(high_byte, target));
    }
}
