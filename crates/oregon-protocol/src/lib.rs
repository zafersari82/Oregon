#![forbid(unsafe_code)]

mod constants;
mod error;
mod features;
mod frame;
mod message;

pub use constants::{
    FRAME_HEADER_BYTES, FRAME_VERSION, MAX_FRAME_PAYLOAD, MAX_GETDATA_ITEMS, MAX_HANDSHAKE_PAYLOAD,
    MAX_HEADERS_PER_MESSAGE, MAX_INV_ITEMS, MAX_LOCATOR_HASHES, PROTOCOL_VERSION_CURRENT,
    PROTOCOL_VERSION_MIN, TAG_BLOCK, TAG_GET_DATA, TAG_GET_HEADERS, TAG_HEADERS, TAG_HELLO,
    TAG_HELLO_ACK, TAG_INV, TAG_PING, TAG_PONG, TAG_TRANSACTION,
};
pub use error::ProtocolError;
pub use features::{FeatureSet, Negotiated, negotiate};
pub use frame::{FrameHeader, build_frame_header, network_magic, verify_frame_payload};
pub use message::{
    GetHeaders, Hello, HelloAck, InventoryItem, InventoryKind, Message, decode_message,
    encode_message,
};
pub use oregon_primitives::Hash256;

#[cfg(test)]
mod tests;
