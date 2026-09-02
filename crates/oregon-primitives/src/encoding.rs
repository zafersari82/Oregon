use crate::PrimitiveError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_transaction_inputs: usize,
    pub max_transaction_outputs: usize,
    pub max_witness_items_per_input: usize,
    pub max_witness_item_bytes: usize,
    pub max_locking_program_bytes: usize,
    pub max_block_transactions: usize,
    pub max_object_bytes: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_transaction_inputs: 65_535,
            max_transaction_outputs: 65_535,
            max_witness_items_per_input: 1_024,
            max_witness_item_bytes: 1_048_576,
            max_locking_program_bytes: 65_536,
            max_block_transactions: 1_000_000,
            max_object_bytes: 67_108_864,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Decoder<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    pub const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, PrimitiveError> {
        let byte = *self
            .input
            .get(self.offset)
            .ok_or(PrimitiveError::UnexpectedEof)?;
        self.offset += 1;
        Ok(byte)
    }

    pub fn read_u16(&mut self) -> Result<u16, PrimitiveError> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u32(&mut self) -> Result<u32, PrimitiveError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_u64(&mut self) -> Result<u64, PrimitiveError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub fn read_varint(&mut self) -> Result<u64, PrimitiveError> {
        match self.read_u8()? {
            marker @ 0x00..=0xfc => Ok(u64::from(marker)),
            0xfd => {
                let value = u64::from(self.read_u16()?);
                if value < 0xfd {
                    return Err(PrimitiveError::NonCanonicalVarInt);
                }
                Ok(value)
            }
            0xfe => {
                let value = u64::from(self.read_u32()?);
                if value <= u64::from(u16::MAX) {
                    return Err(PrimitiveError::NonCanonicalVarInt);
                }
                Ok(value)
            }
            0xff => {
                let value = self.read_u64()?;
                if value <= u64::from(u32::MAX) {
                    return Err(PrimitiveError::NonCanonicalVarInt);
                }
                Ok(value)
            }
        }
    }

    pub fn read_len(&mut self, max: usize) -> Result<usize, PrimitiveError> {
        let value = self.read_varint()?;
        let value = usize::try_from(value).map_err(|_| PrimitiveError::LengthLimitExceeded)?;
        if value > max {
            return Err(PrimitiveError::LengthLimitExceeded);
        }
        Ok(value)
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], PrimitiveError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(PrimitiveError::LengthLimitExceeded)?;
        if end > self.input.len() {
            return Err(PrimitiveError::UnexpectedEof);
        }
        let bytes = &self.input[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    pub fn finish(self) -> Result<(), PrimitiveError> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(PrimitiveError::TrailingBytes)
        }
    }
}

pub fn write_varint(value: u64, output: &mut Vec<u8>) {
    match value {
        0x00..=0xfc => output.push(value as u8),
        0xfd..=0xffff => {
            output.push(0xfd);
            output.extend_from_slice(&(value as u16).to_le_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            output.push(0xfe);
            output.extend_from_slice(&(value as u32).to_le_bytes());
        }
        _ => {
            output.push(0xff);
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
}

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
