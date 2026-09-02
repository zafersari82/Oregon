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
