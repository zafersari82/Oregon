use oregon_primitives::{BlockHeader, Hash256};
use oregon_protocol::{GetHeaders, MAX_HEADERS_PER_MESSAGE, MAX_LOCATOR_HASHES};

use crate::{ChainSyncView, SyncError, SyncViewError};

pub fn locator_heights(tip_height: u64) -> Vec<u64> {
    let mut heights = Vec::with_capacity(MAX_LOCATOR_HASHES);
    let mut height = tip_height;
    let mut step = 1u64;

    loop {
        if heights.len() + 1 == MAX_LOCATOR_HASHES && height != 0 {
            heights.push(0);
            break;
        }
        heights.push(height);
        if height == 0 {
            break;
        }
        if heights.len() >= 10 {
            step = step.saturating_mul(2);
        }
        height = height.saturating_sub(step);
    }

    heights
}

pub async fn build_locator<V: ChainSyncView + ?Sized>(
    view: &V,
    stop: Option<Hash256>,
) -> Result<GetHeaders, SyncViewError> {
    let tip = view.preferred_header_tip().await?;
    let heights = locator_heights(tip.height);
    let mut locator = Vec::with_capacity(heights.len());
    for height in heights {
        let block_id = view
            .preferred_header_id_at_height(height)
            .await?
            .ok_or(SyncViewError::Unavailable)?;
        locator.push(block_id);
    }
    Ok(GetHeaders { locator, stop })
}

pub fn highest_locator_hit(
    locator: &[Hash256],
    preferred_path: &[(u64, Hash256)],
) -> Option<(u64, Hash256)> {
    let mut best = None;
    for locator_id in locator {
        for (height, local_id) in preferred_path {
            if locator_id == local_id && best.is_none_or(|(best_height, _)| *height > best_height) {
                best = Some((*height, *local_id));
            }
        }
    }
    best
}

pub fn validate_headers_response(
    common_ancestor: Hash256,
    headers: &[BlockHeader],
) -> Result<(), SyncError> {
    if headers.len() > MAX_HEADERS_PER_MESSAGE {
        return Err(SyncError::TooManyHeaders);
    }
    let Some(first) = headers.first() else {
        return Ok(());
    };
    if first.previous_block != common_ancestor {
        return Err(SyncError::DetachedHeaders);
    }
    for pair in headers.windows(2) {
        if pair[1].previous_block != pair[0].block_id() {
            return Err(SyncError::NonContiguousHeaders);
        }
    }
    Ok(())
}

pub async fn headers_after_common_height<V: ChainSyncView + ?Sized>(
    view: &V,
    common_height: u64,
    stop: Option<Hash256>,
) -> Result<Vec<BlockHeader>, SyncViewError> {
    let tip = view.preferred_header_tip().await?;
    if common_height >= tip.height {
        return Ok(Vec::new());
    }

    let mut headers = Vec::new();
    let mut height = common_height.saturating_add(1);
    while height <= tip.height && headers.len() < MAX_HEADERS_PER_MESSAGE {
        let Some(header) = view.preferred_header_at_height(height).await? else {
            return Err(SyncViewError::Unavailable);
        };
        let block_id = header.block_id();
        headers.push(header);
        if stop == Some(block_id) {
            break;
        }
        height = height.saturating_add(1);
    }
    Ok(headers)
}
