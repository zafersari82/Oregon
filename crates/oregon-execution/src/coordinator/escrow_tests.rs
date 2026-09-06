use oregon_contract_state::{
    DomainSnapshot, StateError, StateNode, StateSource, empty_hashes, encode_accounting_u64,
    total_execution_balance_key,
};
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::state_commitment::CommitmentDomainId;
use oregon_primitives::Hash256;

use crate::{ExecutionJournalV1, JournalContextV1, JournalLimitsV1};

use super::accounting::{read_balance, read_total_execution_balance, write_balance};
use super::settlement::reserve_execution_funded_escrow;
use super::types::CoordinatorError;

#[derive(Debug, Default)]
struct EmptySource;

impl StateSource for EmptySource {
    fn get_node(&self, _node_hash: &Hash256) -> Result<Option<StateNode>, StateError> {
        Ok(None)
    }

    fn get_value(&self, _value_hash: &Hash256) -> Result<Option<Vec<u8>>, StateError> {
        Ok(None)
    }
}

fn payer() -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Wasm, [0x77; 32]).unwrap()
}

fn journal(source: &EmptySource) -> ExecutionJournalV1<'_, EmptySource> {
    let domain = CommitmentDomainId::ExecutionAccounting;
    ExecutionJournalV1::new(
        source,
        JournalContextV1 {
            chain_id: 42,
            height: 9001,
            parent_block_hash: Hash256::from_bytes([0x11; 32]),
            txid: Hash256::from_bytes([0x22; 32]),
        },
        &[DomainSnapshot {
            domain,
            root: empty_hashes(domain)[0],
        }],
        JournalLimitsV1::default(),
    )
    .unwrap()
}

fn seed_balance(journal: &mut ExecutionJournalV1<'_, EmptySource>, value: u64) {
    write_balance(journal, payer(), value).unwrap();
    journal
        .put(
            CommitmentDomainId::ExecutionAccounting,
            total_execution_balance_key(),
            &encode_accounting_u64(value),
        )
        .unwrap();
}

#[test]
fn max_escrow_is_not_spendable_inside_execution_child() {
    let source = EmptySource;
    let mut journal = journal(&source);
    seed_balance(&mut journal, 100);

    reserve_execution_funded_escrow(&mut journal, payer(), 40).unwrap();
    journal.begin_frame().unwrap();

    assert_eq!(read_balance(&journal, payer()).unwrap(), 60);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);
}

#[test]
fn insufficient_execution_balance_rejects_without_partial_root_debit() {
    let source = EmptySource;
    let mut journal = journal(&source);
    seed_balance(&mut journal, 100);

    assert_eq!(
        reserve_execution_funded_escrow(&mut journal, payer(), 101),
        Err(CoordinatorError::InsufficientExecutionFunding)
    );
    assert_eq!(read_balance(&journal, payer()).unwrap(), 100);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);
}
