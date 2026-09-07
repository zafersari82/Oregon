//! Toolchain smoke checks only; these are not reserve-conservation proofs.

#![forbid(unsafe_code)]

#[kani::proof]
#[kani::solver(cadical)]
fn bootstrap_positive() {
    let value: u8 = kani::any();
    let widened = u16::from(value);
    assert!(widened + 1 > widened, "bootstrap widening preserves increment");
    kani::cover!(value == u8::MAX, "maximum input is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
fn bootstrap_negative() {
    let value: u8 = kani::any();
    assert!(
        value.wrapping_add(1) > value,
        "bootstrap wrapping increment must produce a counterexample"
    );
}
