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
pub use block::{
    validate_non_genesis_block_skeleton, validate_non_genesis_block_structure,
    validate_normal_transaction_skeleton,
};
pub use coinbase::{is_coinbase_form, validate_coinbase};
pub use emission::{
    SCHEDULED_MINING_ISSUANCE_BASE_UNITS, SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS, block_subsidy,
};
pub use error::{ConsensusError, NormalTransactionError};
pub use header::{HeaderContext, PrePowHeaderFacts, validate_header_pre_pow};
pub use params::{ConsensusParams, MAX_TRANSACTION_BYTES};
pub use pow::{PowKeyBlockSource, validate_header_pow};
pub use target::Target;
pub use time::median_time_past;
pub use work::{ChainWork, block_work};

#[cfg(test)]
mod pow_bridge_tests {
    use num_bigint::BigUint;
    use oregon_pow::{LightEngine, PowEngine, derive_randomx_key};
    use oregon_primitives::{BlockHeader, Hash256};

    use super::{
        ConsensusError, ConsensusParams, HeaderContext, PowKeyBlockSource, PrePowHeaderFacts,
        Target, validate_header_pow, validate_header_pre_pow,
    };

    const G: u64 = 1_800_000_000;

    struct KeyBlocks {
        height: u64,
        id: Hash256,
    }

    struct FakeEngine {
        key: [u8; 32],
        hash: [u8; 32],
        calls: usize,
    }

    impl PowEngine for FakeEngine {
        fn key(&self) -> [u8; 32] {
            self.key
        }

        fn hash(&mut self, _input: &[u8]) -> [u8; 32] {
            self.calls += 1;
            self.hash
        }
    }

    impl PowKeyBlockSource for KeyBlocks {
        fn validated_block_id_at_height(&self, height: u64) -> Option<Hash256> {
            (height == self.height).then_some(self.id)
        }
    }

    fn max_target() -> Target {
        Target::from_le_bytes([0xff; 32]).unwrap()
    }

    fn target(value: u64) -> Target {
        Target::from_biguint(&BigUint::from(value)).unwrap()
    }

    fn prevalidated_header(height: u64, difficulty: Target) -> (BlockHeader, PrePowHeaderFacts) {
        let parent_timestamp = G + 300 * height.saturating_sub(1);
        let parent = BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x10; 32]),
            transaction_root: Hash256::from_bytes([0x20; 32]),
            timestamp: parent_timestamp,
            difficulty_commitment: difficulty.to_le_bytes(),
            nonce: 6,
        };
        let header = BlockHeader {
            version: 1,
            previous_block: parent.block_id(),
            transaction_root: Hash256::from_bytes([0x22; 32]),
            timestamp: parent_timestamp + 1,
            difficulty_commitment: difficulty.to_le_bytes(),
            nonce: 7,
        };
        let mtp = [parent_timestamp];
        let context = HeaderContext {
            height,
            parent: &parent,
            genesis_timestamp: G,
            mtp_window: &mtp,
        };
        let params = ConsensusParams::new(max_target(), difficulty, [0x42; 32]).unwrap();
        let facts = validate_header_pre_pow(&header, &context, &params).unwrap();
        (header, facts)
    }

    #[test]
    fn randomx_pow_bridge_requires_prevalidated_header_facts_and_chain_key() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let key_blocks = KeyBlocks {
            height: 864,
            id: key_block_id,
        };
        let (header, facts) = prevalidated_header(888, max_target());

        let hash = validate_header_pow(&header, &facts, &key_blocks, &mut engine)
            .expect("max target accepts every RandomX hash");
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn generic_pow_engine_preserves_validation_order() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let expected_key = derive_randomx_key(key_block_id);
        let key_blocks = KeyBlocks {
            height: 0,
            id: key_block_id,
        };
        let (header, facts) = prevalidated_header(1, max_target());

        let expected_hash = [0x5a; 32];
        let mut matching = FakeEngine {
            key: expected_key,
            hash: expected_hash,
            calls: 0,
        };
        assert_eq!(
            validate_header_pow(&header, &facts, &key_blocks, &mut matching),
            Ok(expected_hash)
        );
        assert_eq!(matching.calls, 1);

        let mut wrong_key = expected_key;
        wrong_key[0] ^= 1;
        let mut mismatched = FakeEngine {
            key: wrong_key,
            hash: expected_hash,
            calls: 0,
        };
        assert_eq!(
            validate_header_pow(&header, &facts, &key_blocks, &mut mismatched),
            Err(ConsensusError::PowEngineKeyMismatch)
        );
        assert_eq!(mismatched.calls, 0);
    }

    #[test]
    fn randomx_pow_bridge_rejects_facts_from_a_different_header() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let key_blocks = KeyBlocks {
            height: 864,
            id: key_block_id,
        };
        let (mut header, facts) = prevalidated_header(888, max_target());
        header.nonce += 1;

        assert_eq!(
            validate_header_pow(&header, &facts, &key_blocks, &mut engine),
            Err(ConsensusError::PowPrevalidationMismatch)
        );
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
        let (header, facts) = prevalidated_header(888, max_target());

        assert_eq!(
            validate_header_pow(&header, &facts, &wrong_height_source, &mut engine),
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
        let (header, facts) = prevalidated_header(888, max_target());

        assert_eq!(
            validate_header_pow(&header, &facts, &key_blocks, &mut engine),
            Err(ConsensusError::PowEngineKeyMismatch)
        );
    }

    #[test]
    fn randomx_pow_bridge_rejects_insufficient_work_after_prevalidation() {
        let key_block_id = Hash256::from_bytes([0x44; 32]);
        let key = derive_randomx_key(key_block_id);
        let mut engine = LightEngine::new(key).expect("RandomX light engine");
        let key_blocks = KeyBlocks {
            height: 0,
            id: key_block_id,
        };
        let (header, facts) = prevalidated_header(1, target(1));

        assert_eq!(
            validate_header_pow(&header, &facts, &key_blocks, &mut engine),
            Err(ConsensusError::InsufficientProofOfWork)
        );
    }
}
