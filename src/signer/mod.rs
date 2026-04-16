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

mod stark_signer;
pub use stark_signer::StarkSigner;

#[cfg(feature = "cartridge")]
pub(crate) mod cartridge_signer;
#[cfg(feature = "cartridge")]
pub use cartridge_signer::CartridgeSigner;

#[cfg(feature = "privy")]
mod privy_signer;
#[cfg(feature = "privy")]
pub use privy_signer::PrivySigner;

use starknet::{
    core::types::Felt,
    signers::{Signer, SignerInteractivityContext, VerifyingKey},
};

use crate::error::StarkzapError;

/// Runtime signer wrapper used by the wallet implementation.
#[derive(Debug, Clone)]
#[doc(hidden)]
pub enum AnySigner {
    Stark(StarkSigner),

    #[cfg(feature = "cartridge")]
    Cartridge(CartridgeSigner),

    #[cfg(feature = "privy")]
    Privy(PrivySigner),
}

impl AnySigner {
    pub fn known_address(&self) -> Option<Felt> {
        match self {
            Self::Stark(signer) => signer.address(),

            #[cfg(feature = "cartridge")]
            Self::Cartridge(signer) => Some(signer.address()),

            #[cfg(feature = "privy")]
            Self::Privy(signer) => signer.address(),
        }
    }
}

#[async_trait::async_trait]
impl Signer for AnySigner {
    type GetPublicKeyError = StarkzapError;
    type SignError = StarkzapError;

    async fn get_public_key(&self) -> std::result::Result<VerifyingKey, Self::GetPublicKeyError> {
        match self {
            Self::Stark(signer) => signer.get_public_key().await,

            #[cfg(feature = "cartridge")]
            Self::Cartridge(signer) => signer.get_public_key().await,

            #[cfg(feature = "privy")]
            Self::Privy(signer) => signer.get_public_key().await,
        }
    }

    async fn sign_hash(
        &self,
        hash: &Felt,
    ) -> std::result::Result<starknet::core::crypto::Signature, Self::SignError> {
        match self {
            Self::Stark(signer) => signer.sign_hash(hash).await,

            #[cfg(feature = "cartridge")]
            Self::Cartridge(signer) => signer.sign_hash(hash).await,

            #[cfg(feature = "privy")]
            Self::Privy(signer) => Signer::sign_hash(signer, hash).await,
        }
    }

    fn is_interactive(&self, context: SignerInteractivityContext<'_>) -> bool {
        match self {
            Self::Stark(signer) => signer.is_interactive(context),

            #[cfg(feature = "cartridge")]
            Self::Cartridge(signer) => signer.is_interactive(context),

            #[cfg(feature = "privy")]
            Self::Privy(signer) => signer.is_interactive(context),
        }
    }
}
