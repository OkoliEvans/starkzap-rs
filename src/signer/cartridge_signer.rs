//! Cartridge session signer (feature = `"cartridge"`).
//!
//! Cartridge sessions are not plain Stark private keys. The official Controller
//! flow stores session signer material alongside registration metadata such as the
//! `ownerGuid`, expiry, and policy merkle root inputs. This module mirrors that
//! shape so Rust can route execution through the same session account model.

use std::process::Command;

use base64::Engine;
use serde::{Deserialize, Serialize};
use starknet::{
    core::types::{Call, Felt},
    core::utils::get_selector_from_name,
    signers::{LocalWallet, Signer, SignerInteractivityContext, SigningKey, VerifyingKey},
};

use crate::error::{Result, StarkzapError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeSessionSignerMaterial {
    #[serde(rename = "privKey")]
    pub priv_key: String,
    #[serde(rename = "pubKey")]
    pub pub_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeSessionRegistration {
    pub username: Option<String>,
    pub address: String,
    #[serde(rename = "ownerGuid")]
    pub owner_guid: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    #[serde(rename = "guardianKeyGuid", default = "default_zero_hex")]
    pub guardian_key_guid: String,
    #[serde(rename = "metadataHash", default = "default_zero_hex")]
    pub metadata_hash: String,
    #[serde(rename = "sessionKeyGuid")]
    pub session_key_guid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeCallPolicy {
    pub target: String,
    pub method: String,
    #[serde(default)]
    pub authorized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartridgeSessionBundle {
    #[serde(rename = "rpcUrl")]
    pub rpc_url: String,
    #[serde(rename = "chainId")]
    pub chain_id: String,
    pub signer: CartridgeSessionSignerMaterial,
    pub session: CartridgeSessionRegistration,
    pub policies: Vec<CartridgeCallPolicy>,
}

fn default_zero_hex() -> String {
    "0x0".to_string()
}

/// Cartridge-backed signer using a pre-issued session bundle.
#[derive(Debug, Clone)]
pub struct CartridgeSigner {
    pub(crate) wallet: LocalWallet,
    pub(crate) address: Felt,
    pub(crate) public_key: Felt,
    pub(crate) session: CartridgeSessionBundle,
}

impl CartridgeSigner {
    /// Construct from a full session bundle exported from the browser flow.
    pub fn from_session_bundle(session: CartridgeSessionBundle) -> Result<Self> {
        let pk_felt =
            Felt::from_hex(&session.signer.priv_key).map_err(|_| StarkzapError::InvalidPrivateKey)?;
        let address = Felt::from_hex(&session.session.address)
            .map_err(|_| StarkzapError::InvalidAddress(session.session.address.clone()))?;

        let signing_key = SigningKey::from_secret_scalar(pk_felt);
        let public_key = if session.signer.pub_key.is_empty() {
            signing_key.verifying_key().scalar()
        } else {
            Felt::from_hex(&session.signer.pub_key)
                .map_err(|_| StarkzapError::Signer("invalid Cartridge session public key".into()))?
        };
        let wallet = LocalWallet::from(signing_key);

        Ok(Self {
            wallet,
            address,
            public_key,
            session,
        })
    }

    /// Construct from the `CARTRIDGE_SESSION_BUNDLE_B64` env var.
    pub fn from_env() -> Result<Self> {
        let encoded = std::env::var("CARTRIDGE_SESSION_BUNDLE_B64").map_err(|_| {
            StarkzapError::Other("CARTRIDGE_SESSION_BUNDLE_B64 env var not set".into())
        })?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|e| StarkzapError::Cartridge(format!("invalid session bundle base64: {e}")))?;
        let session: CartridgeSessionBundle = serde_json::from_slice(&bytes)?;
        Self::from_session_bundle(session)
    }

    /// Backward-compatible constructor for older examples.
    pub fn new(session_key_hex: &str, account_address_hex: &str) -> Result<Self> {
        Self::from_session_bundle(CartridgeSessionBundle {
            rpc_url: String::new(),
            chain_id: String::new(),
            signer: CartridgeSessionSignerMaterial {
                priv_key: session_key_hex.to_string(),
                pub_key: String::new(),
            },
            session: CartridgeSessionRegistration {
                username: None,
                address: account_address_hex.to_string(),
                owner_guid: String::new(),
                expires_at: "0".into(),
                guardian_key_guid: default_zero_hex(),
                metadata_hash: default_zero_hex(),
                session_key_guid: String::new(),
            },
            policies: Vec::new(),
        })
    }

    pub fn address(&self) -> Felt {
        self.address
    }

    pub fn session_bundle(&self) -> &CartridgeSessionBundle {
        &self.session
    }

    pub fn execute_via_session(&self, calls_json: &str) -> Result<Felt> {
        let helper = format!(
            "{}/examples/cartridge_session_web/cartridge_execute.mjs",
            env!("CARGO_MANIFEST_DIR")
        );
        let bundle_json = serde_json::to_string(&self.session)?;

        let output = Command::new("node")
            .arg("--experimental-wasm-modules")
            .arg(helper)
            .arg(bundle_json)
            .arg(calls_json)
            .output()
            .map_err(|e| StarkzapError::Cartridge(format!("failed to launch Cartridge helper: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let details = if !stderr.is_empty() { stderr } else { stdout };
            return Err(StarkzapError::Cartridge(if details.is_empty() {
                "Cartridge helper exited unsuccessfully".into()
            } else {
                details
            }));
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|e| StarkzapError::Cartridge(format!("invalid helper output: {e}")))?;
        let value: serde_json::Value = serde_json::from_str(stdout.trim())
            .map_err(|e| StarkzapError::Cartridge(format!("invalid helper JSON: {e}")))?;
        let hash = value
            .get("transaction_hash")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| StarkzapError::Cartridge("helper returned no transaction_hash".into()))?;

        Felt::from_hex(hash)
            .map_err(|_| StarkzapError::Cartridge(format!("invalid transaction hash: {hash}")))
    }
}

#[async_trait::async_trait]
impl Signer for CartridgeSigner {
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

pub fn calls_to_cartridge_json(calls: &[Call]) -> Result<String> {
    let mapped: Vec<serde_json::Value> = calls
        .iter()
        .map(|call| {
            Ok(serde_json::json!({
                "contractAddress": format!("{:#x}", call.to),
                "entrypoint": selector_to_entrypoint(call.selector)?,
                "calldata": call.calldata.iter().map(|felt| format!("{:#x}", felt)).collect::<Vec<_>>(),
            }))
        })
        .collect::<Result<_>>()?;

    serde_json::to_string(&mapped).map_err(Into::into)
}

fn selector_to_entrypoint(selector: Felt) -> Result<&'static str> {
    for name in [
        "transfer",
        "approve",
        "enter_delegation_pool",
        "add_to_delegation_pool",
        "exit_delegation_pool_intent",
        "exit_delegation_pool_action",
        "claim_rewards",
    ] {
        if get_selector_from_name(name).map_err(|e| StarkzapError::Cartridge(e.to_string()))?
            == selector
        {
            return Ok(name);
        }
    }

    Err(StarkzapError::Cartridge(format!(
        "unsupported entrypoint selector for Cartridge session execution: {:#x}",
        selector
    )))
}
