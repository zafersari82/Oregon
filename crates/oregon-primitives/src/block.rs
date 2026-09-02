use crate::hash::domain_hash;
use crate::{DecodeLimits, Decoder, Hash256, PrimitiveError, Transaction, write_varint};

const BLOCK_DOMAIN: &[u8] = b"OREGON/BLOCK/V0\0";
const BLOCK_HEADER_BYTES: usize = 114;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    pub version: u16,
    pub previous_block: Hash256,
    pub transaction_root: Hash256,
    pub timestamp: u64,
    pub difficulty_commitment: [u8; 32],
    pub nonce: u64,
}

impl BlockHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(BLOCK_HEADER_BYTES);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(self.previous_block.as_bytes());
        bytes.extend_from_slice(self.transaction_root.as_bytes());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.difficulty_commitment);
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PrimitiveError> {
        let mut decoder = Decoder::new(bytes);
        let header = Self::decode_from(&mut decoder)?;
        decoder.finish()?;
        Ok(header)
    }

    fn decode_from(decoder: &mut Decoder<'_>) -> Result<Self, PrimitiveError> {
        let version = decoder.read_u16()?;
        let previous_block = Hash256::from_slice(decoder.read_bytes(32)?)?;
        let transaction_root = Hash256::from_slice(decoder.read_bytes(32)?)?;
        let timestamp = decoder.read_u64()?;
        let mut difficulty_commitment = [0u8; 32];
        difficulty_commitment.copy_from_slice(decoder.read_bytes(32)?);
        let nonce = decoder.read_u64()?;

        Ok(Self {
            version,
            previous_block,
            transaction_root,
            timestamp,
            difficulty_commitment,
            nonce,
        })
    }

    pub fn block_id(&self) -> Hash256 {
        domain_hash(BLOCK_DOMAIN, &self.encode())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = self.header.encode();
        write_varint(self.transactions.len() as u64, &mut bytes);
        for transaction in &self.transactions {
            bytes.extend_from_slice(&transaction.encode());
        }
        bytes
    }

    pub fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<Self, PrimitiveError> {
        if bytes.len() > limits.max_object_bytes {
            return Err(PrimitiveError::LengthLimitExceeded);
        }

        let mut decoder = Decoder::new(bytes);
        let header = BlockHeader::decode_from(&mut decoder)?;
        let transaction_count = decoder.read_len(limits.max_block_transactions)?;
        if transaction_count == 0 {
            return Err(PrimitiveError::EmptyBlockTransactions);
        }

        let mut transactions = Vec::with_capacity(transaction_count);
        for _ in 0..transaction_count {
            transactions.push(Transaction::decode_from(&mut decoder, limits)?);
        }
        decoder.finish()?;

        Ok(Self {
            header,
            transactions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecodeLimits, Hash256, PrimitiveError, Transaction, transaction_root};

    fn transaction(lock_time: u64) -> Transaction {
        Transaction {
            version: 1,
            inputs: vec![],
            outputs: vec![],
            lock_time,
        }
    }

    fn header(transactions: &[Transaction]) -> BlockHeader {
        BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x11; 32]),
            transaction_root: transaction_root(transactions).unwrap(),
            timestamp: 1_800_000_000,
            difficulty_commitment: [0x22; 32],
            nonce: 7,
        }
    }

    #[test]
    fn block_header_round_trips_exactly() {
        let transactions = vec![transaction(0)];
        let header = header(&transactions);
        let encoded = header.encode();

        assert_eq!(encoded.len(), 114);
        assert_eq!(BlockHeader::decode(&encoded).unwrap(), header);
    }

    #[test]
    fn changing_nonce_changes_block_id() {
        let transactions = vec![transaction(0)];
        let first = header(&transactions);
        let mut second = first.clone();
        second.nonce += 1;

        assert_ne!(first.block_id(), second.block_id());
    }

    #[test]
    fn block_id_depends_on_header_not_body_object() {
        let tx0 = transaction(0);
        let tx1 = transaction(1);
        let shared_header = header(std::slice::from_ref(&tx0));

        let first = Block {
            header: shared_header.clone(),
            transactions: vec![tx0],
        };
        let second = Block {
            header: shared_header,
            transactions: vec![tx1],
        };

        assert_eq!(first.header.block_id(), second.header.block_id());
        assert_ne!(first.transactions, second.transactions);
    }

    #[test]
    fn block_round_trips_exactly() {
        let transactions = vec![transaction(0), transaction(1)];
        let block = Block {
            header: header(&transactions),
            transactions,
        };
        let encoded = block.encode();
        let decoded = Block::decode(&encoded, &DecodeLimits::default()).unwrap();

        assert_eq!(decoded, block);
        assert_eq!(decoded.encode(), encoded);
    }

    #[test]
    fn empty_transaction_list_is_rejected_when_decoding_block() {
        let tx = transaction(0);
        let invalid = Block {
            header: header(std::slice::from_ref(&tx)),
            transactions: vec![],
        };

        assert_eq!(
            Block::decode(&invalid.encode(), &DecodeLimits::default()),
            Err(PrimitiveError::EmptyBlockTransactions)
        );
    }

    #[test]
    fn block_transaction_count_limit_is_enforced() {
        let transactions = vec![transaction(0)];
        let block = Block {
            header: header(&transactions),
            transactions,
        };
        let limits = DecodeLimits {
            max_block_transactions: 0,
            ..Default::default()
        };

        assert_eq!(
            Block::decode(&block.encode(), &limits),
            Err(PrimitiveError::LengthLimitExceeded)
        );
    }

    #[test]
    fn block_header_rejects_trailing_bytes() {
        let transactions = vec![transaction(0)];
        let mut encoded = header(&transactions).encode();
        encoded.push(0);

        assert_eq!(
            BlockHeader::decode(&encoded),
            Err(PrimitiveError::TrailingBytes)
        );
    }
}
