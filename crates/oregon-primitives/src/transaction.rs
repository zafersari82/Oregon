use crate::hash::domain_hash;
use crate::{Amount, DecodeLimits, Decoder, Hash256, PrimitiveError, write_varint};

const TX_DOMAIN: &[u8] = b"OREGON/TX/V0\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutPoint {
    pub txid: Hash256,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxInput {
    pub previous_txid: Hash256,
    pub previous_output_index: u32,
    pub sequence: u32,
    pub witness: Vec<Vec<u8>>,
}

impl TxInput {
    pub const fn outpoint(&self) -> OutPoint {
        OutPoint {
            txid: self.previous_txid,
            index: self.previous_output_index,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxOutput {
    pub value: Amount,
    pub locking_program: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub version: u16,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub lock_time: u64,
}

impl Transaction {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.version.to_le_bytes());

        write_varint(self.inputs.len() as u64, &mut bytes);
        for input in &self.inputs {
            bytes.extend_from_slice(input.previous_txid.as_bytes());
            bytes.extend_from_slice(&input.previous_output_index.to_le_bytes());
            bytes.extend_from_slice(&input.sequence.to_le_bytes());
            write_varint(input.witness.len() as u64, &mut bytes);
            for item in &input.witness {
                write_varint(item.len() as u64, &mut bytes);
                bytes.extend_from_slice(item);
            }
        }

        write_varint(self.outputs.len() as u64, &mut bytes);
        for output in &self.outputs {
            bytes.extend_from_slice(&output.value.base_units().to_le_bytes());
            write_varint(output.locking_program.len() as u64, &mut bytes);
            bytes.extend_from_slice(&output.locking_program);
        }

        bytes.extend_from_slice(&self.lock_time.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<Self, PrimitiveError> {
        if bytes.len() > limits.max_object_bytes {
            return Err(PrimitiveError::LengthLimitExceeded);
        }

        let mut decoder = Decoder::new(bytes);
        let version = decoder.read_u16()?;
        if version != 1 {
            return Err(PrimitiveError::InvalidVersion(version));
        }

        let input_count = decoder.read_len(limits.max_transaction_inputs)?;
        let mut inputs = Vec::with_capacity(input_count);
        for _ in 0..input_count {
            let previous_txid = Hash256::from_slice(decoder.read_bytes(32)?)?;
            let previous_output_index = decoder.read_u32()?;
            let sequence = decoder.read_u32()?;

            let witness_count = decoder.read_len(limits.max_witness_items_per_input)?;
            let mut witness = Vec::with_capacity(witness_count);
            for _ in 0..witness_count {
                let item_len = decoder.read_len(limits.max_witness_item_bytes)?;
                witness.push(decoder.read_bytes(item_len)?.to_vec());
            }

            inputs.push(TxInput {
                previous_txid,
                previous_output_index,
                sequence,
                witness,
            });
        }

        let output_count = decoder.read_len(limits.max_transaction_outputs)?;
        let mut outputs = Vec::with_capacity(output_count);
        for _ in 0..output_count {
            let value = Amount::from_base_units(decoder.read_u64()?)?;
            let program_len = decoder.read_len(limits.max_locking_program_bytes)?;
            let locking_program = decoder.read_bytes(program_len)?.to_vec();
            outputs.push(TxOutput {
                value,
                locking_program,
            });
        }

        let lock_time = decoder.read_u64()?;
        decoder.finish()?;

        Ok(Self {
            version,
            inputs,
            outputs,
            lock_time,
        })
    }

    pub fn txid(&self) -> Hash256 {
        domain_hash(TX_DOMAIN, &self.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rich_transaction() -> Transaction {
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0x11; 32]),
                previous_output_index: 3,
                sequence: 7,
                witness: vec![vec![0xaa, 0xbb], vec![0xcc]],
            }],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(42).unwrap(),
                locking_program: vec![0x51, 0x21, 0x02],
            }],
            lock_time: 9,
        }
    }

    #[test]
    fn version_one_minimum_transaction_round_trips_exactly() {
        let tx = Transaction {
            version: 1,
            inputs: vec![],
            outputs: vec![],
            lock_time: 0,
        };

        let encoded = tx.encode();
        let decoded = Transaction::decode(&encoded, &DecodeLimits::default()).unwrap();
        assert_eq!(decoded, tx);
        assert_eq!(decoded.encode(), encoded);
    }

    #[test]
    fn version_zero_is_consensus_invalid() {
        let bytes = [
            0x00, 0x00, // version 0
            0x00, // input count
            0x00, // output count
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // lock_time
        ];

        assert_eq!(
            Transaction::decode(&bytes, &DecodeLimits::default()),
            Err(PrimitiveError::InvalidVersion(0))
        );
    }

    #[test]
    fn witness_bytes_commit_to_transaction_id() {
        let base = rich_transaction();
        let mut changed = base.clone();
        changed.inputs[0].witness[0][0] = 0xab;

        assert_ne!(base.txid(), changed.txid());
    }

    #[test]
    fn input_count_limit_is_enforced_before_input_decoding() {
        let encoded = rich_transaction().encode();
        let mut limits = DecodeLimits::default();
        limits.max_transaction_inputs = 0;

        assert_eq!(
            Transaction::decode(&encoded, &limits),
            Err(PrimitiveError::LengthLimitExceeded)
        );
    }

    #[test]
    fn output_count_limit_is_enforced_before_output_decoding() {
        let encoded = rich_transaction().encode();
        let mut limits = DecodeLimits::default();
        limits.max_transaction_outputs = 0;

        assert_eq!(
            Transaction::decode(&encoded, &limits),
            Err(PrimitiveError::LengthLimitExceeded)
        );
    }

    #[test]
    fn witness_item_count_limit_is_enforced() {
        let encoded = rich_transaction().encode();
        let mut limits = DecodeLimits::default();
        limits.max_witness_items_per_input = 1;

        assert_eq!(
            Transaction::decode(&encoded, &limits),
            Err(PrimitiveError::LengthLimitExceeded)
        );
    }

    #[test]
    fn witness_item_byte_limit_is_enforced() {
        let encoded = rich_transaction().encode();
        let mut limits = DecodeLimits::default();
        limits.max_witness_item_bytes = 1;

        assert_eq!(
            Transaction::decode(&encoded, &limits),
            Err(PrimitiveError::LengthLimitExceeded)
        );
    }

    #[test]
    fn locking_program_byte_limit_is_enforced() {
        let encoded = rich_transaction().encode();
        let mut limits = DecodeLimits::default();
        limits.max_locking_program_bytes = 2;

        assert_eq!(
            Transaction::decode(&encoded, &limits),
            Err(PrimitiveError::LengthLimitExceeded)
        );
    }

    #[test]
    fn complete_object_byte_limit_is_enforced() {
        let encoded = rich_transaction().encode();
        let mut limits = DecodeLimits::default();
        limits.max_object_bytes = encoded.len() - 1;

        assert_eq!(
            Transaction::decode(&encoded, &limits),
            Err(PrimitiveError::LengthLimitExceeded)
        );
    }

    #[test]
    fn truncation_at_multiple_boundaries_is_rejected() {
        let encoded = rich_transaction().encode();
        let cut_points = [0, 1, 2, 3, 10, encoded.len() / 2, encoded.len() - 1];

        for cut in cut_points {
            assert!(
                Transaction::decode(&encoded[..cut], &DecodeLimits::default()).is_err(),
                "decoder unexpectedly accepted truncation at byte {cut}"
            );
        }
    }

    #[test]
    fn trailing_byte_after_valid_transaction_is_rejected() {
        let mut encoded = rich_transaction().encode();
        encoded.push(0x00);

        assert_eq!(
            Transaction::decode(&encoded, &DecodeLimits::default()),
            Err(PrimitiveError::TrailingBytes)
        );
    }

    fn hash_strategy() -> impl Strategy<Value = Hash256> {
        any::<[u8; 32]>().prop_map(Hash256::from_bytes)
    }

    fn input_strategy() -> impl Strategy<Value = TxInput> {
        (
            hash_strategy(),
            any::<u32>(),
            any::<u32>(),
            prop::collection::vec(prop::collection::vec(any::<u8>(), 0..16), 0..4),
        )
            .prop_map(
                |(previous_txid, previous_output_index, sequence, witness)| TxInput {
                    previous_txid,
                    previous_output_index,
                    sequence,
                    witness,
                },
            )
    }

    fn output_strategy() -> impl Strategy<Value = TxOutput> {
        (0u64..=10_000, prop::collection::vec(any::<u8>(), 0..32)).prop_map(
            |(value, locking_program)| TxOutput {
                value: Amount::from_base_units(value).unwrap(),
                locking_program,
            },
        )
    }

    fn transaction_strategy() -> impl Strategy<Value = Transaction> {
        (
            prop::collection::vec(input_strategy(), 0..4),
            prop::collection::vec(output_strategy(), 0..4),
            any::<u64>(),
        )
            .prop_map(|(inputs, outputs, lock_time)| Transaction {
                version: 1,
                inputs,
                outputs,
                lock_time,
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn canonical_round_trip_is_exact(tx in transaction_strategy()) {
            let encoded = tx.encode();
            let decoded = Transaction::decode(&encoded, &DecodeLimits::default()).unwrap();
            prop_assert_eq!(decoded.encode(), encoded);
            prop_assert_eq!(decoded, tx);
        }

        #[test]
        fn arbitrary_hostile_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
            let _ = Transaction::decode(&bytes, &DecodeLimits::default());
        }
    }
}
