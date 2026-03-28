//! Signer backends.
//!
//! All signers implement the [`StarkzapSigner`] trait, which wraps a
//! `starknet::signers::Signer` and supplies the account's on-chain address.
//!
//! | Signer | Description | Feature flag |
//! |---|---|---|
//! | [`StarkSigner`] | Raw private key | *(always available)* |
//! | [`CartridgeSigner`] | Pre-issued session key | `cartridge` |
//! | [`PrivySigner`] | Privy server REST API | `privy` |

mod stark;
pub use stark::StarkSigner;

#[cfg(feature = "cartridge")]
mod cartridge;
#[cfg(feature = "cartridge")]
pub use cartridge::CartridgeSigner;

#[cfg(feature = "privy")]
mod privy;
#[cfg(feature = "privy")]
pub use privy::PrivySigner;