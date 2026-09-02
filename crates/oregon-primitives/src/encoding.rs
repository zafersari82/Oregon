#[cfg(test)]
mod tests {
    use super::*;

    fn encoded_varint(value: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_varint(value, &mut bytes);
        bytes
    }

    fn decoded_varint(bytes: &[u8]) -> Result<u64, PrimitiveError> {
        let mut decoder = Decoder::new(bytes);
        let value = decoder.read_varint()?;
        decoder.finish()?;
        Ok(value)
    }

    #[test]
    fn default_decode_limits_match_protocol_v0() {
        let limits = DecodeLimits::default();
        assert_eq!(limits.max_transaction_inputs, 65_535);
        assert_eq!(limits.max_transaction_outputs, 65_535);
        assert_eq!(limits.max_witness_items_per_input, 1_024);
        assert_eq!(limits.max_witness_item_bytes, 1_048_576);
        assert_eq!(limits.max_locking_program_bytes, 65_536);
        assert_eq!(limits.max_block_transactions, 1_000_000);
        assert_eq!(limits.max_object_bytes, 67_108_864);
    }

    #[test]
    fn canonical_varint_boundaries_are_exact() {
        assert_eq!(encoded_varint(0xfc), vec![0xfc]);
        assert_eq!(encoded_varint(0xfd), vec![0xfd, 0xfd, 0x00]);
        assert_eq!(encoded_varint(0xffff), vec![0xfd, 0xff, 0xff]);
        assert_eq!(encoded_varint(0x1_0000), vec![0xfe, 0x00, 0x00, 0x01, 0x00]);
        assert_eq!(
            encoded_varint(0xffff_ffff),
            vec![0xfe, 0xff, 0xff, 0xff, 0xff]
        );
        assert_eq!(
            encoded_varint(0x1_0000_0000),
            vec![0xff, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn canonical_varints_round_trip() {
        for value in [
            0,
            1,
            0xfc,
            0xfd,
            0xffff,
            0x1_0000,
            0xffff_ffff,
            0x1_0000_0000,
            u64::MAX,
        ] {
            let bytes = encoded_varint(value);
            assert_eq!(decoded_varint(&bytes).unwrap(), value);
        }
    }

    #[test]
    fn non_minimal_varints_are_rejected() {
        for bytes in [
            &[0xfd, 0xfc, 0x00][..],
            &[0xfe, 0xff, 0xff, 0x00, 0x00][..],
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00][..],
        ] {
            assert_eq!(decoded_varint(bytes), Err(PrimitiveError::NonCanonicalVarInt));
        }
    }

    #[test]
    fn fixed_width_reads_reject_truncation() {
        assert_eq!(
            Decoder::new(&[0u8; 1]).read_u16(),
            Err(PrimitiveError::UnexpectedEof)
        );
        assert_eq!(
            Decoder::new(&[0u8; 3]).read_u32(),
            Err(PrimitiveError::UnexpectedEof)
        );
        assert_eq!(
            Decoder::new(&[0u8; 7]).read_u64(),
            Err(PrimitiveError::UnexpectedEof)
        );
    }

    #[test]
    fn read_len_enforces_caller_limit() {
        let bytes = encoded_varint(11);
        let mut decoder = Decoder::new(&bytes);
        assert_eq!(decoder.read_len(10), Err(PrimitiveError::LengthLimitExceeded));
    }

    #[test]
    fn read_bytes_rejects_truncation() {
        let mut decoder = Decoder::new(&[1, 2, 3]);
        assert_eq!(decoder.read_bytes(4), Err(PrimitiveError::UnexpectedEof));
    }

    #[test]
    fn finish_rejects_trailing_bytes() {
        let decoder = Decoder::new(&[0xaa]);
        assert_eq!(decoder.finish(), Err(PrimitiveError::TrailingBytes));
    }
}
