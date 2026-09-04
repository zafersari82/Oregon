#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolConfig {
    pub max_entries: usize,
    pub max_total_bytes: usize,
    pub max_ancestors: usize,
    pub max_descendants: usize,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_entries: 50_000,
            max_total_bytes: 64 * 1024 * 1024,
            max_ancestors: 25,
            max_descendants: 25,
        }
    }
}
