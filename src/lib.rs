//! # starkzap-rs
//!
//! A Rust SDK for seamless Starknet wallet integration — the faithful Rust mirror
//! of the [`starkzap`](https://github.com/keep-starknet-strange/starkzap) TypeScript SDK.
//!
//! Built for the Starknet community.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use starkzap_rs::{
//!     StarkZap, StarkZapConfig, OnboardConfig,
//!     signer::StarkSigner,
//!     tokens::sepolia,
//!     Amount,
//!     wallet::Recipient,
//! };
//!
//! #[tokio::main]
//! async fn main() -> starkzap_rs::error::Result<()> {
//!     let sdk = StarkZap::new(StarkZapConfig::sepolia());
//!
//!     let signer = StarkSigner::new(
//!         &std::env::var("PRIVATE_KEY").unwrap(),
//!         &std::env::var("ACCOUNT_ADDRESS").unwrap(),
//!     )?;
//!
//!     let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
//!
//!     let strk = sepolia::strk();
//!     let balance = wallet.balance_of(&strk).await?;
//!     println!("Balance: {}", balance);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! | Feature | Description |
//! |---|---|
//! | *(default)* | Core SDK: StarkSigner, tokens, transfers, staking |
//! | `privy` | Privy server-side signer |
//! | `cartridge` | Cartridge session-key signer |
//! | `full` | All optional signers |
//! | `wasm` | WebAssembly target support |
//!
//! ## Targets
//!
//! - **Server / CLI** (tokio) — always available, no feature flags needed
//! - **WASM / browser** — compile with `--features wasm --target wasm32-unknown-unknown`

// ── Modules ───────────────────────────────────────────────────────────────────

pub mod amount;
pub mod account;
pub mod error;
pub mod network;
pub mod paymaster;
pub mod sdk;
pub mod signer;
pub mod staking;
pub mod tokens;
pub mod tx;
pub mod wallet;

// ── Re-exports: primary public API ───────────────────────────────────────────

pub use amount::Amount;
pub use account::AccountPreset;
pub use error::{Result, StarkzapError};
pub use network::Network;
pub use sdk::{OnboardConfig, StarkZap, StarkZapConfig};
pub use tx::{Tx, TxStatus};
pub use wallet::{
    DeployMode, DeployPolicy, EnsureReadyOptions, ExecuteOptions, PreflightOptions,
    PreflightResult, ProgressEvent, ProgressStep, Recipient, Wallet,
};
