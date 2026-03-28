//! AVNU Gasless Paymaster integration.
//!
//! Enables fee sponsorship — end users pay no gas. AVNU sponsors the fee and
//! your account or AVNU's quota covers it.
//!
//! ## Setup
//!
//! 1. Register at <https://app.avnu.fi> and obtain an API key (optional for
//!    Sepolia, required for mainnet sponsored calls)
//! 2. Set `AVNU_API_KEY` in your environment (optional)
//! 3. Pass [`PaymasterConfig`] when calling [`Wallet::execute`]
//!
//! ## How it works
//!
//! ```text
//! 1. Build your calls as normal
//! 2. Call wallet.execute(calls, FeeMode::Paymaster(config))
//! 3. starkzap-rs POSTs to AVNU's /gasless/v1/build-transaction
//!    → receives a typed transaction with AVNU's fee token approval prepended
//! 4. You sign the built transaction
//! 5. starkzap-rs POSTs to /gasless/v1/execute with the signature
//!    → AVNU broadcasts the sponsored transaction
//! 6. Returns a Tx handle as normal
//! ```

use reqwest::Client;
use serde::{Deserialize, Serialize};
use starknet::core::types::{Call, Felt};
use tracing::debug;

use crate::{
    error::{Result, StarkzapError},
    network::Network,
};

/// Configuration for AVNU paymaster-sponsored transactions.
#[derive(Debug, Clone)]
pub struct PaymasterConfig {
    /// Optional AVNU API key. Required for mainnet; optional on Sepolia.
    pub api_key: Option<String>,
}

impl PaymasterConfig {
    /// No API key — suitable for Sepolia testing.
    pub fn new() -> Self {
        Self { api_key: None }
    }

    /// With an API key — required for mainnet.
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self { api_key: Some(api_key.into()) }
    }

    /// Load API key from `AVNU_API_KEY` environment variable.
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("AVNU_API_KEY").ok(),
        }
    }
}

impl Default for PaymasterConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Fee payment mode for [`Wallet::execute`].
#[derive(Debug, Clone)]
pub enum FeeMode {
    /// User pays the gas fee normally (default).
    UserPays,
    /// AVNU sponsors the gas fee.
    Paymaster(PaymasterConfig),
}

// ── Internal AVNU API types ───────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildRequest {
    user_address: String,
    calls: Vec<AvnuCall>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AvnuCall {
    contract_address: String,
    entry_point: String,
    calldata: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildResponse {
    transaction_hash: String,
    calls: Vec<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteRequest {
    user_address: String,
    calls: Vec<serde_json::Value>,
    signature: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteResponse {
    transaction_hash: String,
}

// ── Public client ─────────────────────────────────────────────────────────────

/// AVNU paymaster HTTP client.
pub(crate) struct PaymasterClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl PaymasterClient {
    pub fn new(network: &Network, config: &PaymasterConfig) -> Self {
        Self {
            client: Client::new(),
            base_url: network.avnu_base_url().to_string(),
            api_key: config.api_key.clone(),
        }
    }

    /// Submit calls as a gasless transaction.
    ///
    /// Internally:
    /// 1. Calls `/gasless/v1/build-transaction` to get the sponsored tx hash
    /// 2. Signs the hash with the provided signing closure
    /// 3. Calls `/gasless/v1/execute` to broadcast
    ///
    /// Returns the final transaction hash.
    pub async fn execute<F, Fut>(
        &self,
        account_address: Felt,
        calls: Vec<Call>,
        sign: F,
    ) -> Result<Felt>
    where
        F: FnOnce(Felt) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<Felt>>>,
    {
        let avnu_calls: Vec<AvnuCall> = calls
            .iter()
            .map(|c| AvnuCall {
                contract_address: format!("{:#x}", c.to),
                entry_point: format!("{:#x}", c.selector),
                calldata: c.calldata.iter().map(|f| format!("{:#x}", f)).collect(),
            })
            .collect();

        // 1. Build
        debug!("Requesting AVNU gasless build for {:#x}", account_address);
        let build_resp = self
            .post("gasless/v1/build-transaction", &BuildRequest {
                user_address: format!("{:#x}", account_address),
                calls: avnu_calls,
            })
            .await?;

        let build: BuildResponse = serde_json::from_str(&build_resp)?;

        let tx_hash = Felt::from_hex(&build.transaction_hash)
            .map_err(|_| StarkzapError::PaymasterMalformed {
                field: "transactionHash".into(),
            })?;

        // 2. Sign
        let signature_felts = sign(tx_hash).await?;
        let signature: Vec<String> = signature_felts
            .iter()
            .map(|f| format!("{:#x}", f))
            .collect();

        // 3. Execute
        debug!("Submitting AVNU gasless execute");
        let exec_resp = self
            .post("gasless/v1/execute", &ExecuteResponse {
                transaction_hash: build.transaction_hash,
            })
            .await?;

        let result: ExecuteResponse = serde_json::from_str(&exec_resp)?;

        Felt::from_hex(&result.transaction_hash).map_err(|_| {
            StarkzapError::PaymasterMalformed {
                field: "transactionHash".into(),
            }
        })
    }

    async fn post<T: Serialize>(&self, path: &str, body: &T) -> Result<String> {
        let mut req = self
            .client
            .post(format!("{}/{}", self.base_url, path))
            .header("Content-Type", "application/json");

        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req.json(body).send().await?;
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();

        if !(200..=299).contains(&(status as usize)) {
            return Err(StarkzapError::PaymasterRequest { status, body });
        }

        Ok(body)
    }
}