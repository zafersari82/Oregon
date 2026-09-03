use crate::error::StorageError;

pub(crate) const SCHEMA_KEY: &[u8] = b"schema/version";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
}

pub const SCHEMA_VERSION: SchemaVersion = SchemaVersion { major: 1, minor: 0 };

pub(crate) fn encode_schema_version(version: SchemaVersion) -> [u8; 4] {
    let major = version.major.to_be_bytes();
    let minor = version.minor.to_be_bytes();
    [major[0], major[1], minor[0], minor[1]]
}

pub(crate) fn decode_schema_version(bytes: &[u8]) -> Result<SchemaVersion, StorageError> {
    if bytes.len() != 4 {
        return Err(StorageError::CorruptData(
            "schema version must be exactly 4 bytes".to_owned(),
        ));
    }

    Ok(SchemaVersion {
        major: u16::from_be_bytes([bytes[0], bytes[1]]),
        minor: u16::from_be_bytes([bytes[2], bytes[3]]),
    })
}
