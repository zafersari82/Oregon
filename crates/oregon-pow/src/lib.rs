pub const RANDOMX_UPSTREAM_COMMIT: &str = "aaafe71322df6602c21a5c72937ac284724ae561";
pub const OREGON_RANDOMX_ARGON_SALT: &str = "OREGON-RANDOMX-V1";

#[cfg(test)]
mod tests {
    use super::{OREGON_RANDOMX_ARGON_SALT, RANDOMX_UPSTREAM_COMMIT};

    #[test]
    fn randomx_provenance_is_frozen() {
        assert_eq!(
            RANDOMX_UPSTREAM_COMMIT,
            "aaafe71322df6602c21a5c72937ac284724ae561"
        );
        assert_eq!(OREGON_RANDOMX_ARGON_SALT, "OREGON-RANDOMX-V1");
    }
}
