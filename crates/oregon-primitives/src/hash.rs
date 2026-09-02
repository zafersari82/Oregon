#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_requires_exactly_32_bytes() {
        assert!(Hash256::from_slice(&[0u8; 31]).is_err());
        assert!(Hash256::from_slice(&[0u8; 32]).is_ok());
        assert!(Hash256::from_slice(&[0u8; 33]).is_err());
    }

    #[test]
    fn hash_hex_is_lowercase_and_round_trips() {
        let hash = Hash256::from_bytes([0xab; 32]);
        let text = hash.to_string();
        assert_eq!(text.len(), 64);
        assert!(text.bytes().all(|b| !b.is_ascii_uppercase()));
        assert_eq!(text.parse::<Hash256>().unwrap(), hash);
    }

    #[test]
    fn uppercase_hex_is_rejected() {
        let text = "AB".repeat(32);
        assert!(text.parse::<Hash256>().is_err());
    }

    #[test]
    fn domains_change_hash_identity() {
        let payload = b"same payload";
        assert_ne!(
            domain_hash(b"OREGON/TX/V0\0", payload),
            domain_hash(b"OREGON/BLOCK/V0\0", payload)
        );
    }
}
