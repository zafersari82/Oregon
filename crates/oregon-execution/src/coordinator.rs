mod accounting;
mod calls;
mod effects;
mod host;
mod proposal;
mod settlement;
mod types;

use oregon_contract_state::StateSource;
use proposal::execute_transaction_v1;
use settlement::FundingSourceValidatorV1;

const _: () = {
    let _ = execute_transaction_v1::<
        dyn StateSource,
        dyn StateSource,
        dyn FundingSourceValidatorV1,
    >;
};

#[cfg(test)]
mod calls_tests;
#[cfg(test)]
mod domain_binding_tests;
#[cfg(test)]
mod escrow_tests;
#[cfg(test)]
mod funding_tests;
#[cfg(test)]
mod tests;
