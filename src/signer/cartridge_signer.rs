//! Cartridge session-key signer (feature = `"cartridge"`).
//!
//! ## How Cartridge works
//!
//! Cartridge Controller is a browser-native smart wallet that uses WebAuthn
//! (passkeys / biometrics) for the primary auth flow — this cannot be replicated
//! server-side. However, once a user has authenticated in the browser, Cartridge
//! issues a **session key**: a temporary key pair authorised to sign on behalf of
//! the Cartridge account up to a configured allowance and expiry.
//!
//! This signer accepts that pre-issued session key and wraps it as a
//! `LocalWallet`, enabling your Rust backend to sign transactions on behalf of
//! the user without ever holding the primary key.
//!
//! ## Integration flow
//!
//! ```text
//! Browser (JS)                      Rust backend (starkzap-rs)
//! ─────────────────────────────     ─────────────────────────
//! 1. User authenticates via Cartridge passkey
//! 2. Request a session key:
//!    controller.requestSession({ ... })
//! 3. Extract session key:
//!    const { privateKey, publicKey } = session
//! 4. Send to your backend ──────────────────────────────────▶
//!                                   5. CartridgeSigner::new(session_key, account_address)
//!                                   6. sdk.onboard(OnboardConfig::Cartridge(signer))
//!                                   7. wallet.transfer(...)  ← signs with session key
//! ```
//!
//! ## Session key lifetime
//!
//! Session keys have an expiry set at the time of issuance. Your Rust code does
//! not enforce this — it is the Starknet network that will reject expired-session
//! signatures. Re-request a session from the browser when needed.

use starknet::{
    core::types::Felt,
    signers::{LocalWallet, SigningKey},
};

use crate::error::{Result, StarkzapError};

/// A Cartridge-backed signer using a pre-issued session key.
///
/// # Example
///
/// ```rust
/// use starkzap_rs::signer::CartridgeSigner;
///
/// // Session key and account address received from the Cartridge browser flow
/// let session_key_hex = std::env::var("CARTRIDGE_SESSION_KEY").unwrap();
/// let account_address_hex = std::env::var("CARTRIDGE_ACCOUNT_ADDRESS").unwrap();
///
/// let signer = CartridgeSigner::new(&session_key_hex, &account_address_hex)?;
/// # Ok::<(), starkzap_rs::StarkzapError>(())
/// ```
#[derive(Debug, Clone)]
pub struct CartridgeSigner {
    pub(crate) wallet: LocalWallet,
    pub(crate) address: Felt,
}

impl CartridgeSigner {
    /// Construct from a 0x-prefixed hex session private key and the Cartridge
    /// account address.
    ///
    /// # Arguments
    ///
    /// * `session_key_hex` — the session private key from Cartridge's
    ///   `requestSession()` call
    /// * `account_address_hex` — the Cartridge account's on-chain address
    pub fn new(session_key_hex: &str, account_address_hex: &str) -> Result<Self> {
        let pk_felt = Felt::from_hex(session_key_hex)
            .map_err(|_| StarkzapError::InvalidPrivateKey)?;

        let address = Felt::from_hex(account_address_hex)
            .map_err(|_| StarkzapError::InvalidAddress(account_address_hex.to_string()))?;

        let signing_key = SigningKey::from_secret_scalar(pk_felt);
        let wallet = LocalWallet::from(signing_key);

        Ok(Self { wallet, address })
    }

    /// The Cartridge account address this session key is authorised for.
    pub fn address(&self) -> Felt {
        self.address
    }

    /// Access the underlying [`LocalWallet`].
    pub fn local_wallet(&self) -> &LocalWallet {
        &self.wallet
    }
}