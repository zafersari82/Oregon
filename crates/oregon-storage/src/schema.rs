use crate::error::StorageError;

pub(crate) const SCHEMA_KEY: &[u8] = b"schema/version";
#[cfg(test)]
const MIGRATION_MARKER_VERSION: u8 = 1;
#[cfg(test)]
const MIGRATION_MARKER_BYTES: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
}

pub(crate) const SCHEMA_VERSION: SchemaVersion = SchemaVersion { major: 1, minor: 0 };

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

#[cfg(test)]
pub(crate) fn encode_migration_marker(
    from: SchemaVersion,
    to: SchemaVersion,
) -> [u8; MIGRATION_MARKER_BYTES] {
    let from = encode_schema_version(from);
    let to = encode_schema_version(to);
    [
        MIGRATION_MARKER_VERSION,
        from[0],
        from[1],
        from[2],
        from[3],
        to[0],
        to[1],
        to[2],
        to[3],
    ]
}

#[cfg(test)]
pub(crate) fn decode_migration_marker(
    bytes: &[u8],
) -> Result<(SchemaVersion, SchemaVersion), StorageError> {
    if bytes.len() != MIGRATION_MARKER_BYTES {
        return Err(StorageError::CorruptData(
            "migration marker must be exactly 9 bytes".to_owned(),
        ));
    }
    if bytes[0] != MIGRATION_MARKER_VERSION {
        return Err(StorageError::CorruptData(format!(
            "unsupported migration marker version {}",
            bytes[0]
        )));
    }

    let from = decode_schema_version(&bytes[1..5])?;
    let to = decode_schema_version(&bytes[5..9])?;
    Ok((from, to))
}
