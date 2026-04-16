//! Raw private-key signer — mirrors `new StarkSigner(privateKey)` from the TS SDK.

use starknet::{
    core::types::Felt,
    signers::{LocalWallet, Signer, SignerInteractivityContext, SigningKey, VerifyingKey},
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
/// ```rust,no_run
/// use starkzap_rs::signer::StarkSigner;
///
/// let signer = StarkSigner::new(
///     "0x..your_private_key..",
///     "0x..your_address..",
/// )?;
/// # Ok::<(), starkzap_rs::StarkzapError>(())
/// ```

#[derive(Debug, Clone)]
pub struct StarkSigner {
    pub(crate) wallet: LocalWallet,
    pub(crate) address: Option<Felt>,
    pub(crate) public_key: Felt,
}

impl StarkSigner {
    /// Construct from a 0x-prefixed hex private key and infer the counterfactual
    /// address later from an account preset.
    pub fn from_private_key(private_key_hex: &str) -> Result<Self> {
        let pk_felt = Felt::from_hex(private_key_hex)
            .map_err(|_| StarkzapError::InvalidPrivateKey)?;

        let signing_key = SigningKey::from_secret_scalar(pk_felt);
        let public_key = signing_key.verifying_key().scalar();
        let wallet = LocalWallet::from(signing_key);

        Ok(Self {
            wallet,
            address: None,
            public_key,
        })
    }

    /// Construct from a 0x-prefixed hex private key and the account address.
    ///
    /// # Arguments
    ///
    /// * `private_key_hex` — e.g. `"0xabc123..."`
    /// * `address_hex` — the deployed account address, e.g. `"0x1234..."`
    pub fn new(private_key_hex: &str, address_hex: &str) -> Result<Self> {
        let address = Felt::from_hex(address_hex)
            .map_err(|_| StarkzapError::InvalidAddress(address_hex.to_string()))?;

        let mut signer = Self::from_private_key(private_key_hex)?;
        signer.address = Some(address);
        Ok(signer)
    }

    /// Attach a known on-chain address to the signer.
    pub fn with_address(mut self, address_hex: &str) -> Result<Self> {
        let address = Felt::from_hex(address_hex)
            .map_err(|_| StarkzapError::InvalidAddress(address_hex.to_string()))?;
        self.address = Some(address);
        Ok(self)
    }

    /// Account address.
    pub fn address(&self) -> Option<Felt> {
        self.address
    }

    /// Stark public key derived from the private key.
    pub fn public_key(&self) -> Felt {
        self.public_key
    }

    /// Access the underlying [`LocalWallet`] (needed to build [`SingleOwnerAccount`]).
    pub fn local_wallet(&self) -> &LocalWallet {
        &self.wallet
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl Signer for StarkSigner {
    type GetPublicKeyError = StarkzapError;
    type SignError = StarkzapError;

    async fn get_public_key(&self) -> std::result::Result<VerifyingKey, Self::GetPublicKeyError> {
        Ok(VerifyingKey::from_scalar(self.public_key))
    }

    async fn sign_hash(
        &self,
        hash: &Felt,
    ) -> std::result::Result<starknet::core::crypto::Signature, Self::SignError> {
        self.wallet
            .sign_hash(hash)
            .await
            .map_err(|e| StarkzapError::Signer(e.to_string()))
    }

    fn is_interactive(&self, _context: SignerInteractivityContext<'_>) -> bool {
        false
    }
}
