use oregon_primitives::execution_envelope::ExecutionDomain;

use crate::EscrowTicketV1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ValidatedEscrowV1 {
    ticket: EscrowTicketV1,
    execution_domain: ExecutionDomain,
}

impl ValidatedEscrowV1 {
    pub(super) const fn new(ticket: EscrowTicketV1, execution_domain: ExecutionDomain) -> Self {
        Self {
            ticket,
            execution_domain,
        }
    }

    pub(super) const fn ticket(self) -> EscrowTicketV1 {
        self.ticket
    }

    pub(super) const fn execution_domain(self) -> ExecutionDomain {
        self.execution_domain
    }
}
