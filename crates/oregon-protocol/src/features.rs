use std::ops::{BitAnd, BitOr, BitOrAssign};

use crate::{Hello, ProtocolError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeatureSet(u64);

impl FeatureSet {
    pub const HEADERS_SYNC: Self = Self(1 << 0);
    pub const BLOCK_RELAY: Self = Self(1 << 1);
    pub const TX_RELAY: Self = Self(1 << 2);
    pub const KNOWN: Self = Self(Self::HEADERS_SYNC.0 | Self::BLOCK_RELAY.0 | Self::TX_RELAY.0);

    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for FeatureSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for FeatureSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for FeatureSet {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Negotiated {
    pub protocol_version: u16,
    pub features: FeatureSet,
}

pub fn negotiate(local: &Hello, remote: &Hello) -> Result<Negotiated, ProtocolError> {
    validate_range(local)?;
    validate_range(remote)?;
    validate_required_subset(local)?;
    validate_required_subset(remote)?;

    let minimum = local.min_protocol_version.max(remote.min_protocol_version);
    let maximum = local.max_protocol_version.min(remote.max_protocol_version);
    if minimum > maximum {
        return Err(ProtocolError::NoCommonProtocolVersion);
    }

    let unsupported = (local.required_features.bits() & !remote.offered_features.bits())
        | (remote.required_features.bits() & !local.offered_features.bits())
        | (local.required_features.bits() & !FeatureSet::KNOWN.bits())
        | (remote.required_features.bits() & !FeatureSet::KNOWN.bits());
    if unsupported != 0 {
        return Err(ProtocolError::UnsupportedRequiredFeatures(unsupported));
    }

    Ok(Negotiated {
        protocol_version: maximum,
        features: local.offered_features & remote.offered_features & FeatureSet::KNOWN,
    })
}

fn validate_range(hello: &Hello) -> Result<(), ProtocolError> {
    if hello.min_protocol_version == 0 || hello.min_protocol_version > hello.max_protocol_version {
        return Err(ProtocolError::InvalidProtocolVersionRange {
            min: hello.min_protocol_version,
            max: hello.max_protocol_version,
        });
    }
    Ok(())
}

fn validate_required_subset(hello: &Hello) -> Result<(), ProtocolError> {
    let missing = hello.required_features.bits() & !hello.offered_features.bits();
    if missing != 0 {
        return Err(ProtocolError::RequiredFeaturesNotOffered(missing));
    }
    Ok(())
}
