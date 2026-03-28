//! Network configuration — Mainnet and Sepolia.

use starknet::core::types::Felt;

/// The Starknet network to connect to.
///
/// Mirrors starkzap's `network: "mainnet" | "sepolia" | "devnet"` preset system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// Starknet Mainnet (production — real funds).
    Mainnet,
    /// Starknet Sepolia testnet (staging — test tokens).
    Sepolia,
    /// Local starknet-devnet instance (development — no tokens needed).
    ///
    /// Defaults to `http://127.0.0.1:5050`. Override with a custom `rpc_url`
    /// in [`crate::sdk::StarkZapConfig`] if you use a different port.
    Devnet,
}

impl Network {
    /// Default public JSON-RPC endpoint for this network.
    ///
    /// - **Mainnet / Sepolia**: BlastAPI public endpoints (RPC v0.8).
    ///   For production traffic, supply your own Alchemy/Infura key via
    ///   [`crate::sdk::StarkZapConfig::with_rpc`].
    /// - **Devnet**: local starknet-devnet at `127.0.0.1:5050`.
    pub fn default_rpc_url(&self) -> &'static str {
        match self {
            Network::Mainnet => "https://starknet-mainnet.public.blastapi.io/rpc/v0_8",
            Network::Sepolia => "https://starknet-sepolia.public.blastapi.io/rpc/v0_8",
            Network::Devnet  => "http://127.0.0.1:5050/rpc",
        }
    }

    /// Chain ID felt used when building `SingleOwnerAccount`.
    pub fn chain_id(&self) -> Felt {
        match self {
            // "SN_MAIN"
            Network::Mainnet => Felt::from_hex_unchecked("0x534e5f4d41494e"),
            // "SN_SEPOLIA"
            Network::Sepolia => Felt::from_hex_unchecked("0x534e5f5345504f4c4941"),
            // "SN_GOERLI" — devnet uses this by default; override if your devnet differs
            Network::Devnet  => Felt::from_hex_unchecked("0x534e5f5345504f4c4941"),
        }
    }

    /// Returns `true` if this is mainnet.
    pub fn is_mainnet(&self) -> bool {
        matches!(self, Network::Mainnet)
    }

    /// Returns `true` if this is a local devnet instance.
    pub fn is_devnet(&self) -> bool {
        matches!(self, Network::Devnet)
    }

    /// AVNU paymaster base URL for this network.
    ///
    /// Devnet has no AVNU paymaster — [`FeeMode::Paymaster`] will error on devnet.
    pub fn avnu_base_url(&self) -> &'static str {
        match self {
            Network::Mainnet => "https://starknet.api.avnu.fi",
            Network::Sepolia => "https://sepolia.api.avnu.fi",
            Network::Devnet  => "http://localhost:0", // no paymaster on devnet
        }
    }

    /// Universal Deployer Contract (UDC) address — same across all networks.
    pub fn udc_address(&self) -> Felt {
        Felt::from_hex_unchecked(
            "0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
        )
    }

    /// Starknet native staking contract address.
    ///
    /// Staking is not available on devnet. Returns `Felt::ZERO` for devnet,
    /// which will produce a meaningful error when called against the RPC.
    pub fn staking_contract(&self) -> Felt {
        match self {
            Network::Mainnet => Felt::from_hex_unchecked(
                "0x0e8c7920d56e3cc753bab32bc7b01b4011c151c5e893db2a90f48d1e02bbaedb",
            ),
            Network::Sepolia => Felt::from_hex_unchecked(
                "0x07a1b1f4ee3e8efc02b4a0e3fe5a1ef33d3521c63b37e6a51588fd6c57c17db0",
            ),
            Network::Devnet  => Felt::ZERO,
        }
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Sepolia => write!(f, "sepolia"),
            Network::Devnet  => write!(f, "devnet"),
        }
    }
}