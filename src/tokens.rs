//! ERC-20 token definitions and network presets.
//!
//! Mirrors the `mainnetTokens` / `sepoliaTokens` presets from the TypeScript SDK.
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

// ── Mainnet presets ───────────────────────────────────────────────────────────

/// Mainnet token presets.
///
/// Addresses verified against the official Starknet token list.
pub mod mainnet {
    use super::*;

    /// USD Coin (6 decimals)
    pub fn usdc() -> Token {
        Token::new(
            "USDC",
            "USDC",
            6,
            Felt::from_hex_unchecked(
                "0x033068f6539f8e6e6b131e6b2b814e6c34a5224bc66947c47dab9dfee93b35fb",
            ),
        )
    }

    /// Bridged USD Coin — USDC.e (6 decimals)
    pub fn usdc_e() -> Token {
        Token::new(
            "USDC.e",
            "Bridged USDC",
            6,
            Felt::from_hex_unchecked(
                "0x053c91253bc9682c04929ca02ed00b3e423f6710d2ee7e0d5ebb06f3ecf368a8",
            ),
        )
    }

    /// Tether USD (6 decimals)
    pub fn usdt() -> Token {
        Token::new(
            "USDT",
            "Tether USD",
            6,
            Felt::from_hex_unchecked(
                "0x068f5c6a61780768455de69077e07e89787839bf8166decfbf92b645209c0fb8",
            ),
        )
    }

    /// Starknet Token (18 decimals)
    pub fn strk() -> Token {
        Token::new(
            "STRK",
            "Starknet Token",
            18,
            Felt::from_hex_unchecked(
                "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d",
            ),
        )
    }

    /// Wrapped Ether (18 decimals)
    pub fn eth() -> Token {
        Token::new(
            "ETH",
            "Wrapped Ether",
            18,
            Felt::from_hex_unchecked(
                "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
            ),
        )
    }

    /// Wrapped Bitcoin (8 decimals)
    pub fn wbtc() -> Token {
        Token::new(
            "WBTC",
            "Wrapped Bitcoin",
            8,
            Felt::from_hex_unchecked(
                "0x03fe2b97c1fd336e750087d68b9b867997fd64a2661ff3ca5a7c771641e8e7ac",
            ),
        )
    }

    /// Wrapped stETH — liquid staking ETH (18 decimals)
    pub fn wsteth() -> Token {
        Token::new(
            "wstETH",
            "Wrapped stETH",
            18,
            Felt::from_hex_unchecked(
                "0x042b8f0484674ca266ac5d08e4ac6a3fe65bd3129795def2dca5c34ecc5f96d2",
            ),
        )
    }

    /// All mainnet tokens as a `Vec`.
    pub fn all() -> Vec<Token> {
        vec![usdc(), usdc_e(), usdt(), strk(), eth(), wbtc(), wsteth()]
    }

    /// Look up a token by symbol (case-insensitive).
    pub fn by_symbol(symbol: &str) -> Option<Token> {
        all().into_iter().find(|t| t.symbol.eq_ignore_ascii_case(symbol))
    }
}

// ── Sepolia presets ───────────────────────────────────────────────────────────

/// Sepolia testnet token presets.
pub mod sepolia {
    use super::*;

    /// USD Coin on Sepolia (6 decimals)
    pub fn usdc() -> Token {
        Token::new(
            "USDC",
            "USDC",
            6,
            Felt::from_hex_unchecked(
                "0x0512feac6339ff7889822cb5aa2a86c848e9d392bb0e3e237c008674feed8343",
            ),
        )
    }

    /// Starknet Token on Sepolia (18 decimals)
    pub fn strk() -> Token {
        Token::new(
            "STRK",
            "Starknet Token",
            18,
            Felt::from_hex_unchecked(
                "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d",
            ),
        )
    }

    /// Wrapped Ether on Sepolia (18 decimals)
    pub fn eth() -> Token {
        Token::new(
            "ETH",
            "Wrapped Ether",
            18,
            Felt::from_hex_unchecked(
                "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
            ),
        )
    }

    /// Bridged USDC on Sepolia (6 decimals)
    pub fn usdc_e() -> Token {
        Token::new(
            "USDC.e",
            "USDC.e",
            6,
            Felt::from_hex_unchecked(
                "0x053b40a647cedfca6ca84f542a0fe36736031905a9639a7f19a3c1e66bfd5080",
            ),
        )
    }

    /// Wrapped stETH on Sepolia (18 decimals)
    pub fn wsteth() -> Token {
        Token::new(
            "wstETH",
            "Wrapped liquid staked Ether 2.0",
            18,
            Felt::from_hex_unchecked(
                "0x030de54c07e57818ae4a1210f2a3018a0b9521b8f8ae5206605684741650ac25",
            ),
        )
    }

    /// All Sepolia tokens as a `Vec`.
    pub fn all() -> Vec<Token> {
        vec![usdc(), usdc_e(), strk(), eth(), wsteth()]
    }

    /// Look up a token by symbol (case-insensitive).
    pub fn by_symbol(symbol: &str) -> Option<Token> {
        all().into_iter().find(|t| t.symbol.eq_ignore_ascii_case(symbol))
    }
}
