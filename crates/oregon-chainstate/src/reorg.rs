use crate::state::REORG_WINDOW;

pub(crate) fn reorg_depth_allowed(depth: u64) -> bool {
    depth <= REORG_WINDOW
}

#[cfg(test)]
mod tests {
    use super::reorg_depth_allowed;

    #[test]
    fn reorg_window_accepts_8064_and_rejects_8065() {
        assert!(reorg_depth_allowed(8_064));
        assert!(!reorg_depth_allowed(8_065));
    }
}
