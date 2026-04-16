//! Top-level SDK entry point — mirrors `new StarkZap(config)` from the TS SDK.

use std::sync::Arc;

use starknet::{
    accounts::{ExecutionEncoding, SingleOwnerAccount},
    core::types::{BlockId, BlockTag, StarknetError},
    providers::{JsonRpcClient, Provider, Url, jsonrpc::HttpTransport},
    signers::Signer,
};
use tracing::info;

use crate::{
    account::AccountPreset,
    error::Result,
    network::Network,
    signer::{AnySigner, StarkSigner},
    wallet::{StarknetProvider, Wallet},
    StarkzapError,
};

#[cfg(feature = "cartridge")]
use crate::signer::CartridgeSigner;

#[cfg(feature = "privy")]
use crate::signer::PrivySigner;

/// SDK initialisation configuration.
///
/// # RPC URL resolution order
///
/// 1. Explicit `.with_rpc("https://...")` call on this config.
/// 2. `RPC_URL` environment variable (read at [`StarkZap::new`] time).
/// 3. Network default (dRPC public endpoint — fine for dev, use your own key in prod).
///
/// # Example
///
/// ```rust,no_run
/// use starkzap_rs::{StarkZap, StarkZapConfig};
///
/// // Use env var RPC_URL if set, otherwise dRPC public fallback:
/// let sdk = StarkZap::new(StarkZapConfig::sepolia());
///
/// // Pin a specific endpoint explicitly:
/// let sdk = StarkZap::new(
///     StarkZapConfig::sepolia().with_rpc("https://starknet-sepolia.g.alchemy.com/v2/YOUR_API_KEY")
/// );
/// ```
#[derive(Debug, Clone)]
pub struct StarkZapConfig {
    pub network: Network,
    pub rpc_url: Option<String>,
}

impl StarkZapConfig {
    pub fn mainnet() -> Self {
        Self { network: Network::Mainnet, rpc_url: None }
    }

    pub fn sepolia() -> Self {
        Self { network: Network::Sepolia, rpc_url: None }
    }

    pub fn devnet() -> Self {
        Self { network: Network::Devnet, rpc_url: None }
    }

    /// Override the RPC endpoint explicitly.
    ///
    /// Takes priority over the `RPC_URL` env var.
    pub fn with_rpc(mut self, url: impl Into<String>) -> Self {
        self.rpc_url = Some(url.into());
        self
    }

    /// Resolve the RPC URL to use.
    ///
    /// Priority: explicit `with_rpc` → `RPC_URL` env var → network default.
    fn resolve_rpc_url(&self) -> String {
        if let Some(url) = &self.rpc_url {
            return url.clone();
        }
        if let Ok(url) = std::env::var("RPC_URL") {
            if !url.is_empty() {
                return url;
            }
        }
        self.network.default_rpc_url().to_string()
    }
}

// ── Onboard config ────────────────────────────────────────────────────────────

pub enum OnboardConfig {
    Signer(StarkSigner),
    SignerWithPreset(StarkSigner, AccountPreset),

    #[cfg(feature = "cartridge")]
    Cartridge(CartridgeSigner),
    #[cfg(feature = "cartridge")]
    CartridgeWithPreset(CartridgeSigner, AccountPreset),

    #[cfg(feature = "privy")]
    Privy(PrivySigner),
    #[cfg(feature = "privy")]
    PrivyWithPreset(PrivySigner, AccountPreset),
}

// ── StarkZap ──────────────────────────────────────────────────────────────────

pub struct StarkZap {
    config: StarkZapConfig,
    provider: Arc<StarknetProvider>,
    rpc_url: String,
}

impl StarkZap {
    pub fn new(config: StarkZapConfig) -> Self {
        let rpc_url = config.resolve_rpc_url();

        let url = Url::parse(&rpc_url)
            .unwrap_or_else(|_| panic!("Invalid RPC URL: {}", rpc_url));

        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(url)));

        info!(
            network = %config.network,
            rpc = %rpc_url,
            "StarkZap initialised"
        );

        Self {
            config,
            provider,
            rpc_url,
        }
    }

    pub async fn onboard(&self, config: OnboardConfig) -> Result<Wallet<StarknetProvider>> {
        match config {
            OnboardConfig::Signer(signer) => {
                self.build_wallet(AnySigner::Stark(signer), AccountPreset::default())
                    .await
            }
            OnboardConfig::SignerWithPreset(signer, preset) => {
                self.build_wallet(AnySigner::Stark(signer), preset).await
            }

            #[cfg(feature = "cartridge")]
            OnboardConfig::Cartridge(signer) => {
                self.build_wallet(AnySigner::Cartridge(signer), AccountPreset::default())
                    .await
            }
            #[cfg(feature = "cartridge")]
            OnboardConfig::CartridgeWithPreset(signer, preset) => {
                self.build_wallet(AnySigner::Cartridge(signer), preset).await
            }

            #[cfg(feature = "privy")]
            OnboardConfig::Privy(signer) => {
                self.build_wallet(AnySigner::Privy(signer), AccountPreset::ArgentXV050)
                    .await
            }
            #[cfg(feature = "privy")]
            OnboardConfig::PrivyWithPreset(signer, preset) => {
                self.build_wallet(AnySigner::Privy(signer), preset).await
            }
        }
    }

    /// Expose the underlying provider for advanced use cases.
    pub fn provider(&self) -> Arc<StarknetProvider> {
        Arc::clone(&self.provider)
    }

    pub fn network(&self) -> Network {
        self.config.network
    }

    // ── Private ───────────────────────────────────────────────────────────────

    async fn build_wallet(
        &self,
        signer: AnySigner,
        requested_preset: AccountPreset,
    ) -> Result<Wallet<StarknetProvider>> {
        let signer = Arc::new(signer);
        let public_key = signer
            .get_public_key()
            .await
            .map_err(|e| StarkzapError::Signer(e.to_string()))?
            .scalar();
        let counterfactual_address = requested_preset.counterfactual_address(public_key);
        let address = match signer.as_ref() {
            #[cfg(feature = "privy")]
            AnySigner::Privy(_) => counterfactual_address,
            _ => signer.known_address().unwrap_or(counterfactual_address),
        };
        let account_preset = self
            .resolve_account_preset(Arc::clone(&signer), address, requested_preset)
            .await?;

        let mut account = SingleOwnerAccount::new(
            Arc::clone(&self.provider),
            Arc::clone(&signer),
            address,
            self.config.network.chain_id(),
            if account_preset.uses_legacy_execution_encoding() {
                ExecutionEncoding::Legacy
            } else {
                ExecutionEncoding::New
            },
        );
        // Use `latest` for cross-provider compatibility. Older JSON-RPC
        // versions (including v0.8) reject the newer `pre_confirmed` tag.
        account.set_block_id(BlockId::Tag(BlockTag::Latest));

        Ok(Wallet {
            account: Arc::new(account),
            provider: Arc::clone(&self.provider),
            signer,
            address,
            network: self.config.network,
            account_preset,
            rpc_url: self.rpc_url.clone(),
        })
    }

    async fn resolve_account_preset(
        &self,
        signer: Arc<AnySigner>,
        address: starknet::core::types::Felt,
        requested_preset: AccountPreset,
    ) -> Result<AccountPreset> {
        match self
            .provider
            .get_class_hash_at(BlockId::Tag(BlockTag::Latest), address)
            .await
        {
            Ok(class_hash) => Ok(AccountPreset::from_class_hash(class_hash).unwrap_or(requested_preset)),
            Err(starknet::providers::ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                self.infer_preset_from_signer_address(signer, address, requested_preset).await
            }
            Err(_) => self
                .infer_preset_from_signer_address(signer, address, requested_preset)
                .await,
        }
    }

    async fn infer_preset_from_signer_address(
        &self,
        signer: Arc<AnySigner>,
        address: starknet::core::types::Felt,
        fallback: AccountPreset,
    ) -> Result<AccountPreset> {
        let public_key = signer
            .get_public_key()
            .await
            .map_err(|e| StarkzapError::Signer(e.to_string()))?
            .scalar();

        // Devnet is checked last — its address derivation can alias other presets
        // in some configurations, so we prefer the more specific presets first.
        let matching: Vec<AccountPreset> = [
            AccountPreset::OpenZeppelin,
            AccountPreset::Argent,
            AccountPreset::Braavos,
            AccountPreset::ArgentXV050,
            AccountPreset::Devnet,
        ]
        .into_iter()
        .filter(|preset| preset.counterfactual_address(public_key) == address)
        .collect();

        Ok(match matching.as_slice() {
            [preset] => *preset,
            _ => fallback,
        })
    }
}
