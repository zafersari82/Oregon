use std::path::Path;

use oregon_consensus::ChainWork;
use oregon_primitives::{Block, BlockHeader, Hash256};
use oregon_storage::OregonDb;
use oregon_utxo::{SpendVerifier, UtxoState};

use crate::header::{self, HeaderImportOutcome, HeaderTip};
use crate::{ChainConfig, ChainStateError, admission, recovery};

pub const REORG_WINDOW: u64 = 8_064;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tip {
    pub block_id: Hash256,
    pub height: u64,
    pub cumulative_work: ChainWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionHealth {
    Healthy,
    StorageFaulted,
    ReindexRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptOutcome {
    Extended,
    StoredSideChain,
    Reorganized,
}

pub struct ChainState {
    pub(crate) db: OregonDb,
    pub(crate) config: ChainConfig,
    pub(crate) tip: Tip,
    pub(crate) header_tip: HeaderTip,
    pub(crate) utxos: UtxoState,
    pub(crate) session_health: SessionHealth,
}

impl ChainState {
    pub fn open(path: impl AsRef<Path>, config: ChainConfig) -> Result<Self, ChainStateError> {
        recovery::open(path, config)
    }

    pub fn tip(&self) -> &Tip {
        &self.tip
    }

    pub fn preferred_header_tip(&self) -> &HeaderTip {
        &self.header_tip
    }

    pub fn utxos(&self) -> &UtxoState {
        &self.utxos
    }

    pub fn session_health(&self) -> SessionHealth {
        self.session_health
    }

    pub(crate) fn storage(&self) -> &OregonDb {
        &self.db
    }

    pub fn accept_header(
        &mut self,
        header: BlockHeader,
    ) -> Result<HeaderImportOutcome, ChainStateError> {
        self.ensure_mutation_allowed()?;
        let result = header::accept_header_healthy(self, header);
        if matches!(&result, Err(ChainStateError::Storage(_))) {
            self.session_health = SessionHealth::StorageFaulted;
        }
        result
    }

    pub fn accept_block<V: SpendVerifier>(
        &mut self,
        block: Block,
        verifier: &V,
    ) -> Result<AcceptOutcome, ChainStateError> {
        self.ensure_mutation_allowed()?;
        let result = admission::accept_block_healthy(self, block, verifier);
        if matches!(&result, Err(ChainStateError::Storage(_))) {
            self.session_health = SessionHealth::StorageFaulted;
        }
        result
    }

    fn ensure_mutation_allowed(&self) -> Result<(), ChainStateError> {
        match self.session_health {
            SessionHealth::Healthy => Ok(()),
            SessionHealth::StorageFaulted => Err(ChainStateError::StorageFaulted),
            SessionHealth::ReindexRequired => Err(ChainStateError::ReindexRequired),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_fail_next_durable_write(&self) {
        self.db.test_hooks().fail_next_durable_write();
    }
}

#[cfg(test)]
mod deep_reorg_tests {
    use oregon_consensus::block_work;
    use oregon_primitives::{Block, BlockHeader, Transaction};
    use oregon_storage::{BlockIndexRecord, NodeHealth, StorageBatch, ValidationStatus};
    use oregon_utxo::{UtxoEntry, UtxoError};

    use super::*;
    use crate::test_support::{TestDir, standard_chain_config};
    use crate::transition;

    struct NeverCalledVerifier;

    impl SpendVerifier for NeverCalledVerifier {
        fn verify_spend(
            &self,
            _transaction: &Transaction,
            _input_index: usize,
            _prevout: &UtxoEntry,
        ) -> Result<(), UtxoError> {
            panic!("deep-reorg preflight must not reach spend verification")
        }
    }

    fn candidate_header(config: &ChainConfig, parent: Hash256, height: u64) -> BlockHeader {
        let mut root = [0u8; 32];
        root[..8].copy_from_slice(&height.to_le_bytes());
        root[8] = 0xa5;
        BlockHeader {
            version: 1,
            previous_block: parent,
            transaction_root: Hash256::from_bytes(root),
            timestamp: config.genesis_timestamp + height * 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: 50_000 + height,
        }
    }

    #[test]
    fn depth_8065_marks_reindex_before_loading_any_rollback_data() {
        let dir = TestDir::scoped("deep-reorg", "depth-8065");
        let path = dir.path();
        let config = standard_chain_config();
        let anchor_id = config.anchor_header.block_id();
        let mut state = ChainState::open(path, config.clone()).unwrap();

        let per_block_work = block_work(config.params.initial_target);
        let mut cumulative_work = ChainWork::zero();
        let mut parent_id = anchor_id;
        let mut batch = StorageBatch::new();

        for height in 1..=REORG_WINDOW + 1 {
            let header = candidate_header(&config, parent_id, height);
            cumulative_work.add_assign(&per_block_work);
            let id = header.block_id();
            batch.put_index(BlockIndexRecord {
                header,
                parent: parent_id,
                height,
                cumulative_work: cumulative_work.clone(),
                validation: ValidationStatus::HeaderValidated,
                body_retained: false,
            });
            parent_id = id;
        }
        state.db.commit_durable(batch).unwrap();

        state.tip = Tip {
            block_id: Hash256::from_bytes([0x77; 32]),
            height: REORG_WINDOW + 1,
            cumulative_work: cumulative_work.clone(),
        };
        let before_tip = state.tip.clone();
        let before_utxos = state.utxos.clone();

        let candidate_height = REORG_WINDOW + 2;
        let candidate_header = candidate_header(&config, parent_id, candidate_height);
        let candidate_id = candidate_header.block_id();
        let mut candidate_work = cumulative_work;
        candidate_work.add_assign(&per_block_work);
        let candidate_index = BlockIndexRecord {
            header: candidate_header.clone(),
            parent: parent_id,
            height: candidate_height,
            cumulative_work: candidate_work,
            validation: ValidationStatus::HeaderValidated,
            body_retained: true,
        };
        let candidate_block = Block {
            header: candidate_header,
            transactions: Vec::new(),
        };

        assert!(matches!(
            transition::reorganize(
                &mut state,
                candidate_block,
                candidate_index,
                &NeverCalledVerifier,
            ),
            Err(ChainStateError::ReindexRequired)
        ));
        assert_eq!(state.tip, before_tip);
        assert_eq!(state.utxos, before_utxos);
        assert_eq!(state.session_health, SessionHealth::ReindexRequired);
        assert_eq!(
            state.db.health().unwrap(),
            Some(NodeHealth::ReindexRequired)
        );
        assert_eq!(state.db.active_tip().unwrap(), Some((anchor_id, 0)));
        assert!(matches!(
            state.ensure_mutation_allowed(),
            Err(ChainStateError::ReindexRequired)
        ));
        drop(state);

        assert!(matches!(
            ChainState::open(path, config),
            Err(ChainStateError::ReindexRequired)
        ));
        let _ = candidate_id;
    }
}
