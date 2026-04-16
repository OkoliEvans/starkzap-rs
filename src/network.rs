//! Network configuration — Mainnet, Sepolia, and Devnet.

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
    /// Defaults to `http://127.0.0.1:5050`. Override via
    /// [`crate::sdk::StarkZapConfig::with_rpc`] if you use a different port.
    Devnet,
}

impl Network {
    /// Default public JSON-RPC endpoint for this network.
    ///
    /// For production always pass your own URL via
    /// [`crate::sdk::StarkZapConfig::with_rpc`].
    pub fn default_rpc_url(&self) -> &'static str {
        match self {
            Network::Mainnet => "https://starknet.drpc.org",
            Network::Sepolia => "https://starknet-sepolia.drpc.org",
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
            // Devnet mirrors Sepolia by default
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

    /// AVNU paymaster JSON-RPC base URL.
    ///
    /// This mirrors StarkZap TS / `starknet.js` `PaymasterRpc`.
    pub fn avnu_paymaster_url(&self) -> &'static str {
        match self {
            Network::Mainnet => "https://starknet.paymaster.avnu.fi",
            Network::Sepolia => "https://sepolia.paymaster.avnu.fi",
            Network::Devnet  => "http://localhost:0", // intentionally invalid
        }
    }

    /// AVNU swap/exchange REST API base URL.
    pub fn avnu_base_url(&self) -> &'static str {
        match self {
            Network::Mainnet => "https://starknet.api.avnu.fi",
            Network::Sepolia => "https://sepolia.api.avnu.fi",
            Network::Devnet  => "http://localhost:0",
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
    /// Returns `Felt::ZERO` for devnet — staking is not available locally.
    pub fn staking_contract(&self) -> Felt {
        match self {
            Network::Mainnet => Felt::from_hex_unchecked(
                "0x00ca1702e64c81d9a07b86bd2c540188d92a2c73cf5cc0e508d949015e7e84a7",
            ),
            Network::Sepolia => Felt::from_hex_unchecked(
                "0x03745ab04a431fc02871a139be6b93d9260b0ff3e779ad9c8b377183b23109f1",
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
