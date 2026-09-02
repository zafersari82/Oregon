use std::fmt;
use std::str::FromStr;

use crate::PrimitiveError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash256([u8; 32]);

impl Hash256 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, PrimitiveError> {
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| PrimitiveError::InvalidHashLength(bytes.len()))?;
        Ok(Self(array))
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for Hash256 {
    type Err = PrimitiveError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 || !value.is_ascii() {
            return Err(PrimitiveError::InvalidHashHex);
        }

        let bytes = value.as_bytes();
        let mut decoded = [0u8; 32];
        for (index, output) in decoded.iter_mut().enumerate() {
            let high = decode_lower_hex(bytes[index * 2])?;
            let low = decode_lower_hex(bytes[index * 2 + 1])?;
            *output = (high << 4) | low;
        }
        Ok(Self(decoded))
    }
}

fn decode_lower_hex(value: u8) -> Result<u8, PrimitiveError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(PrimitiveError::InvalidHashHex),
    }
}

pub(crate) fn domain_hash(domain: &[u8], payload: &[u8]) -> Hash256 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(payload);
    Hash256::from_bytes(*hasher.finalize().as_bytes())
}

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
