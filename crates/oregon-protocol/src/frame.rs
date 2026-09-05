use oregon_primitives::{Decoder, Hash256};

use crate::ProtocolError;
use crate::constants::{FRAME_HEADER_BYTES, FRAME_VERSION, is_known_message_type, payload_limit};

const NETWORK_MAGIC_DOMAIN: &[u8] = b"OREGON/NETMAGIC/V1\0";
const FRAME_DOMAIN: &[u8] = b"OREGON/FRAME/V1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub network_magic: [u8; 4],
    pub frame_version: u8,
    pub message_type: u8,
    pub flags: u16,
    pub payload_length: u32,
    pub checksum: [u8; 4],
}

impl FrameHeader {
    pub fn encode(&self) -> [u8; FRAME_HEADER_BYTES] {
        let mut bytes = [0u8; FRAME_HEADER_BYTES];
        bytes[..4].copy_from_slice(&self.network_magic);
        bytes[4] = self.frame_version;
        bytes[5] = self.message_type;
        bytes[6..8].copy_from_slice(&self.flags.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.payload_length.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.checksum);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut decoder = Decoder::new(bytes);
        let mut network_magic = [0u8; 4];
        network_magic.copy_from_slice(decoder.read_bytes(4)?);
        let frame_version = decoder.read_bytes(1)?[0];
        let message_type = decoder.read_bytes(1)?[0];
        let flags = decoder.read_u16()?;
        let payload_length = decoder.read_u32()?;
        let mut checksum = [0u8; 4];
        checksum.copy_from_slice(decoder.read_bytes(4)?);
        decoder.finish()?;

        validate_header_fields(frame_version, message_type, flags, payload_length)?;
        Ok(Self {
            network_magic,
            frame_version,
            message_type,
            flags,
            payload_length,
            checksum,
        })
    }
}

pub fn network_magic(chain_id: Hash256) -> [u8; 4] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(NETWORK_MAGIC_DOMAIN);
    hasher.update(chain_id.as_bytes());
    first_four(hasher.finalize().as_bytes())
}

pub fn build_frame_header(
    network_magic: [u8; 4],
    message_type: u8,
    payload: &[u8],
) -> Result<FrameHeader, ProtocolError> {
    validate_payload_size(message_type, payload.len())?;
    let payload_length =
        u32::try_from(payload.len()).map_err(|_| ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            max: payload_limit(message_type),
        })?;
    validate_header_fields(FRAME_VERSION, message_type, 0, payload_length)?;

    Ok(FrameHeader {
        network_magic,
        frame_version: FRAME_VERSION,
        message_type,
        flags: 0,
        payload_length,
        checksum: frame_checksum(network_magic, message_type, payload_length, payload),
    })
}

pub fn verify_frame_payload(
    header: &FrameHeader,
    expected_magic: [u8; 4],
    payload: &[u8],
) -> Result<(), ProtocolError> {
    validate_header_fields(
        header.frame_version,
        header.message_type,
        header.flags,
        header.payload_length,
    )?;
    if header.network_magic != expected_magic {
        return Err(ProtocolError::WrongNetworkMagic);
    }
    if usize::try_from(header.payload_length).ok() != Some(payload.len()) {
        return Err(ProtocolError::PayloadLengthMismatch {
            declared: header.payload_length,
            actual: payload.len(),
        });
    }
    let expected = frame_checksum(
        header.network_magic,
        header.message_type,
        header.payload_length,
        payload,
    );
    if header.checksum != expected {
        return Err(ProtocolError::ChecksumMismatch);
    }
    Ok(())
}

fn validate_header_fields(
    frame_version: u8,
    message_type: u8,
    flags: u16,
    payload_length: u32,
) -> Result<(), ProtocolError> {
    if frame_version != FRAME_VERSION {
        return Err(ProtocolError::UnsupportedFrameVersion(frame_version));
    }
    if !is_known_message_type(message_type) {
        return Err(ProtocolError::UnknownMessageType(message_type));
    }
    if flags != 0 {
        return Err(ProtocolError::NonZeroFlags(flags));
    }
    validate_payload_size(message_type, payload_length as usize)
}

fn validate_payload_size(message_type: u8, actual: usize) -> Result<(), ProtocolError> {
    if !is_known_message_type(message_type) {
        return Err(ProtocolError::UnknownMessageType(message_type));
    }
    let max = payload_limit(message_type);
    if actual > max {
        return Err(ProtocolError::PayloadTooLarge { actual, max });
    }
    Ok(())
}

fn frame_checksum(
    network_magic: [u8; 4],
    message_type: u8,
    payload_length: u32,
    payload: &[u8],
) -> [u8; 4] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(FRAME_DOMAIN);
    hasher.update(&network_magic);
    hasher.update(&[FRAME_VERSION, message_type]);
    hasher.update(&0u16.to_le_bytes());
    hasher.update(&payload_length.to_le_bytes());
    hasher.update(payload);
    first_four(hasher.finalize().as_bytes())
}

fn first_four(bytes: &[u8; 32]) -> [u8; 4] {
    [bytes[0], bytes[1], bytes[2], bytes[3]]
}
