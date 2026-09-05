use oregon_primitives::{
    Block, BlockHeader, DecodeLimits, Decoder, Hash256, Transaction, write_varint,
};

use crate::constants::{
    BLOCK_HEADER_BYTES, MAX_GETDATA_ITEMS, MAX_HEADERS_PER_MESSAGE, MAX_INV_ITEMS,
    MAX_LOCATOR_HASHES, TAG_BLOCK, TAG_GET_DATA, TAG_GET_HEADERS, TAG_HEADERS, TAG_HELLO,
    TAG_HELLO_ACK, TAG_INV, TAG_PING, TAG_PONG, TAG_TRANSACTION, is_known_message_type,
    payload_limit,
};
use crate::{FeatureSet, ProtocolError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub min_protocol_version: u16,
    pub max_protocol_version: u16,
    pub chain_id: Hash256,
    pub instance_nonce: [u8; 16],
    pub offered_features: FeatureSet,
    pub required_features: FeatureSet,
    pub best_height: u64,
    pub best_block_id: Hash256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloAck {
    pub selected_protocol_version: u16,
    pub enabled_features: FeatureSet,
    pub remote_nonce_echo: [u8; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InventoryKind {
    Transaction,
    Block,
}

impl InventoryKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Transaction => 0x01,
            Self::Block => 0x02,
        }
    }

    fn decode(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0x01 => Ok(Self::Transaction),
            0x02 => Ok(Self::Block),
            other => Err(ProtocolError::UnknownInventoryKind(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InventoryItem {
    pub kind: InventoryKind,
    pub hash: Hash256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetHeaders {
    pub locator: Vec<Hash256>,
    pub stop: Option<Hash256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Hello(Hello),
    HelloAck(HelloAck),
    Ping(u64),
    Pong(u64),
    Inv(Vec<InventoryItem>),
    GetData(Vec<InventoryItem>),
    GetHeaders(GetHeaders),
    Headers(Vec<BlockHeader>),
    Transaction(Transaction),
    Block(Block),
}

pub fn encode_message(message: &Message) -> Result<(u8, Vec<u8>), ProtocolError> {
    let (message_type, payload) = match message {
        Message::Hello(hello) => (TAG_HELLO, encode_hello(hello)),
        Message::HelloAck(ack) => (TAG_HELLO_ACK, encode_hello_ack(ack)),
        Message::Ping(nonce) => (TAG_PING, nonce.to_le_bytes().to_vec()),
        Message::Pong(nonce) => (TAG_PONG, nonce.to_le_bytes().to_vec()),
        Message::Inv(items) => (TAG_INV, encode_inventory(items, MAX_INV_ITEMS)?),
        Message::GetData(items) => (TAG_GET_DATA, encode_inventory(items, MAX_GETDATA_ITEMS)?),
        Message::GetHeaders(request) => (TAG_GET_HEADERS, encode_get_headers(request)?),
        Message::Headers(headers) => (TAG_HEADERS, encode_headers(headers)?),
        Message::Transaction(transaction) => (TAG_TRANSACTION, transaction.encode()),
        Message::Block(block) => (TAG_BLOCK, block.encode()),
    };

    let max = payload_limit(message_type);
    if payload.len() > max {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            max,
        });
    }
    Ok((message_type, payload))
}

pub fn decode_message(message_type: u8, payload: &[u8]) -> Result<Message, ProtocolError> {
    if !is_known_message_type(message_type) {
        return Err(ProtocolError::UnknownMessageType(message_type));
    }
    let max = payload_limit(message_type);
    if payload.len() > max {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            max,
        });
    }

    match message_type {
        TAG_HELLO => decode_hello(payload).map(Message::Hello),
        TAG_HELLO_ACK => decode_hello_ack(payload).map(Message::HelloAck),
        TAG_PING => decode_nonce(payload).map(Message::Ping),
        TAG_PONG => decode_nonce(payload).map(Message::Pong),
        TAG_INV => decode_inventory(payload, MAX_INV_ITEMS).map(Message::Inv),
        TAG_GET_DATA => decode_inventory(payload, MAX_GETDATA_ITEMS).map(Message::GetData),
        TAG_GET_HEADERS => decode_get_headers(payload).map(Message::GetHeaders),
        TAG_HEADERS => decode_headers(payload).map(Message::Headers),
        TAG_TRANSACTION => Transaction::decode(payload, &DecodeLimits::default())
            .map(Message::Transaction)
            .map_err(ProtocolError::from),
        TAG_BLOCK => Block::decode(payload, &DecodeLimits::default())
            .map(Message::Block)
            .map_err(ProtocolError::from),
        other => Err(ProtocolError::UnknownMessageType(other)),
    }
}

fn encode_hello(hello: &Hello) -> Vec<u8> {
    let mut payload = Vec::with_capacity(108);
    payload.extend_from_slice(&hello.min_protocol_version.to_le_bytes());
    payload.extend_from_slice(&hello.max_protocol_version.to_le_bytes());
    payload.extend_from_slice(hello.chain_id.as_bytes());
    payload.extend_from_slice(&hello.instance_nonce);
    payload.extend_from_slice(&hello.offered_features.bits().to_le_bytes());
    payload.extend_from_slice(&hello.required_features.bits().to_le_bytes());
    payload.extend_from_slice(&hello.best_height.to_le_bytes());
    payload.extend_from_slice(hello.best_block_id.as_bytes());
    payload
}

fn decode_hello(payload: &[u8]) -> Result<Hello, ProtocolError> {
    let mut decoder = Decoder::new(payload);
    let min_protocol_version = decoder.read_u16()?;
    let max_protocol_version = decoder.read_u16()?;
    let chain_id = read_hash(&mut decoder)?;
    let mut instance_nonce = [0u8; 16];
    instance_nonce.copy_from_slice(decoder.read_bytes(16)?);
    let offered_features = FeatureSet::from_bits(decoder.read_u64()?);
    let required_features = FeatureSet::from_bits(decoder.read_u64()?);
    let best_height = decoder.read_u64()?;
    let best_block_id = read_hash(&mut decoder)?;
    decoder.finish()?;
    Ok(Hello {
        min_protocol_version,
        max_protocol_version,
        chain_id,
        instance_nonce,
        offered_features,
        required_features,
        best_height,
        best_block_id,
    })
}

fn encode_hello_ack(ack: &HelloAck) -> Vec<u8> {
    let mut payload = Vec::with_capacity(26);
    payload.extend_from_slice(&ack.selected_protocol_version.to_le_bytes());
    payload.extend_from_slice(&ack.enabled_features.bits().to_le_bytes());
    payload.extend_from_slice(&ack.remote_nonce_echo);
    payload
}

fn decode_hello_ack(payload: &[u8]) -> Result<HelloAck, ProtocolError> {
    let mut decoder = Decoder::new(payload);
    let selected_protocol_version = decoder.read_u16()?;
    let enabled_features = FeatureSet::from_bits(decoder.read_u64()?);
    let mut remote_nonce_echo = [0u8; 16];
    remote_nonce_echo.copy_from_slice(decoder.read_bytes(16)?);
    decoder.finish()?;
    Ok(HelloAck {
        selected_protocol_version,
        enabled_features,
        remote_nonce_echo,
    })
}

fn decode_nonce(payload: &[u8]) -> Result<u64, ProtocolError> {
    let mut decoder = Decoder::new(payload);
    let nonce = decoder.read_u64()?;
    decoder.finish()?;
    Ok(nonce)
}

fn encode_inventory(items: &[InventoryItem], max: usize) -> Result<Vec<u8>, ProtocolError> {
    enforce_list_limit(items.len(), max)?;
    let mut payload = Vec::new();
    write_varint(items.len() as u64, &mut payload);
    for item in items {
        payload.push(item.kind.tag());
        payload.extend_from_slice(item.hash.as_bytes());
    }
    Ok(payload)
}

fn decode_inventory(payload: &[u8], max: usize) -> Result<Vec<InventoryItem>, ProtocolError> {
    let mut decoder = Decoder::new(payload);
    let count = decoder.read_len(max)?;
    let mut items = Vec::new();
    for _ in 0..count {
        let kind = InventoryKind::decode(decoder.read_bytes(1)?[0])?;
        let hash = read_hash(&mut decoder)?;
        items.push(InventoryItem { kind, hash });
    }
    decoder.finish()?;
    Ok(items)
}

fn encode_get_headers(request: &GetHeaders) -> Result<Vec<u8>, ProtocolError> {
    enforce_list_limit(request.locator.len(), MAX_LOCATOR_HASHES)?;
    let mut payload = Vec::new();
    write_varint(request.locator.len() as u64, &mut payload);
    for hash in &request.locator {
        payload.extend_from_slice(hash.as_bytes());
    }
    match request.stop {
        None => payload.push(0),
        Some(stop) => {
            payload.push(1);
            payload.extend_from_slice(stop.as_bytes());
        }
    }
    Ok(payload)
}

fn decode_get_headers(payload: &[u8]) -> Result<GetHeaders, ProtocolError> {
    let mut decoder = Decoder::new(payload);
    let count = decoder.read_len(MAX_LOCATOR_HASHES)?;
    let mut locator = Vec::new();
    for _ in 0..count {
        locator.push(read_hash(&mut decoder)?);
    }
    let stop = match decoder.read_bytes(1)?[0] {
        0 => None,
        1 => Some(read_hash(&mut decoder)?),
        other => return Err(ProtocolError::InvalidStopFlag(other)),
    };
    decoder.finish()?;
    Ok(GetHeaders { locator, stop })
}

fn encode_headers(headers: &[BlockHeader]) -> Result<Vec<u8>, ProtocolError> {
    enforce_list_limit(headers.len(), MAX_HEADERS_PER_MESSAGE)?;
    let mut payload = Vec::new();
    write_varint(headers.len() as u64, &mut payload);
    for header in headers {
        payload.extend_from_slice(&header.encode());
    }
    Ok(payload)
}

fn decode_headers(payload: &[u8]) -> Result<Vec<BlockHeader>, ProtocolError> {
    let mut decoder = Decoder::new(payload);
    let count = decoder.read_len(MAX_HEADERS_PER_MESSAGE)?;
    let mut headers = Vec::new();
    for _ in 0..count {
        headers.push(BlockHeader::decode(
            decoder.read_bytes(BLOCK_HEADER_BYTES)?,
        )?);
    }
    decoder.finish()?;
    Ok(headers)
}

fn read_hash(decoder: &mut Decoder<'_>) -> Result<Hash256, ProtocolError> {
    Hash256::from_slice(decoder.read_bytes(32)?).map_err(ProtocolError::from)
}

fn enforce_list_limit(actual: usize, max: usize) -> Result<(), ProtocolError> {
    if actual > max {
        return Err(ProtocolError::ListLimitExceeded { actual, max });
    }
    Ok(())
}
