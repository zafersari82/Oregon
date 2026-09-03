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
pub use pow::{PowKeyBlockSource, validate_header_pow};
pub use target::Target;
pub use time::median_time_past;
pub use work::{ChainWork, block_work};

#[cfg(test)]
mod pow_bridge_tests {
    use oregon_pow::{LightEngine, derive_randomx_key};
    use oregon_primitives::{BlockHeader, Hash256};

    use super::{ConsensusError, PowKeyBlockSource, validate_header_pow};

    struct KeyBlocks {
        height: u64,
        id: Hash256,
    }

    impl PowKeyBlockSource for KeyBlocks {
        fn validated_block_id_at_height(&self, height: u64) -> Option<Hash256> {
            (height == self.height).then_some(self.id)
        }
    }

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
    fn randomx_pow_bridge_fetches_required_key_block_from_validated_chain() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let key_blocks = KeyBlocks {
            height: 864,
            id: key_block_id,
        };

        let hash = validate_header_pow(&header([0xff; 32]), 888, &key_blocks, &mut engine)
            .expect("max target accepts every RandomX hash");
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn randomx_pow_bridge_rejects_missing_required_key_block() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let wrong_height_source = KeyBlocks {
            height: 0,
            id: key_block_id,
        };

        assert_eq!(
            validate_header_pow(
                &header([0xff; 32]),
                888,
                &wrong_height_source,
                &mut engine,
            ),
            Err(ConsensusError::PowKeyBlockUnavailable)
        );
    }

    #[test]
    fn randomx_pow_bridge_rejects_engine_bound_to_different_chain_key() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let mut wrong_key = derive_randomx_key(key_block_id);
        wrong_key[0] ^= 1;
        let mut engine = LightEngine::new(wrong_key).expect("RandomX light engine");
        let key_blocks = KeyBlocks {
            height: 864,
            id: key_block_id,
        };

        assert_eq!(
            validate_header_pow(&header([0xff; 32]), 888, &key_blocks, &mut engine),
            Err(ConsensusError::PowEngineKeyMismatch)
        );
    }

    #[test]
    fn randomx_pow_bridge_rejects_insufficient_work() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let key_blocks = KeyBlocks {
            height: 864,
            id: key_block_id,
        };
        let mut tiny_target = [0u8; 32];
        tiny_target[0] = 1;

        assert_eq!(
            validate_header_pow(&header(tiny_target), 888, &key_blocks, &mut engine),
            Err(ConsensusError::InsufficientProofOfWork)
        );
    }
}
