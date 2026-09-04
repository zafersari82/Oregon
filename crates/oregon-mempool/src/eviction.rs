use std::cmp::Ordering;

use crate::MempoolEntry;

pub(crate) fn eviction_cmp(left: &MempoolEntry, right: &MempoolEntry) -> Ordering {
    let left_cross = u128::from(left.fee()) * right.encoded_bytes() as u128;
    let right_cross = u128::from(right.fee()) * left.encoded_bytes() as u128;

    left_cross
        .cmp(&right_cross)
        .then_with(|| left.fee().cmp(&right.fee()))
        .then_with(|| left.txid().cmp(&right.txid()))
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::collections::BTreeSet;

    use oregon_primitives::{Hash256, Transaction};

    use crate::MempoolEntry;

    use super::eviction_cmp;

    fn entry(tag: u8, fee: u64, encoded_bytes: usize) -> MempoolEntry {
        MempoolEntry {
            transaction: Transaction {
                version: 1,
                inputs: vec![],
                outputs: vec![],
                lock_time: 0,
            },
            txid: Hash256::from_bytes([tag; 32]),
            fee,
            encoded_bytes,
            parents: BTreeSet::new(),
            children: BTreeSet::new(),
        }
    }

    #[test]
    fn comparator_uses_integer_rate_then_fee_then_txid() {
        let lower_rate = entry(0x30, 1, 20);
        let higher_rate = entry(0x40, 2, 20);
        assert_eq!(eviction_cmp(&lower_rate, &higher_rate), Ordering::Less);

        let equal_rate_lower_fee = entry(0x50, 1, 10);
        let equal_rate_higher_fee = entry(0x60, 2, 20);
        assert_eq!(
            eviction_cmp(&equal_rate_lower_fee, &equal_rate_higher_fee),
            Ordering::Less
        );

        let smaller_txid = entry(0x10, 2, 20);
        let larger_txid = entry(0x20, 2, 20);
        assert_eq!(eviction_cmp(&smaller_txid, &larger_txid), Ordering::Less);
        assert_eq!(eviction_cmp(&larger_txid, &smaller_txid), Ordering::Greater);
    }
}
