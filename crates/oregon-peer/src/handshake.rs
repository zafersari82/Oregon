use oregon_network::TransportConnection;
use oregon_protocol::{Hello, HelloAck, Message, Negotiated, negotiate};
use tokio::time::timeout;

use crate::{Direction, HANDSHAKE_TIMEOUT, PeerError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    Connected,
    HelloSent,
    Negotiated,
    AckSent,
    Established,
}

#[derive(Debug)]
pub(crate) struct HandshakeMachine {
    local: Hello,
    remote: Option<Hello>,
    negotiated: Option<Negotiated>,
    state: HandshakeState,
}

impl HandshakeMachine {
    pub(crate) fn new(local: Hello) -> Self {
        Self {
            local,
            remote: None,
            negotiated: None,
            state: HandshakeState::Connected,
        }
    }

    pub(crate) fn state(&self) -> HandshakeState {
        self.state
    }

    pub(crate) fn start(&mut self) -> Result<Message, PeerError> {
        if self.state != HandshakeState::Connected {
            return Err(PeerError::HandshakeViolation("Hello sent twice"));
        }
        self.state = HandshakeState::HelloSent;
        Ok(Message::Hello(self.local.clone()))
    }

    pub(crate) fn on_message(&mut self, message: Message) -> Result<Option<Message>, PeerError> {
        match message {
            Message::Ping(nonce) if self.state != HandshakeState::Established => {
                Ok(Some(Message::Pong(nonce)))
            }
            Message::Pong(_) if self.state != HandshakeState::Established => Ok(None),
            Message::Hello(remote) if self.state == HandshakeState::HelloSent => {
                if remote.chain_id != self.local.chain_id {
                    return Err(PeerError::WrongChain);
                }
                if remote.instance_nonce == self.local.instance_nonce {
                    return Err(PeerError::SelfPeer);
                }
                let negotiated = negotiate(&self.local, &remote)?;
                self.remote = Some(remote.clone());
                self.negotiated = Some(negotiated);
                self.state = HandshakeState::Negotiated;
                let ack = HelloAck {
                    selected_protocol_version: negotiated.protocol_version,
                    enabled_features: negotiated.features,
                    remote_nonce_echo: remote.instance_nonce,
                };
                self.state = HandshakeState::AckSent;
                Ok(Some(Message::HelloAck(ack)))
            }
            Message::HelloAck(ack) if self.state == HandshakeState::AckSent => {
                let negotiated = self.negotiated.ok_or(PeerError::HandshakeViolation(
                    "HelloAck received before negotiation",
                ))?;
                if ack.selected_protocol_version != negotiated.protocol_version
                    || ack.enabled_features != negotiated.features
                    || ack.remote_nonce_echo != self.local.instance_nonce
                {
                    return Err(PeerError::AckMismatch);
                }
                self.state = HandshakeState::Established;
                Ok(None)
            }
            Message::Hello(_) => Err(PeerError::HandshakeViolation("unexpected Hello")),
            Message::HelloAck(_) => Err(PeerError::HandshakeViolation("unexpected HelloAck")),
            _ if self.state != HandshakeState::Established => Err(PeerError::HandshakeViolation(
                "application message received before Established",
            )),
            _ => Err(PeerError::HandshakeViolation(
                "handshake machine used after Established",
            )),
        }
    }

    pub(crate) fn remote(&self) -> Option<&Hello> {
        self.remote.as_ref()
    }

    pub(crate) fn negotiated(&self) -> Option<Negotiated> {
        self.negotiated
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HandshakeResult {
    pub(crate) remote: Hello,
    pub(crate) negotiated: Negotiated,
}

pub(crate) async fn perform_handshake<C: TransportConnection>(
    connection: &mut C,
    local: Hello,
) -> Result<HandshakeResult, PeerError> {
    let future = async {
        let mut machine = HandshakeMachine::new(local);
        let hello = machine.start()?;
        connection.write_message(&hello).await?;

        while machine.state() != HandshakeState::Established {
            let incoming = connection.read_message().await?;
            if let Some(response) = machine.on_message(incoming)? {
                connection.write_message(&response).await?;
            }
        }

        Ok(HandshakeResult {
            remote: machine
                .remote()
                .cloned()
                .ok_or(PeerError::HandshakeViolation(
                    "Established without remote Hello",
                ))?,
            negotiated: machine.negotiated().ok_or(PeerError::HandshakeViolation(
                "Established without negotiation",
            ))?,
        })
    };

    timeout(HANDSHAKE_TIMEOUT, future)
        .await
        .map_err(|_| PeerError::HandshakeTimeout)?
}

pub fn generate_process_nonce() -> Result<[u8; 16], PeerError> {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| PeerError::Entropy)?;
    Ok(nonce)
}

pub fn preferred_direction(
    local_nonce: [u8; 16],
    remote_nonce: [u8; 16],
) -> Result<Direction, PeerError> {
    if local_nonce == remote_nonce {
        return Err(PeerError::SelfPeer);
    }
    Ok(if local_nonce < remote_nonce {
        Direction::Outbound
    } else {
        Direction::Inbound
    })
}
