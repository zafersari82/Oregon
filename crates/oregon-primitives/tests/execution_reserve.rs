use oregon_primitives::execution_reserve::{
    EXECUTION_RESERVE_LOCKING_PROGRAM_V1, reserve_outpoint_txid, reserve_transition_id,
};
use oregon_primitives::{Hash256, domain_hash};

#[test]
fn reserve_locking_program_is_exact_and_versioned() {
    assert_eq!(
        EXECUTION_RESERVE_LOCKING_PROGRAM_V1.as_slice(),
        b"OREGON/EXEC/RESERVE/V1\0"
    );
    assert_eq!(
        EXECUTION_RESERVE_LOCKING_PROGRAM_V1,
        &[
            0x4f, 0x52, 0x45, 0x47, 0x4f, 0x4e, 0x2f, 0x45, 0x58, 0x45, 0x43, 0x2f, 0x52, 0x45,
            0x53, 0x45, 0x52, 0x56, 0x45, 0x2f, 0x56, 0x31, 0x00,
        ]
    );
}

#[test]
fn reserve_transition_id_uses_its_own_domain() {
    let transition_bytes = b"canonical transition preimage";
    assert_eq!(
        reserve_transition_id(transition_bytes),
        domain_hash(b"OREGON/RESERVE/TRANSITION/V1\0", transition_bytes)
    );
}

#[test]
fn reserve_outpoint_txid_is_derived_only_from_transition_id() {
    let transition_id = Hash256::from_bytes([0x5a; 32]);
    assert_eq!(
        reserve_outpoint_txid(transition_id),
        domain_hash(b"OREGON/RESERVE/OUTPOINT/V1\0", transition_id.as_bytes())
    );
}

#[test]
fn reserve_domains_do_not_alias() {
    let payload = Hash256::from_bytes([0x7b; 32]);
    assert_ne!(
        reserve_transition_id(payload.as_bytes()),
        reserve_outpoint_txid(payload)
    );
}
