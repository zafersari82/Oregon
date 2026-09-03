use oregon_primitives::TxOutput;

pub const COINBASE_MATURITY: u64 = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtxoEntry {
    pub output: TxOutput,
    pub creation_height: u64,
    pub is_coinbase: bool,
}

impl UtxoEntry {
    pub fn is_spendable_at(&self, spend_height: u64) -> bool {
        if !self.is_coinbase {
            return true;
        }

        self.creation_height
            .checked_add(COINBASE_MATURITY - 1)
            .is_some_and(|mature_height| spend_height >= mature_height)
    }
}
