//! Starknet native staking integration.
//!
//! Provides delegation pool management:
//! enter, add, exit (intent + action), reward claiming, and pool discovery.
//!
//! All staking methods are implemented directly on [`crate::wallet::Wallet`].
//!
//! # Example
//!
//! ```rust,no_run
//! use starkzap_rs::{
//!     Amount, StarkZap, StarkZapConfig, Network, OnboardConfig,
//!     paymaster::FeeMode,
//!     signer::StarkSigner,
//!     staking::presets::mainnet_validators,
//!     tokens::mainnet,
//! };
//!
//! # async fn example() -> starkzap_rs::error::Result<()> {
//! let sdk = StarkZap::new(StarkZapConfig::mainnet());
//! let signer = StarkSigner::new(&std::env::var("PRIVATE_KEY").unwrap(), &std::env::var("ADDRESS").unwrap())?;
//! let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
//!
//! let strk = mainnet::strk();
//! let validators = mainnet_validators();
//! let pool = validators[0].pool_address;
//!
//! // Stake 100 STRK
//! let amount = Amount::parse("100", &strk)?;
//! let tx = wallet.enter_pool(&strk, pool, amount, wallet.address(), FeeMode::UserPays).await?;
//! tx.wait().await?;
//!
//! // Check position
//! let pos = wallet.get_pool_position(pool, &strk).await?;
//! println!("Staked: {}", pos.staked);
//! println!("Rewards: {}", pos.rewards);
//!
//! // Claim rewards
//! let tx = wallet.claim_rewards(pool, FeeMode::UserPays).await?;
//! tx.wait().await?;
//! # Ok(())
//! # }
//! ```

pub mod discovery;
pub mod pool;
pub mod presets;
pub mod rewards;

pub use presets::{mainnet_validators, sepolia_validators, Validator};
pub use rewards::PoolPosition;
pub use discovery::DiscoveredPool;