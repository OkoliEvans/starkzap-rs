//! Raw private-key signer — mirrors `new StarkSigner(privateKey)` from the TS SDK.

use starknet::{
    core::types::Felt,
    signers::{LocalWallet, SigningKey},
};

use crate::error::{Result, StarkzapError};

/// A signer backed by a raw private key.
///
/// The private key is held in memory as a `SigningKey` and used directly for
/// transaction signing. Suitable for server-side scripts, CI/CD pipelines, and
/// developer demos.
///
/// # Security
///
/// Never hard-code private keys. Load them from environment variables or a
/// secrets manager.
///
/// # Example
///
/// ```rust
/// use starkzap_rs::signer::StarkSigner;
///
/// // Load from environment
/// let pk = std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set");
/// let signer = StarkSigner::from_hex(&pk)?;
/// # Ok::<(), starkzap_rs::StarkzapError>(())
/// ```
#[derive(Debug, Clone)]
pub struct StarkSigner {
    pub(crate) wallet: LocalWallet,
    pub(crate) address: Felt,
}

impl StarkSigner {
    /// Construct from a 0x-prefixed hex private key and the account address.
    ///
    /// # Arguments
    ///
    /// * `private_key_hex` — e.g. `"0xabc123..."`
    /// * `address_hex` — the deployed account address, e.g. `"0x1234..."`
    pub fn new(private_key_hex: &str, address_hex: &str) -> Result<Self> {
        let pk_felt = Felt::from_hex(private_key_hex)
            .map_err(|_| StarkzapError::InvalidPrivateKey)?;

        let address = Felt::from_hex(address_hex)
            .map_err(|_| StarkzapError::InvalidAddress(address_hex.to_string()))?;

        let signing_key = SigningKey::from_secret_scalar(pk_felt);
        let wallet = LocalWallet::from(signing_key);

        Ok(Self { wallet, address })
    }

    /// Account address.
    pub fn address(&self) -> Felt {
        self.address
    }

    /// Access the underlying [`LocalWallet`] (needed to build [`SingleOwnerAccount`]).
    pub fn local_wallet(&self) -> &LocalWallet {
        &self.wallet
    }
}