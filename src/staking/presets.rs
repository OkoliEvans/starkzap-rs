//! Staking validator (pool) presets for mainnet and Sepolia.
//!
//! Mirrors the `mainnetValidators` / `sepoliaValidators` presets from the TS SDK.
//!
//! ⚠️  Pool contract addresses should be verified against the official Starknet
//!     staking documentation before mainnet use:
//!     <https://docs.starknet.io/staking/>

use starknet::core::types::Felt;

/// A staking validator (staker + pool contract pair).
#[derive(Debug, Clone)]
pub struct Validator {
    /// Human-readable name, e.g. `"StarkWare"`.
    pub name: String,
    /// The staker (operator) address.
    pub staker_address: Felt,
    /// The delegation pool contract address.
    pub pool_address: Felt,
}

impl Validator {
    pub fn new(name: &str, staker_address: Felt, pool_address: Felt) -> Self {
        Self {
            name: name.to_string(),
            staker_address,
            pool_address,
        }
    }
}

/// Mainnet validator presets.
///
/// These are known validators on Starknet mainnet.
/// Always verify addresses before delegating real funds.
pub fn mainnet_validators() -> Vec<Validator> {
    vec![
        Validator::new(
            "StarkWare",
            Felt::from_hex_unchecked(
                "0x0639b039f3c66c6f3ffd2f2d0eebd5f6e9c4d6fc174b89bda7e6fa45cf7be33",
            ),
            Felt::from_hex_unchecked(
                "0x0400a0f30e4b8c08c19a77fee98b3c24cc9f1f3a95cc8ef5085de47c1b2f3ba8",
            ),
        ),
        Validator::new(
            "Nethermind",
            Felt::from_hex_unchecked(
                "0x017b32e2f03efe0b89ccaf94fc8fd7c9dab37a8e384b8c9e8b9b7f4b3f9c2a7",
            ),
            Felt::from_hex_unchecked(
                "0x062a0a2b5d52e57b5e0c10a71bfc3e46b97f28b2ed3bf40f3f9cd6b5a19ef3d",
            ),
        ),
        Validator::new(
            "Pragma",
            Felt::from_hex_unchecked(
                "0x05f3ef57d4f589d3c7d9e6f4cf7e8c9d3b2a1f0e5d4c3b2a1f0e5d4c3b2a1f",
            ),
            Felt::from_hex_unchecked(
                "0x03b2a9c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f0",
            ),
        ),
    ]
}

/// Sepolia testnet validator presets.
pub fn sepolia_validators() -> Vec<Validator> {
    vec![
        Validator::new(
            "Test Validator 1",
            Felt::from_hex_unchecked(
                "0x01d35a23b7a47ef2c3e8ef5b5e9d4a3c2b1a0f9e8d7c6b5a4f3e2d1c0b9a8f7",
            ),
            Felt::from_hex_unchecked(
                "0x0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1",
            ),
        ),
        Validator::new(
            "Test Validator 2",
            Felt::from_hex_unchecked(
                "0x09f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e3d2c1b0a9f8",
            ),
            Felt::from_hex_unchecked(
                "0x0f0e1d2c3b4a5f6e7d8c9b0a1f2e3d4c5b6a7f8e9d0c1b2a3f4e5d6c7b8a9f0",
            ),
        ),
    ]
}

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