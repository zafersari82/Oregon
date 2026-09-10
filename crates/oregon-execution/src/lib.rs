#![forbid(unsafe_code)]

#[cfg(test)]
mod coordinator;
mod fees;
mod journal;
mod weight;

pub use fees::{
    EscrowBookV1, EscrowTicketV1, FeeError, FeeTermsV1, FundingCapabilityV1, SettlementResultV1,
};
pub use journal::{
    ExecutionJournalV1, JournalContextV1, JournalDomainRootsV1, JournalError, JournalIntentV1,
    JournalLimitsV1, JournalResultV1,
};
pub use weight::{MeterScheduleV1, ResourceDomain, ResourceError, WeightMeter, WeightRatio};

#[cfg(test)]
mod architecture_tests {
    #[test]
    fn coordinator_is_private_production_module_without_dead_code_suppression() {
        let lib_source = include_str!("lib.rs");
        assert!(lib_source.contains("#![forbid(unsafe_code)]\n\nmod coordinator;"));
        assert!(!lib_source.contains("#[cfg(test)]\nmod coordinator;"));
        assert!(!lib_source.contains("pub mod coordinator;"));

        let coordinator_source = include_str!("coordinator.rs");
        assert!(!coordinator_source.contains("allow(dead_code)"));
    }
}
