use oregon_primitives::Hash256;
use oregon_protocol::{InventoryItem, InventoryKind};

pub(crate) fn validated_inventory<T, E>(
    kind: InventoryKind,
    hash: Hash256,
    result: &Result<T, E>,
) -> Option<InventoryItem> {
    result.as_ref().ok().map(|_| InventoryItem { kind, hash })
}
