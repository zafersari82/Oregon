use std::path::Path;

use oregon_consensus::{ChainWork, Target, block_work};
use oregon_primitives::Hash256;
use oregon_storage::{BlockIndexRecord, NodeHealth, OregonDb, StorageBatch, ValidationStatus};
use oregon_utxo::UtxoState;

use crate::branch::BranchView;
use crate::header::HeaderTip;
use crate::state::{ChainState, REORG_WINDOW, SessionHealth, Tip};
use crate::{ChainConfig, ChainStateError};

pub(crate) fn open(
    path: impl AsRef<Path>,
    config: ChainConfig,
) -> Result<ChainState, ChainStateError> {
    validate_config(&config)?;
    let db = OregonDb::open(path)?;
    match db.active_tip()? {
        None => bootstrap(db, config),
        Some((tip_id, tip_height)) => reopen(db, config, tip_id, tip_height),
    }
}

fn bootstrap(db: OregonDb, config: ChainConfig) -> Result<ChainState, ChainStateError> {
    let anchor_id = config.anchor_header.block_id();

    if db.config_anchor_id()?.is_some()
        || db.config_genesis_timestamp()?.is_some()
        || db.health()?.is_some()
        || db.prune_cursor()?.is_some()
        || db.active_id_at_height(0)?.is_some()
        || db.preferred_header_tip()?.is_some()
        || db.get_index(anchor_id)?.is_some()
    {
        return Err(corrupt("partial chainstate bootstrap metadata"));
    }

    let anchor_index = BlockIndexRecord {
        header: config.anchor_header.clone(),
        parent: config.anchor_header.previous_block,
        height: 0,
        cumulative_work: ChainWork::zero(),
        validation: ValidationStatus::FullyValidated,
        body_retained: false,
    };

    let mut batch = StorageBatch::new();
    batch.put_index(anchor_index);
    batch.set_active_height(0, anchor_id);
    batch.set_tip(anchor_id, 0);
    batch.set_preferred_header_tip(anchor_id, 0);
    batch.set_config_anchor_id(anchor_id);
    batch.set_config_genesis_timestamp(config.genesis_timestamp);
    batch.set_health(NodeHealth::Healthy);
    batch.set_prune_cursor(0);
    db.commit_durable(batch)?;

    Ok(ChainState {
        db,
        config,
        tip: Tip {
            block_id: anchor_id,
            height: 0,
            cumulative_work: ChainWork::zero(),
        },
        header_tip: HeaderTip {
            block_id: anchor_id,
            height: 0,
            cumulative_work: ChainWork::zero(),
        },
        utxos: UtxoState::new(),
        session_health: SessionHealth::Healthy,
    })
}

fn reopen(
    db: OregonDb,
    config: ChainConfig,
    tip_id: Hash256,
    tip_height: u64,
) -> Result<ChainState, ChainStateError> {
    let expected_anchor_id = config.anchor_header.block_id();
    let stored_anchor_id = db
        .config_anchor_id()?
        .ok_or_else(|| corrupt("missing config anchor id"))?;
    if stored_anchor_id != expected_anchor_id {
        return Err(ChainStateError::ConfigMismatch(
            "anchor block id differs from persisted chain".to_owned(),
        ));
    }

    let stored_genesis_timestamp = db
        .config_genesis_timestamp()?
        .ok_or_else(|| corrupt("missing config genesis timestamp"))?;
    if stored_genesis_timestamp != config.genesis_timestamp {
        return Err(ChainStateError::ConfigMismatch(
            "genesis timestamp differs from persisted chain".to_owned(),
        ));
    }

    match db
        .health()?
        .ok_or_else(|| corrupt("missing node health state"))?
    {
        NodeHealth::Healthy => {}
        NodeHealth::ReindexRequired => return Err(ChainStateError::ReindexRequired),
    }

    let prune_cursor = db
        .prune_cursor()?
        .ok_or_else(|| corrupt("missing prune cursor"))?;
    let maximum_safe_prune_cursor = tip_height.saturating_sub(REORG_WINDOW);
    if prune_cursor > maximum_safe_prune_cursor {
        return Err(corrupt(format!(
            "prune cursor {prune_cursor} exceeds safe cursor {maximum_safe_prune_cursor} for tip {tip_height}"
        )));
    }

    let mut previous_id = None;
    let mut previous_work = ChainWork::zero();
    let mut final_work = None;

    for height in 0..=tip_height {
        let block_id = db
            .active_id_at_height(height)?
            .ok_or_else(|| corrupt(format!("missing active mapping at height {height}")))?;
        let record = db
            .get_index(block_id)?
            .ok_or_else(|| corrupt(format!("missing block index at height {height}")))?;

        if record.height != height {
            return Err(corrupt(format!(
                "block index height {} does not match active height {height}",
                record.height
            )));
        }
        if record.validation != ValidationStatus::FullyValidated {
            return Err(corrupt(format!(
                "active block at height {height} is not fully validated"
            )));
        }

        if height == 0 {
            if block_id != expected_anchor_id || record.header != config.anchor_header {
                return Err(ChainStateError::ConfigMismatch(
                    "height-zero anchor differs from configured anchor".to_owned(),
                ));
            }
            if record.cumulative_work != ChainWork::zero() {
                return Err(corrupt("height-zero anchor cumulative work is not zero"));
            }
            if record.body_retained {
                return Err(corrupt(
                    "height-zero anchor unexpectedly retains a block body",
                ));
            }
        } else {
            let parent_id = previous_id.ok_or_else(|| corrupt("missing prior active block"))?;
            if record.parent != parent_id {
                return Err(corrupt(format!(
                    "active parent mismatch at height {height}"
                )));
            }

            let target = Target::from_le_bytes(record.header.difficulty_commitment)
                .map_err(|error| corrupt(format!("invalid target at height {height}: {error}")))?;
            target
                .validate_against(config.params.pow_limit)
                .map_err(|error| corrupt(format!("invalid target at height {height}: {error}")))?;
            let mut expected_work = previous_work.clone();
            expected_work.add_assign(&block_work(target));
            if record.cumulative_work != expected_work {
                return Err(corrupt(format!(
                    "cumulative chainwork mismatch at height {height}"
                )));
            }

            let must_retain = height > maximum_safe_prune_cursor;
            if must_retain && !record.body_retained {
                return Err(corrupt(format!(
                    "active block at height {height} is inside retained range but marked pruned"
                )));
            }
            if record.body_retained {
                if db.get_block(block_id)?.is_none() {
                    return Err(corrupt(format!(
                        "missing retained block body at height {height}"
                    )));
                }
                if db.get_undo(block_id)?.is_none() {
                    return Err(corrupt(format!("missing retained undo at height {height}")));
                }
            }
        }

        previous_id = Some(block_id);
        previous_work = record.cumulative_work.clone();
        final_work = Some(record.cumulative_work);
    }

    if previous_id != Some(tip_id) {
        return Err(corrupt("active tip id does not match final active mapping"));
    }

    let cumulative_work = final_work.ok_or_else(|| corrupt("active chain has no anchor"))?;
    let header_tip = load_preferred_header_tip(&db, expected_anchor_id)?;
    let utxos = UtxoState::try_from_entries(db.iter_utxos()?)?;

    Ok(ChainState {
        db,
        config,
        tip: Tip {
            block_id: tip_id,
            height: tip_height,
            cumulative_work,
        },
        header_tip,
        utxos,
        session_health: SessionHealth::Healthy,
    })
}

fn load_preferred_header_tip(
    db: &OregonDb,
    expected_anchor_id: Hash256,
) -> Result<HeaderTip, ChainStateError> {
    let (block_id, height) = db
        .preferred_header_tip()?
        .ok_or_else(|| corrupt("missing preferred header tip"))?;
    let record = db
        .get_index(block_id)?
        .ok_or_else(|| corrupt("missing preferred header tip index"))?;
    if record.height != height {
        return Err(corrupt("preferred header tip height does not match index"));
    }
    if record.validation == ValidationStatus::Invalid {
        return Err(corrupt("preferred header tip is marked invalid"));
    }

    let branch = BranchView::new(db, block_id);
    let anchor_id = branch
        .ancestor_id_at_height(0)?
        .ok_or_else(|| corrupt("preferred header branch has no height-zero anchor"))?;
    if anchor_id != expected_anchor_id {
        return Err(corrupt("preferred header branch does not reach configured anchor"));
    }

    Ok(HeaderTip {
        block_id,
        height,
        cumulative_work: record.cumulative_work,
    })
}

fn validate_config(config: &ChainConfig) -> Result<(), ChainStateError> {
    let zero = Hash256::from_bytes([0u8; 32]);
    if config.anchor_header.previous_block != zero {
        return Err(ChainStateError::ConfigMismatch(
            "height-zero anchor must have a zero previous block id".to_owned(),
        ));
    }
    if config.anchor_header.timestamp != config.genesis_timestamp {
        return Err(ChainStateError::ConfigMismatch(
            "anchor timestamp must equal genesis timestamp".to_owned(),
        ));
    }

    let target =
        Target::from_le_bytes(config.anchor_header.difficulty_commitment).map_err(|error| {
            ChainStateError::ConfigMismatch(format!("invalid anchor target: {error}"))
        })?;
    target
        .validate_against(config.params.pow_limit)
        .map_err(|error| {
            ChainStateError::ConfigMismatch(format!(
                "anchor target exceeds configured limit: {error}"
            ))
        })?;
    Ok(())
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}
