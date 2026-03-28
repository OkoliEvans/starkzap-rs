use thiserror::Error;

/// Unified error type for all starkzap-rs operations.
#[derive(Debug, Error)]
pub enum StarkzapError {
    // ── Provider / RPC ────────────────────────────────────────────────────────
    #[error("RPC provider error: {0}")]
    Provider(#[from] starknet::providers::ProviderError),

    #[error("Account error: {0}")]
    Account(String),

    // ── Signing ───────────────────────────────────────────────────────────────
    #[error("Signer error: {0}")]
    Signer(String),

    #[error("Invalid private key: must be a 0x-prefixed hex felt")]
    InvalidPrivateKey,

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    // ── Transactions ──────────────────────────────────────────────────────────
    #[error("Transaction rejected: {reason}")]
    TransactionRejected { reason: String },

    #[error("Transaction reverted: {reason}")]
    TransactionReverted { reason: String },

    #[error("Transaction wait timed out after {attempts} attempts")]
    WaitTimeout { attempts: u32 },

    // ── Amounts ───────────────────────────────────────────────────────────────
    #[error("Amount parse error: '{input}' is not a valid decimal string")]
    AmountParse { input: String },

    #[error("Amount overflow: value exceeds u128::MAX")]
    AmountOverflow,

    // ── Tokens ────────────────────────────────────────────────────────────────
    #[error("Unknown token: {symbol}")]
    UnknownToken { symbol: String },

    // ── Paymaster ─────────────────────────────────────────────────────────────
    #[error("Paymaster request failed ({status}): {body}")]
    PaymasterRequest { status: u16, body: String },

    #[error("Paymaster response missing field: {field}")]
    PaymasterMalformed { field: String },

    // ── Privy (feature = "privy") ─────────────────────────────────────────────
    #[cfg(feature = "privy")]
    #[error("Privy API error ({status}): {body}")]
    PrivyApi { status: u16, body: String },

    #[cfg(feature = "privy")]
    #[error("Privy signing failed: {0}")]
    PrivySigning(String),

    // ── Staking ───────────────────────────────────────────────────────────────
    #[error("Staking error: {0}")]
    Staking(String),

    #[error("No pools found for staker: {address}")]
    NoPoolsFound { address: String },

    // ── Deployment ────────────────────────────────────────────────────────────
    #[error("Account deploy failed: {0}")]
    DeployFailed(String),

    #[error("Account is not deployed and deploy was not requested")]
    NotDeployed,

    // ── HTTP ──────────────────────────────────────────────────────────────────
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    // ── Serialization ─────────────────────────────────────────────────────────
    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    // ── Misc ──────────────────────────────────────────────────────────────────
    #[error("Hex decode error: {0}")]
    HexDecode(#[from] hex::FromHexError),

    #[error("{0}")]
    Other(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, StarkzapError>;