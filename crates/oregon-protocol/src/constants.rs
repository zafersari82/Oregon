pub const FRAME_HEADER_BYTES: usize = 16;
pub const FRAME_VERSION: u8 = 1;

pub const PROTOCOL_VERSION_CURRENT: u16 = 1;
pub const PROTOCOL_VERSION_MIN: u16 = 1;

pub const MAX_FRAME_PAYLOAD: usize = 2 * 1024 * 1024;
pub const MAX_HANDSHAKE_PAYLOAD: usize = 4 * 1024;
pub const MAX_INV_ITEMS: usize = 4_096;
pub const MAX_GETDATA_ITEMS: usize = 128;
pub const MAX_LOCATOR_HASHES: usize = 64;
pub const MAX_HEADERS_PER_MESSAGE: usize = 128;

pub const TAG_HELLO: u8 = 0x01;
pub const TAG_HELLO_ACK: u8 = 0x02;
pub const TAG_PING: u8 = 0x03;
pub const TAG_PONG: u8 = 0x04;
pub const TAG_INV: u8 = 0x10;
pub const TAG_GET_DATA: u8 = 0x11;
pub const TAG_GET_HEADERS: u8 = 0x20;
pub const TAG_HEADERS: u8 = 0x21;
pub const TAG_TRANSACTION: u8 = 0x30;
pub const TAG_BLOCK: u8 = 0x31;

pub(crate) const BLOCK_HEADER_BYTES: usize = 114;

pub(crate) const fn is_known_message_type(message_type: u8) -> bool {
    matches!(
        message_type,
        TAG_HELLO
            | TAG_HELLO_ACK
            | TAG_PING
            | TAG_PONG
            | TAG_INV
            | TAG_GET_DATA
            | TAG_GET_HEADERS
            | TAG_HEADERS
            | TAG_TRANSACTION
            | TAG_BLOCK
    )
}

pub(crate) const fn payload_limit(message_type: u8) -> usize {
    match message_type {
        TAG_HELLO | TAG_HELLO_ACK => MAX_HANDSHAKE_PAYLOAD,
        _ => MAX_FRAME_PAYLOAD,
    }
}
