use crate::state::REORG_WINDOW;

pub(crate) fn retained_active_floor(height: u64) -> u64 {
    height.saturating_sub(REORG_WINDOW - 1)
}

#[cfg(test)]
mod tests {
    use super::{
        prune_cursor_for_tip, retained_active_floor, should_retain_body, should_retain_undo,
    };
    use crate::state::REORG_WINDOW;

    #[test]
    fn active_floor_retains_8064_blocks_and_saturates_at_zero() {
        assert_eq!(retained_active_floor(0), 0);
        assert_eq!(retained_active_floor(REORG_WINDOW - 1), 0);
        assert_eq!(retained_active_floor(REORG_WINDOW), 1);

        let tip = 20_000;
        let floor = retained_active_floor(tip);
        assert_eq!(floor, tip - (REORG_WINDOW - 1));
        assert_eq!(floor, 11_937);
        assert_eq!(tip - floor + 1, REORG_WINDOW);
    }

    #[test]
    fn prune_cursor_is_highest_active_height_eligible_for_pruning() {
        assert_eq!(prune_cursor_for_tip(0), 0);
        assert_eq!(prune_cursor_for_tip(REORG_WINDOW - 1), 0);
        assert_eq!(prune_cursor_for_tip(REORG_WINDOW), 0);
        assert_eq!(prune_cursor_for_tip(REORG_WINDOW + 1), 1);
        assert_eq!(prune_cursor_for_tip(20_000), 11_936);
    }

    #[test]
    fn body_retention_requires_live_height_and_permitted_common_fork_depth() {
        let tip = 20_000;
        let floor = retained_active_floor(tip);
        let deepest_permitted_fork = tip - REORG_WINDOW;

        assert!(should_retain_body(floor, deepest_permitted_fork, tip));
        assert!(!should_retain_body(
            floor - 1,
            deepest_permitted_fork,
            tip
        ));
        assert!(!should_retain_body(
            floor,
            deepest_permitted_fork - 1,
            tip
        ));
        assert!(should_retain_body(tip, tip, tip));
    }

    #[test]
    fn undo_is_retained_only_for_active_blocks_inside_live_window() {
        let tip = 20_000;
        let floor = retained_active_floor(tip);

        assert!(should_retain_undo(true, floor, tip));
        assert!(should_retain_undo(true, tip, tip));
        assert!(!should_retain_undo(true, floor - 1, tip));
        assert!(!should_retain_undo(false, floor, tip));
    }
}
