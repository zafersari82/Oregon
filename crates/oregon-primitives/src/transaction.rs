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
    use crate::DecodeLimits;

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
        let previous_txid = Hash256::from_bytes([0x11; 32]);
        let base = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid,
                previous_output_index: 3,
                sequence: 7,
                witness: vec![vec![0xaa]],
            }],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(42).unwrap(),
                locking_program: vec![0x51],
            }],
            lock_time: 9,
        };

        let mut changed = base.clone();
        changed.inputs[0].witness[0][0] = 0xab;

        assert_ne!(base.txid(), changed.txid());
    }
}
