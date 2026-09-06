use crate::execution_address::ExecutionAddress;
use crate::execution_receipt::ExecutionReceiptError;
use crate::{Hash256, domain_hash};

pub const MAX_EXECUTION_EVENT_TOPICS_V1: usize = 4;
pub const MAX_EXECUTION_EVENT_DATA_BYTES_V1: usize = 65_536;
pub const MAX_EXECUTION_EVENTS_V1: usize = 256;

const EVENT_VERSION_V1: u16 = 1;
const EVENT_DOMAIN: &[u8] = b"OREGON/EXEC/EVENT/V1\0";
const EVENTS_DOMAIN: &[u8] = b"OREGON/EXEC/EVENTS/V1\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionEventV1 {
    emitter: ExecutionAddress,
    topics: Vec<Hash256>,
    data: Vec<u8>,
}

impl ExecutionEventV1 {
    pub fn new(
        emitter: ExecutionAddress,
        topics: Vec<Hash256>,
        data: Vec<u8>,
    ) -> Result<Self, ExecutionReceiptError> {
        if topics.len() > MAX_EXECUTION_EVENT_TOPICS_V1 {
            return Err(ExecutionReceiptError::TooManyEventTopics);
        }
        if data.len() > MAX_EXECUTION_EVENT_DATA_BYTES_V1 {
            return Err(ExecutionReceiptError::EventDataTooLarge);
        }
        Ok(Self {
            emitter,
            topics,
            data,
        })
    }

    pub const fn emitter(&self) -> ExecutionAddress {
        self.emitter
    }

    pub fn topics(&self) -> &[Hash256] {
        &self.topics
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            2 + 33 + 1 + self.topics.len() * 32 + 4 + self.data.len(),
        );
        bytes.extend_from_slice(&EVENT_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&self.emitter.to_bytes());
        bytes.push(self.topics.len() as u8);
        for topic in &self.topics {
            bytes.extend_from_slice(topic.as_bytes());
        }
        bytes.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    pub fn event_id(&self) -> Hash256 {
        domain_hash(EVENT_DOMAIN, &self.encode())
    }
}

pub fn events_root(events: &[ExecutionEventV1]) -> Result<Hash256, ExecutionReceiptError> {
    if events.len() > MAX_EXECUTION_EVENTS_V1 {
        return Err(ExecutionReceiptError::TooManyEvents);
    }

    let mut bytes = Vec::with_capacity(4 + events.len() * 32);
    bytes.extend_from_slice(&(events.len() as u32).to_le_bytes());
    for event in events {
        bytes.extend_from_slice(event.event_id().as_bytes());
    }
    Ok(domain_hash(EVENTS_DOMAIN, &bytes))
}
