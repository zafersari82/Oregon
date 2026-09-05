use oregon_primitives::{Block, DecodeLimits, PrimitiveError, Transaction};

#[test]
fn huge_declared_input_count_with_tiny_payload_is_bounded_failure() {
    let bytes = [0x01, 0x00, 0xfd, 0xff, 0xff];
    assert_eq!(
        Transaction::decode(&bytes, &DecodeLimits::default()),
        Err(PrimitiveError::UnexpectedEof)
    );
}

#[test]
fn huge_declared_transaction_count_with_tiny_payload_is_bounded_failure() {
    let mut bytes = vec![0u8; 114];
    bytes.extend_from_slice(&[0xfe, 0x00, 0x00, 0x01, 0x00]);
    assert_eq!(
        Block::decode(&bytes, &DecodeLimits::default()),
        Err(PrimitiveError::UnexpectedEof)
    );
}

#[test]
fn remote_counts_never_drive_direct_vector_capacity() {
    let transaction_source = include_str!("../src/transaction.rs");
    let block_source = include_str!("../src/block.rs");

    for forbidden in [
        "Vec::with_capacity(input_count)",
        "Vec::with_capacity(output_count)",
        "Vec::with_capacity(witness_count)",
    ] {
        assert!(
            !transaction_source.contains(forbidden),
            "remote transaction count still drives direct allocation: {forbidden}"
        );
    }

    let forbidden = "Vec::with_capacity(transaction_count)";
    assert!(
        !block_source.contains(forbidden),
        "remote block count still drives direct allocation: {forbidden}"
    );
}
