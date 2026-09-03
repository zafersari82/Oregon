#[cfg(test)]
mod tests {
    use super::retained_active_floor;
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
}
