use crate::{PeerConfig, PeerError};

#[test]
fn peer_config_rejects_invalid_sums_and_hard_limit() {
    assert_eq!(
        PeerConfig::new(64, 16, 49),
        Err(PeerError::InvalidConfig("inbound + outbound exceeds max_peers"))
    );
    assert_eq!(
        PeerConfig::new(129, 16, 48),
        Err(PeerError::InvalidConfig("max_peers exceeds hard limit"))
    );
    assert_eq!(PeerConfig::new(64, 16, 48).unwrap().max_peers, 64);
}
