use oregon_contract_state::{StateError, decode_accounting_u64};

#[test]
fn accounting_u64_round_trips_exact_width() {
    let bytes = 0x8877_6655_4433_2211u64.to_le_bytes();
    assert_eq!(
        decode_accounting_u64(&bytes).unwrap(),
        0x8877_6655_4433_2211
    );
}

#[test]
fn accounting_u64_rejects_non_eight_byte_values() {
    assert_eq!(
        decode_accounting_u64(&[0u8; 7]),
        Err(StateError::InvalidAccountingValueLength(7)),
    );
    assert_eq!(
        decode_accounting_u64(&[0u8; 9]),
        Err(StateError::InvalidAccountingValueLength(9)),
    );
}
