use crate::hash::domain_hash;
use crate::{Hash256, PrimitiveError, Transaction};

const MERKLE_LEAF_DOMAIN: &[u8] = b"OREGON/MERKLE-LEAF/V0\0";
const MERKLE_NODE_DOMAIN: &[u8] = b"OREGON/MERKLE/V0\0";

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction(lock_time: u64) -> Transaction {
        Transaction {
            version: 1,
            inputs: vec![],
            outputs: vec![],
            lock_time,
        }
    }

    fn leaf(transaction: &Transaction) -> Hash256 {
        domain_hash(MERKLE_LEAF_DOMAIN, transaction.txid().as_bytes())
    }

    fn node(left: Hash256, right: Hash256) -> Hash256 {
        let mut payload = [0u8; 64];
        payload[..32].copy_from_slice(left.as_bytes());
        payload[32..].copy_from_slice(right.as_bytes());
        domain_hash(MERKLE_NODE_DOMAIN, &payload)
    }

    #[test]
    fn empty_transaction_list_is_invalid() {
        assert_eq!(
            transaction_root(&[]),
            Err(PrimitiveError::EmptyBlockTransactions)
        );
    }

    #[test]
    fn one_transaction_root_is_its_domain_separated_leaf() {
        let tx0 = transaction(0);
        assert_eq!(transaction_root(&[tx0.clone()]).unwrap(), leaf(&tx0));
    }

    #[test]
    fn two_transaction_root_hashes_ordered_leaves() {
        let tx0 = transaction(0);
        let tx1 = transaction(1);
        let expected = node(leaf(&tx0), leaf(&tx1));

        assert_eq!(transaction_root(&[tx0, tx1]).unwrap(), expected);
    }

    #[test]
    fn three_transaction_root_promotes_last_leaf_without_duplication() {
        let tx0 = transaction(0);
        let tx1 = transaction(1);
        let tx2 = transaction(2);

        let l0 = leaf(&tx0);
        let l1 = leaf(&tx1);
        let l2 = leaf(&tx2);
        let p0 = node(l0, l1);
        let expected = node(p0, l2);
        let duplicate_last_candidate = node(p0, node(l2, l2));

        let actual = transaction_root(&[tx0, tx1, tx2]).unwrap();
        assert_eq!(actual, expected);
        assert_ne!(actual, duplicate_last_candidate);
    }
}
