use thiserror::Error;

const VERSION_V1: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ResourceError {
    #[error("unsupported resource schedule version {0}")]
    UnsupportedVersion(u16),
    #[error("resource ratio must have positive numerator and denominator")]
    InvalidRatio,
    #[error("resource meter budget is invalid")]
    InvalidBudget,
    #[error("normalized weight does not fit in u64")]
    WeightOverflow,
    #[error("resource meter budget exhausted")]
    WeightExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightRatio {
    numerator: u64,
    denominator: u64,
}

impl WeightRatio {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, ResourceError> {
        if numerator == 0 || denominator == 0 {
            return Err(ResourceError::InvalidRatio);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub fn normalized_weight(self, units: u64) -> Result<u64, ResourceError> {
        let product = u128::from(units) * u128::from(self.numerator);
        let quotient = product / u128::from(self.denominator);
        let remainder = product % u128::from(self.denominator);
        let rounded = quotient + u128::from(remainder != 0);
        u64::try_from(rounded).map_err(|_| ResourceError::WeightOverflow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResourceDomain {
    Native,
    Evm,
    Wasm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeterScheduleV1 {
    native: WeightRatio,
    evm: WeightRatio,
    wasm: WeightRatio,
}

impl MeterScheduleV1 {
    pub fn new(
        version: u16,
        native: WeightRatio,
        evm: WeightRatio,
        wasm: WeightRatio,
    ) -> Result<Self, ResourceError> {
        if version != VERSION_V1 {
            return Err(ResourceError::UnsupportedVersion(version));
        }
        Ok(Self { native, evm, wasm })
    }

    const fn ratio(self, domain: ResourceDomain) -> WeightRatio {
        match domain {
            ResourceDomain::Native => self.native,
            ResourceDomain::Evm => self.evm,
            ResourceDomain::Wasm => self.wasm,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct WeightMeter {
    schedule: MeterScheduleV1,
    counters: [u64; 3],
    consumed: u64,
    max_weight: u64,
    exhausted: bool,
}

impl WeightMeter {
    pub fn new(
        schedule: MeterScheduleV1,
        max_weight: u64,
        intrinsic_weight: u64,
    ) -> Result<Self, ResourceError> {
        if max_weight == 0 || intrinsic_weight > max_weight {
            return Err(ResourceError::InvalidBudget);
        }
        Ok(Self {
            schedule,
            counters: [0; 3],
            consumed: intrinsic_weight,
            max_weight,
            exhausted: false,
        })
    }

    pub fn charge(&mut self, domain: ResourceDomain, units: u64) -> Result<(), ResourceError> {
        if self.exhausted {
            return Err(ResourceError::WeightExhausted);
        }
        if units == 0 {
            return Ok(());
        }
        let index = domain as usize;
        let Some(next_counter) = self.counters[index].checked_add(units) else {
            return self.exhaust();
        };
        let old_weight = self
            .schedule
            .ratio(domain)
            .normalized_weight(self.counters[index]);
        let new_weight = self.schedule.ratio(domain).normalized_weight(next_counter);
        let (Ok(old_weight), Ok(new_weight)) = (old_weight, new_weight) else {
            return self.exhaust();
        };
        let Some(delta) = new_weight.checked_sub(old_weight) else {
            return self.exhaust();
        };
        if delta > self.max_weight - self.consumed {
            return self.exhaust();
        }
        self.counters[index] = next_counter;
        self.consumed += delta;
        Ok(())
    }

    pub fn charge_common(&mut self, weight: u64) -> Result<(), ResourceError> {
        if self.exhausted {
            return Err(ResourceError::WeightExhausted);
        }
        if weight > self.max_weight - self.consumed {
            return self.exhaust();
        }
        self.consumed += weight;
        Ok(())
    }

    fn exhaust(&mut self) -> Result<(), ResourceError> {
        self.consumed = self.max_weight;
        self.exhausted = true;
        Err(ResourceError::WeightExhausted)
    }

    pub const fn consumed(&self) -> u64 {
        self.consumed
    }

    pub const fn remaining(&self) -> u64 {
        self.max_weight - self.consumed
    }

    pub const fn max_weight(&self) -> u64 {
        self.max_weight
    }

    pub const fn is_exhausted(&self) -> bool {
        self.exhausted
    }
}
