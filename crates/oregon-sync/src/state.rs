use oregon_peer::PeerId;
use oregon_primitives::{Block, Hash256};
use oregon_protocol::GetHeaders;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAction {
    SendGetHeaders {
        peer_id: PeerId,
        request: GetHeaders,
    },
    RequestBlock {
        peer_id: PeerId,
        block_id: Hash256,
    },
    SubmitBlock {
        source: PeerId,
        block: Block,
    },
    Stalled {
        block_id: Hash256,
    },
}
