use oregon_chainstate::AcceptOutcome;
use oregon_mempool::{ChainBase, Mempool, MempoolConfig, MempoolError, ReconcileReport};
use oregon_primitives::Block;
use oregon_utxo::{SpendVerifier, UtxoState};

pub(crate) fn reconcile_after_acceptance<V: SpendVerifier>(
    mempool: &mut Mempool,
    saved_config: &MempoolConfig,
    outcome: AcceptOutcome,
    block: &Block,
    new_base: ChainBase,
    chain_utxos: &UtxoState,
    verifier: &V,
) -> bool {
    let result = match outcome {
        AcceptOutcome::StoredSideChain => return false,
        AcceptOutcome::Extended => {
            mempool.reconcile_active_block(block, new_base, chain_utxos, verifier)
        }
        AcceptOutcome::Reorganized => mempool.reconcile_reorg(new_base, chain_utxos, verifier),
    };

    recover_reconciliation_failure(mempool, saved_config, new_base, result)
}

pub(crate) fn recover_reconciliation_failure(
    mempool: &mut Mempool,
    saved_config: &MempoolConfig,
    new_base: ChainBase,
    result: Result<ReconcileReport, MempoolError>,
) -> bool {
    if result.is_ok() {
        return false;
    }

    *mempool = Mempool::new(new_base, saved_config.clone())
        .expect("saved mempool configuration was validated at node startup");
    true
}
