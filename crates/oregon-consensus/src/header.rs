use oregon_primitives::BlockHeader;

use crate::{
    ConsensusError, ConsensusParams, Target,
    asert::required_target,
    time::median_time_past,
    work::{ChainWork, block_work},
};

pub struct HeaderContext<'a> {
    pub height: u64,
    pub parent: &'a BlockHeader,
    pub genesis_timestamp: u64,
    pub mtp_window: &'a [u64],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrePowHeaderFacts {
    pub target: Target,
    pub work: ChainWork,
}

pub fn validate_header_pre_pow(
    header: &BlockHeader,
    context: &HeaderContext<'_>,
    params: &ConsensusParams,
) -> Result<PrePowHeaderFacts, ConsensusError> {
    if context.height == 0 {
        return Err(ConsensusError::InvalidHeight);
    }
    if header.previous_block != context.parent.block_id() {
        return Err(ConsensusError::PreviousBlockMismatch);
    }

    let mtp = median_time_past(context.mtp_window)?;
    if header.timestamp <= mtp {
        return Err(ConsensusError::TimestampNotAfterMtp);
    }

    let expected = required_target(
        context.height,
        context.parent.timestamp,
        context.genesis_timestamp,
        params,
    )?;
    let actual = Target::from_le_bytes(header.difficulty_commitment)?;
    actual.validate_against(params.pow_limit)?;
    if actual != expected {
        return Err(ConsensusError::UnexpectedTarget);
    }

    Ok(PrePowHeaderFacts {
        target: actual,
        work: block_work(actual),
    })
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;
    use oregon_primitives::{BlockHeader, Hash256};

    use super::*;
    use crate::{ConsensusError, ConsensusParams, Target, block_work};

    const G: u64 = 1_800_000_000;

    fn target(value: u64) -> Target {
        Target::from_biguint(&BigUint::from(value)).unwrap()
    }

    fn params() -> ConsensusParams {
        ConsensusParams::new(target(10_000_000), target(1_000_000), [0x42; 32]).unwrap()
    }

    fn parent() -> BlockHeader {
        BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x11; 32]),
            transaction_root: Hash256::from_bytes([0x22; 32]),
            timestamp: G + 300,
            difficulty_commitment: target(1_000_000).to_le_bytes(),
            nonce: 7,
        }
    }

    fn child(parent: &BlockHeader, timestamp: u64, difficulty: Target) -> BlockHeader {
        BlockHeader {
            version: 1,
            previous_block: parent.block_id(),
            transaction_root: Hash256::from_bytes([0x33; 32]),
            timestamp,
            difficulty_commitment: difficulty.to_le_bytes(),
            nonce: 8,
        }
    }

    #[test]
    fn valid_context_returns_target_and_work_without_pow() {
        let params = params();
        let parent = parent();
        let mtp = [G + 299];
        let header = child(&parent, G + 301, target(1_000_000));
        let context = HeaderContext {
            height: 2,
            parent: &parent,
            genesis_timestamp: G,
            mtp_window: &mtp,
        };

        let facts = validate_header_pre_pow(&header, &context, &params).unwrap();
        assert_eq!(facts.target, target(1_000_000));
        assert_eq!(facts.work, block_work(target(1_000_000)));
    }

    #[test]
    fn wrong_parent_is_rejected() {
        let params = params();
        let parent = parent();
        let mtp = [G + 299];
        let mut header = child(&parent, G + 301, target(1_000_000));
        header.previous_block = Hash256::from_bytes([0x99; 32]);
        let context = HeaderContext {
            height: 2,
            parent: &parent,
            genesis_timestamp: G,
            mtp_window: &mtp,
        };

        assert_eq!(
            validate_header_pre_pow(&header, &context, &params),
            Err(ConsensusError::PreviousBlockMismatch)
        );
    }

    #[test]
    fn timestamp_must_be_strictly_after_mtp() {
        let params = params();
        let parent = parent();
        let mtp = [G + 300];
        let header = child(&parent, G + 300, target(1_000_000));
        let context = HeaderContext {
            height: 2,
            parent: &parent,
            genesis_timestamp: G,
            mtp_window: &mtp,
        };

        assert_eq!(
            validate_header_pre_pow(&header, &context, &params),
            Err(ConsensusError::TimestampNotAfterMtp)
        );
    }

    #[test]
    fn zero_target_is_rejected() {
        let params = params();
        let parent = parent();
        let mtp = [G + 299];
        let mut header = child(&parent, G + 301, target(1_000_000));
        header.difficulty_commitment = [0; 32];
        let context = HeaderContext {
            height: 2,
            parent: &parent,
            genesis_timestamp: G,
            mtp_window: &mtp,
        };

        assert_eq!(
            validate_header_pre_pow(&header, &context, &params),
            Err(ConsensusError::ZeroTarget)
        );
    }

    #[test]
    fn target_above_pow_limit_is_rejected() {
        let params = params();
        let parent = parent();
        let mtp = [G + 299];
        let header = child(&parent, G + 301, target(11_000_000));
        let context = HeaderContext {
            height: 2,
            parent: &parent,
            genesis_timestamp: G,
            mtp_window: &mtp,
        };

        assert_eq!(
            validate_header_pre_pow(&header, &context, &params),
            Err(ConsensusError::TargetAbovePowLimit)
        );
    }

    #[test]
    fn wrong_expected_target_is_rejected() {
        let params = params();
        let parent = parent();
        let mtp = [G + 299];
        let header = child(&parent, G + 301, target(2_000_000));
        let context = HeaderContext {
            height: 2,
            parent: &parent,
            genesis_timestamp: G,
            mtp_window: &mtp,
        };

        assert_eq!(
            validate_header_pre_pow(&header, &context, &params),
            Err(ConsensusError::UnexpectedTarget)
        );
    }

    #[test]
    fn candidate_timestamp_only_obeys_mtp_and_does_not_drive_asert() {
        let params = params();
        let parent = parent();
        let mtp = [G + 299];
        let near = child(&parent, G + 301, target(1_000_000));
        let far = child(&parent, G + 100_000, target(1_000_000));
        let context = HeaderContext {
            height: 2,
            parent: &parent,
            genesis_timestamp: G,
            mtp_window: &mtp,
        };

        assert_eq!(
            validate_header_pre_pow(&near, &context, &params)
                .unwrap()
                .target,
            target(1_000_000)
        );
        assert_eq!(
            validate_header_pre_pow(&far, &context, &params)
                .unwrap()
                .target,
            target(1_000_000)
        );
    }
}
