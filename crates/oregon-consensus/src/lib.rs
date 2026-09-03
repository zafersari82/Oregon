pub mod asert;
pub mod block;
pub mod coinbase;
pub mod emission;
pub mod error;
pub mod header;
pub mod params;
pub mod pow;
pub mod target;
pub mod time;
pub mod work;

pub use asert::required_target;
pub use block::validate_non_genesis_block_structure;
pub use coinbase::{is_coinbase_form, validate_coinbase};
pub use emission::{
    SCHEDULED_MINING_ISSUANCE_BASE_UNITS, SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS, block_subsidy,
};
pub use error::ConsensusError;
pub use header::{HeaderContext, PrePowHeaderFacts, validate_header_pre_pow};
pub use params::ConsensusParams;
pub use pow::{PowContext, validate_header_pow};
pub use target::Target;
pub use time::median_time_past;
pub use work::{ChainWork, block_work};

#[cfg(test)]
mod pow_bridge_tests {
    use oregon_pow::{LightEngine, derive_randomx_key};
    use oregon_primitives::{BlockHeader, Hash256};

    use super::{ConsensusError, PowContext, validate_header_pow};

    fn header(target: [u8; 32]) -> BlockHeader {
        BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x11; 32]),
            transaction_root: Hash256::from_bytes([0x22; 32]),
            timestamp: 1_800_000_000,
            difficulty_commitment: target,
            nonce: 7,
        }
    }

    #[test]
    fn randomx_pow_bridge_accepts_matching_schedule_key_and_max_target() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let context = PowContext {
            candidate_height: 888,
            key_block_height: 864,
            key_block_id,
        };

        let hash = validate_header_pow(&header([0xff; 32]), context, &mut engine)
            .expect("max target accepts every RandomX hash");
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn randomx_pow_bridge_rejects_wrong_schedule_height_before_hashing() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let context = PowContext {
            candidate_height: 888,
            key_block_height: 0,
            key_block_id,
        };

        assert_eq!(
            validate_header_pow(&header([0xff; 32]), context, &mut engine),
            Err(ConsensusError::PowKeyHeightMismatch)
        );
    }

    #[test]
    fn randomx_pow_bridge_rejects_engine_bound_to_different_key() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let mut wrong_key = derive_randomx_key(key_block_id);
        wrong_key[0] ^= 1;
        let mut engine = LightEngine::new(wrong_key).expect("RandomX light engine");
        let context = PowContext {
            candidate_height: 888,
            key_block_height: 864,
            key_block_id,
        };

        assert_eq!(
            validate_header_pow(&header([0xff; 32]), context, &mut engine),
            Err(ConsensusError::PowEngineKeyMismatch)
        );
    }

    #[test]
    fn randomx_pow_bridge_rejects_insufficient_work() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let context = PowContext {
            candidate_height: 888,
            key_block_height: 864,
            key_block_id,
        };
        let mut tiny_target = [0u8; 32];
        tiny_target[0] = 1;

        assert_eq!(
            validate_header_pow(&header(tiny_target), context, &mut engine),
            Err(ConsensusError::InsufficientProofOfWork)
        );
    }
}
