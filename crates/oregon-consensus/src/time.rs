//! RED phase: median-time-past consensus tests.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConsensusError;

    #[test]
    fn even_early_mtp_uses_upper_median() {
        assert_eq!(median_time_past(&[100, 200]).unwrap(), 200);
    }

    #[test]
    fn odd_unsorted_window_uses_sorted_median() {
        assert_eq!(median_time_past(&[300, 100, 200]).unwrap(), 200);
    }

    #[test]
    fn empty_or_twelve_item_window_is_invalid() {
        assert_eq!(
            median_time_past(&[]),
            Err(ConsensusError::InvalidMtpWindow)
        );
        assert_eq!(
            median_time_past(&[0; 12]),
            Err(ConsensusError::InvalidMtpWindow)
        );
    }
}
