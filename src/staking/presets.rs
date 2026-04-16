//! Staking validator presets for mainnet and Sepolia.
//!
//! The preset data is generated at build time from
//! `codegen/presets/validators.json`, so validator updates stay data-driven
//! while preserving the public API shape of the SDK.

use starknet::core::types::Felt;

/// A staking validator (staker identity).
#[derive(Debug, Clone)]
pub struct Validator {
    /// Human-readable name, e.g. `"StarkWare"`.
    pub name: String,
    /// The staker (operator) address.
    pub staker_address: Felt,
}

impl Validator {
    pub fn new(name: &str, staker_address: Felt) -> Self {
        Self {
            name: name.to_string(),
            staker_address,
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/validators_generated.rs"));

/// Look up a mainnet validator by name (case-insensitive).
pub fn mainnet_validator(name: &str) -> Option<Validator> {
    mainnet_validators()
        .into_iter()
        .find(|v| v.name.eq_ignore_ascii_case(name))
}

/// Look up a Sepolia validator by name (case-insensitive).
pub fn sepolia_validator(name: &str) -> Option<Validator> {
    sepolia_validators()
        .into_iter()
        .find(|v| v.name.eq_ignore_ascii_case(name))
}
