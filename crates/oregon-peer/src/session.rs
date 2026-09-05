use std::net::SocketAddr;

use oregon_network::TransportConnection;
use oregon_protocol::{FRAME_HEADER_BYTES, FeatureSet, Hash256, Message, encode_message};

use crate::budget::PeerQueueBudget;
use crate::service::Registration;
use crate::{Direction, PeerError, QueueClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstablishedPeer {
    pub peer_id: PeerId,
    pub remote_addr: SocketAddr,
    pub direction: Direction,
    pub negotiated_version: u16,
    pub features: FeatureSet,
    pub remote_best_height: u64,
    pub remote_best_block_id: Hash256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    Requested,
    Network,
    Handshake,
    Replaced,
    QueueTimeout,
    Misbehavior,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    Established(EstablishedPeer),
    Message {
        peer_id: PeerId,
        message: Message,
    },
    Disconnected {
        peer_id: PeerId,
        reason: DisconnectReason,
    },
}

pub struct PeerSession<C: TransportConnection> {
    peer: EstablishedPeer,
    connection: C,
    budget: PeerQueueBudget,
    _registration: Registration,
}

impl<C: TransportConnection> PeerSession<C> {
    pub(crate) fn new(
        peer: EstablishedPeer,
        connection: C,
        budget: PeerQueueBudget,
        registration: Registration,
    ) -> Self {
        Self {
            peer,
            connection,
            budget,
            _registration: registration,
        }
    }

    pub fn peer(&self) -> &EstablishedPeer {
        &self.peer
    }

    pub fn remote_addr(&self) -> SocketAddr {
        self.connection.remote_addr()
    }

    pub async fn send(&mut self, message: &Message, class: QueueClass) -> Result<bool, PeerError> {
        let (_, payload) = encode_message(message)?;
        let bytes = FRAME_HEADER_BYTES
            .checked_add(payload.len())
            .ok_or(PeerError::QueueItemTooLarge)?;
        let Some(_permit) = self.budget.reserve(class, bytes).await? else {
            return Ok(false);
        };
        self.connection.write_message(message).await?;
        Ok(true)
    }

    pub async fn read_message(&mut self) -> Result<Message, PeerError> {
        self.connection
            .read_message()
            .await
            .map_err(PeerError::from)
    }

    pub async fn shutdown(&mut self) -> Result<(), PeerError> {
        self.connection.shutdown().await.map_err(PeerError::from)
    }
}
