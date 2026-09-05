#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Misbehavior {
    MalformedFrame,
    OversizedFrame,
    HandshakeViolation,
    InvalidResponse,
    UnsolicitedObject,
    RequestAbuse,
    SyncTimeout,
    InvalidHeader,
    InvalidBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreDecision {
    Continue,
    StopSync,
    Disconnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerFeedback {
    InvalidHeader,
    InvalidBlock,
    RequestAbuse,
}

impl PeerFeedback {
    pub const fn misbehavior(self) -> Misbehavior {
        match self {
            Self::InvalidHeader => Misbehavior::InvalidHeader,
            Self::InvalidBlock => Misbehavior::InvalidBlock,
            Self::RequestAbuse => Misbehavior::RequestAbuse,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeerScore {
    points: u16,
}

impl PeerScore {
    pub const fn points(&self) -> u16 {
        self.points
    }

    pub const fn points_for(&self, offense: Misbehavior) -> u16 {
        match offense {
            Misbehavior::MalformedFrame | Misbehavior::HandshakeViolation => 25,
            Misbehavior::InvalidResponse
            | Misbehavior::UnsolicitedObject
            | Misbehavior::RequestAbuse => 10,
            Misbehavior::SyncTimeout => 5,
            Misbehavior::InvalidHeader | Misbehavior::InvalidBlock => 50,
            Misbehavior::OversizedFrame => 0,
        }
    }

    pub fn apply(&mut self, offense: Misbehavior) -> ScoreDecision {
        if offense == Misbehavior::OversizedFrame {
            return ScoreDecision::Disconnect;
        }
        self.points = self.points.saturating_add(self.points_for(offense));
        self.decision()
    }

    pub const fn sync_eligible(&self) -> bool {
        self.points < 50
    }

    pub const fn disconnect_required(&self) -> bool {
        self.points >= 100
    }

    pub const fn decision(&self) -> ScoreDecision {
        if self.disconnect_required() {
            ScoreDecision::Disconnect
        } else if !self.sync_eligible() {
            ScoreDecision::StopSync
        } else {
            ScoreDecision::Continue
        }
    }
}
