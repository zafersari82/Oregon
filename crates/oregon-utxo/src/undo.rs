use oregon_primitives::OutPoint;

use crate::UtxoEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockUndo {
    pub spent: Vec<(OutPoint, UtxoEntry)>,
    pub created: Vec<OutPoint>,
}
