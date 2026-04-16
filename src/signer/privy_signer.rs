//! Privy server-side signer (feature = `"privy"`).
//!
//! Privy provides an embedded wallet API where your **server** creates and
//! manages wallets, and signing is delegated to Privy's infrastructure via REST.
//! Your application never holds the private key.
//!
//! ## Setup
//!
//! 1. Create a Privy app at <https://privy.io>
//! 2. In the dashboard → **Settings → API Keys** → copy your `App ID` and `App Secret`
//! 3. Set the environment variables:
//!    ```sh
//!    PRIVY_APP_ID=clxxxxxxxxxxxxxxxx
//!    PRIVY_APP_SECRET=privy_secret_...
//!    ```
//!
//! ## Usage
//!
//! ```rust,no_run
//! use starkzap_rs::signer::PrivySigner;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), starkzap_rs::StarkzapError> {
//! let signer = PrivySigner::from_env()?;
//!
//! // Create a new embedded wallet for a user (returns address)
//! let mut signer = signer;
//! let address = signer.create_wallet("user-id-123").await?;
//!
//! // Or load an existing wallet
//! let signer = signer.with_wallet("wallet-id", address);
//! # let _ = signer;
//! # Ok(())
//! # }
//! ```
//!
//! ## How signing works
//!
//! Privy's server API does not expose raw private keys. When you call
//! `account.execute_v3(calls)`, the starknet-rs account asks the signer to
//! sign a transaction hash. This signer sends that hash to Privy's
//! `/wallets/{id}/rpc` endpoint and receives back the `(r, s)` signature.

use std::sync::Arc;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use starknet::{
    core::types::Felt,
    signers::{Signer, SignerInteractivityContext, VerifyingKey},
};

use crate::error::{Result, StarkzapError};

/// Privy server-side signer.
///
/// Wraps the Privy API for wallet creation and transaction signing.
/// Requires the `privy` feature flag.
#[derive(Debug, Clone)]
pub struct PrivySigner {
    client: Arc<Client>,
    app_id: String,
    app_secret: String,
    /// The Privy wallet ID (returned when a wallet is created).
    wallet_id: Option<String>,
    /// The on-chain Starknet address of the Privy wallet.
    pub(crate) address: Option<Felt>,
    /// The Stark public key when available.
    pub(crate) public_key: Option<Felt>,
}

/// Privy wallet metadata needed to reuse a created Starknet wallet.
#[derive(Debug, Clone)]
pub struct PrivyWalletInfo {
    pub wallet_id: String,
    pub address: Felt,
    pub public_key: Option<Felt>,
}

// ── Internal Privy API types ──────────────────────────────────────────────────

#[derive(Serialize)]
struct CreateWalletRequest<'a> {
    chain_type: &'a str,
}

#[derive(Deserialize)]
struct CreateWalletResponse {
    id: String,
    address: String,
    #[serde(default)]
    public_key: Option<String>,
}

#[derive(Serialize)]
struct RpcRequest<'a> {
    method: &'a str,
    params: serde_json::Value,
}

#[derive(Deserialize)]
struct SignResponse {
    data: SignData,
}

#[derive(Deserialize)]
struct SignData {
    signature: String,
}

// ── Implementation ────────────────────────────────────────────────────────────

impl PrivySigner {
    const PRIVY_BASE: &'static str = "https://auth.privy.io/api/v1";

    /// Construct from explicit credentials.
    pub fn new(app_id: impl Into<String>, app_secret: impl Into<String>) -> Self {
        Self {
            client: Arc::new(Client::new()),
            app_id: app_id.into(),
            app_secret: app_secret.into(),
            wallet_id: None,
            address: None,
            public_key: None,
        }
    }

    /// Construct from environment variables.
    ///
    /// Reads `PRIVY_APP_ID` and `PRIVY_APP_SECRET`.
    ///
    /// # Errors
    ///
    /// Returns [`StarkzapError::Other`] if either variable is missing.
    pub fn from_env() -> Result<Self> {
        let app_id = std::env::var("PRIVY_APP_ID")
            .map_err(|_| StarkzapError::Other("PRIVY_APP_ID env var not set".into()))?;
        let app_secret = std::env::var("PRIVY_APP_SECRET")
            .map_err(|_| StarkzapError::Other("PRIVY_APP_SECRET env var not set".into()))?;
        Ok(Self::new(app_id, app_secret))
    }

    /// Attach a known wallet ID and address (for an existing Privy wallet).
    pub fn with_wallet(mut self, wallet_id: impl Into<String>, address: Felt) -> Self {
        self.wallet_id = Some(wallet_id.into());
        self.address = Some(address);
        self
    }

    /// Attach a known wallet ID, address, and Stark public key.
    pub fn with_wallet_and_public_key(
        mut self,
        wallet_id: impl Into<String>,
        address: Felt,
        public_key: Felt,
    ) -> Self {
        self.wallet_id = Some(wallet_id.into());
        self.address = Some(address);
        self.public_key = Some(public_key);
        self
    }

    /// Create a new Starknet embedded wallet via the Privy API.
    ///
    /// Returns the created wallet metadata so callers can persist the
    /// `wallet_id`, `address`, and `public_key` for later sessions.
    pub async fn create_wallet_info(&mut self, _user_id: &str) -> Result<PrivyWalletInfo> {
        let auth = self.basic_auth_header();

        let resp = self
            .client
            .post(format!("{}/wallets", Self::PRIVY_BASE))
            .header("Authorization", auth)
            .header("privy-app-id", &self.app_id)
            .header("Content-Type", "application/json")
            .json(&CreateWalletRequest { chain_type: "starknet" })
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status != 200 && status != 201 {
            let body = resp.text().await.unwrap_or_default();
            return Err(StarkzapError::PrivyApi { status, body });
        }

        let payload: CreateWalletResponse = resp.json().await?;
        let address = Felt::from_hex(&payload.address)
            .map_err(|_| StarkzapError::InvalidAddress(payload.address.clone()))?;
        let public_key = payload
            .public_key
            .as_deref()
            .map(Felt::from_hex)
            .transpose()
            .map_err(|_| StarkzapError::Other("Invalid Privy public key".into()))?;

        let wallet_id = payload.id;

        self.wallet_id = Some(wallet_id.clone());
        self.address = Some(address);
        self.public_key = public_key;

        Ok(PrivyWalletInfo {
            wallet_id,
            address,
            public_key,
        })
    }

    /// Create a new Starknet embedded wallet via the Privy API.
    ///
    /// Returns the on-chain address of the new wallet.
    pub async fn create_wallet(&mut self, user_id: &str) -> Result<Felt> {
        Ok(self.create_wallet_info(user_id).await?.address)
    }

    /// Sign a transaction hash using the Privy signing API.
    ///
    /// Returns the `(r, s)` signature components as felt bytes.
    pub async fn sign_hash(&self, hash: Felt) -> Result<(Felt, Felt)> {
        let wallet_id = self
            .wallet_id
            .as_ref()
            .ok_or_else(|| StarkzapError::PrivySigning("No wallet loaded".into()))?;

        let auth = self.basic_auth_header();

        let resp = self
            .client
            .post(format!("{}/wallets/{}/rpc", Self::PRIVY_BASE, wallet_id))
            .header("Authorization", auth)
            .header("privy-app-id", &self.app_id)
            .header("Content-Type", "application/json")
            .json(&RpcRequest {
                method: "starknet_signHash",
                params: serde_json::json!({ "hash": format!("{:#x}", hash) }),
            })
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status != 200 {
            let body = resp.text().await.unwrap_or_default();
            return Err(StarkzapError::PrivyApi { status, body });
        }

        let payload: SignResponse = resp.json().await?;

        // Privy returns signature as "0xR,S" or as a hex string — parse accordingly.
        parse_signature(&payload.data.signature)
    }

    /// The on-chain address of the loaded Privy wallet.
    pub fn address(&self) -> Option<Felt> {
        self.address
    }

    /// The Privy wallet ID of the loaded wallet.
    pub fn wallet_id(&self) -> Option<&str> {
        self.wallet_id.as_deref()
    }

    /// The Stark public key of the loaded Privy wallet, when available.
    pub fn public_key(&self) -> Option<Felt> {
        self.public_key
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn basic_auth_header(&self) -> String {
        use base64::Engine;
        let credentials = format!("{}:{}", self.app_id, self.app_secret);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        format!("Basic {}", encoded)
    }
}

fn parse_signature(sig: &str) -> Result<(Felt, Felt)> {
    // Privy may return either "r,s" or a 64-byte concatenated signature.
    let parts: Vec<&str> = sig.split(',').collect();
    match parts.as_slice() {
        [r, s] => {
            let r = Felt::from_hex(r.trim()).map_err(|_| {
                StarkzapError::PrivySigning(format!("invalid r component: {}", r))
            })?;
            let s = Felt::from_hex(s.trim()).map_err(|_| {
                StarkzapError::PrivySigning(format!("invalid s component: {}", s))
            })?;
            Ok((r, s))
        }
        _ => {
            let normalized = sig.trim().strip_prefix("0x").unwrap_or(sig.trim());
            if normalized.len() == 128 && normalized.chars().all(|c| c.is_ascii_hexdigit()) {
                let r = Felt::from_hex(&format!("0x{}", &normalized[..64]))
                    .map_err(|_| StarkzapError::PrivySigning("invalid r component".into()))?;
                let s = Felt::from_hex(&format!("0x{}", &normalized[64..]))
                    .map_err(|_| StarkzapError::PrivySigning("invalid s component".into()))?;
                Ok((r, s))
            } else {
                Err(StarkzapError::PrivySigning(format!(
                    "unexpected signature format: {}",
                    sig
                )))
            }
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl Signer for PrivySigner {
    type GetPublicKeyError = StarkzapError;
    type SignError = StarkzapError;

    async fn get_public_key(&self) -> std::result::Result<VerifyingKey, Self::GetPublicKeyError> {
        let public_key = self
            .public_key
            .ok_or_else(|| StarkzapError::PrivySigning("No public key loaded".into()))?;
        Ok(VerifyingKey::from_scalar(public_key))
    }

    async fn sign_hash(
        &self,
        hash: &Felt,
    ) -> std::result::Result<starknet::core::crypto::Signature, Self::SignError> {
        let (r, s) = self.sign_hash(*hash).await?;
        Ok(starknet::core::crypto::Signature { r, s })
    }

    fn is_interactive(&self, _context: SignerInteractivityContext<'_>) -> bool {
        false
    }
}
