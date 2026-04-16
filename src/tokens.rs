//! ERC-20 token definitions and network presets.
//!
//! The preset data is generated at build time from
//! `codegen/presets/tokens.json`, which keeps the public Rust API stable while
//! making token updates a data-only change.
//!
//! # Usage
//!
//! ```rust
//! use starkzap_rs::{Network, tokens::{self, mainnet, sepolia}};
//!
//! let usdc = mainnet::usdc();
//! let strk = sepolia::strk();
//! let mainnet_tokens = tokens::all(Network::Mainnet);
//! ```

use starknet::core::types::Felt;

use crate::network::Network;

/// A Starknet ERC-20 token definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Uppercase ticker symbol, e.g. `"USDC"`.
    pub symbol: String,
    /// Human-readable name, e.g. `"USD Coin"`.
    pub name: String,
    /// Number of decimal places (6 for USDC, 18 for ETH/STRK).
    pub decimals: u8,
    /// Contract address as a `Felt`.
    pub address: Felt,
}

impl Token {
    /// Construct a new [`Token`].
    pub fn new(symbol: &str, name: &str, decimals: u8, address: Felt) -> Self {
        Self {
            symbol: symbol.to_string(),
            name: name.to_string(),
            decimals,
            address,
        }
    }
}

/// All preset tokens for the given network.
pub fn all(network: Network) -> Vec<Token> {
    match network {
        Network::Mainnet => mainnet::all(),
        Network::Sepolia | Network::Devnet => sepolia::all(),
    }
}

/// Look up a token by symbol for the given network.
pub fn by_symbol(network: Network, symbol: &str) -> Option<Token> {
    match network {
        Network::Mainnet => mainnet::by_symbol(symbol),
        Network::Sepolia | Network::Devnet => sepolia::by_symbol(symbol),
    }
}

include!(concat!(env!("OUT_DIR"), "/tokens_generated.rs"));
