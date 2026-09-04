use oregon_storage::OregonDb;

use crate::test_support::{TestDir, standard_chain_config};
use crate::ChainState;

#[test]
fn bootstrap_persists_anchor_as_preferred_header_tip() {
    let dir = TestDir::scoped("header", "bootstrap-preferred-tip");
    let config = standard_chain_config();
    let anchor_id = config.anchor_header.block_id();

    drop(ChainState::open(dir.path(), config).unwrap());

    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.preferred_header_tip().unwrap(), Some((anchor_id, 0)));
}

#[test]
fn public_header_import_api_contract_is_declared() {
    let lib_source = include_str!("lib.rs");
    let state_source = include_str!("state.rs");

    assert!(lib_source.contains("HeaderTip"));
    assert!(lib_source.contains("HeaderImportStatus"));
    assert!(lib_source.contains("HeaderImportOutcome"));
    assert!(state_source.contains("pub fn accept_header"));
    assert!(state_source.contains("pub fn preferred_header_tip"));
}

#[test]
fn block_admission_delegates_to_one_authoritative_header_validator() {
    let source = include_str!("admission.rs");
    assert_eq!(source.matches("validate_candidate_header").count(), 1);
}
