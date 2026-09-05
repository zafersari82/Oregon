use oregon_primitives::Hash256;

use crate::{ChainSyncView, SyncViewError};

pub async fn find_common_height<V: ChainSyncView + ?Sized>(
    view: &V,
) -> Result<u64, SyncViewError> {
    let active = view.active_tip().await?;
    let preferred = view.preferred_header_tip().await?;
    let mut height = active.height.min(preferred.height);

    loop {
        let active_id = view
            .active_id_at_height(height)
            .await?
            .ok_or(SyncViewError::Unavailable)?;
        let preferred_id = view
            .preferred_header_id_at_height(height)
            .await?
            .ok_or(SyncViewError::Unavailable)?;
        if active_id == preferred_id {
            return Ok(height);
        }
        if height == 0 {
            return Err(SyncViewError::Unavailable);
        }
        height -= 1;
    }
}

pub async fn missing_body_targets<V: ChainSyncView + ?Sized>(
    view: &V,
) -> Result<Vec<Hash256>, SyncViewError> {
    let common_height = find_common_height(view).await?;
    let preferred = view.preferred_header_tip().await?;
    if common_height >= preferred.height {
        return Ok(Vec::new());
    }

    let mut targets = Vec::new();
    let mut height = common_height + 1;
    while height <= preferred.height {
        let block_id = view
            .preferred_header_id_at_height(height)
            .await?
            .ok_or(SyncViewError::Unavailable)?;
        if !view.body_retained(block_id).await? {
            targets.push(block_id);
        }
        height += 1;
    }
    Ok(targets)
}
