mod engine;
mod ffi;

pub use engine::{LightEngine, PowError};

pub const RANDOMX_UPSTREAM_COMMIT: &str = "aaafe71322df6602c21a5c72937ac284724ae561";
pub const OREGON_RANDOMX_ARGON_SALT: &str = "OREGON-RANDOMX-V1";

#[cfg(test)]
mod tests {
    use super::{
        LightEngine, OREGON_RANDOMX_ARGON_SALT, RANDOMX_UPSTREAM_COMMIT, key_block_height,
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
        let first = engine_a.hash(input_a);
        let second = engine_a.hash(input_a);
        assert_eq!(first, second);

        let changed_input = engine_a.hash(&input_b);
        assert_ne!(first, changed_input);
        drop(engine_a);

        let mut engine_b = LightEngine::new(key_b).expect("light engine B");
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
}
