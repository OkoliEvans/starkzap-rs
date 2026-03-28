//! Top-level SDK entry point — mirrors `new StarkZap(config)` from the TS SDK.

use std::sync::Arc;

use starknet::{
    accounts::{ExecutionEncoding, SingleOwnerAccount},
    core::types::Felt,
    providers::{JsonRpcClient, Url, jsonrpc::HttpTransport},
    signers::LocalWallet,
};
use tracing::info;

use crate::{
    error::{Result, StarkzapError},
    network::Network,
    signer::StarkSigner,
    wallet::Wallet,
};

#[cfg(feature = "cartridge")]
use crate::signer::CartridgeSigner;

#[cfg(feature = "privy")]
use crate::signer::PrivySigner;

/// SDK initialisation configuration.
#[derive(Debug, Clone)]
pub struct StarkZapConfig {
    /// Which network to connect to.
    pub network: Network,
    /// Custom RPC endpoint URL. Falls back to a public endpoint if `None`.
    pub rpc_url: Option<String>,
}

impl StarkZapConfig {
    /// Mainnet configuration with the default public RPC.
    pub fn mainnet() -> Self {
        Self { network: Network::Mainnet, rpc_url: None }
    }

    /// Sepolia configuration with the default public RPC.
    pub fn sepolia() -> Self {
        Self { network: Network::Sepolia, rpc_url: None }
    }

    /// Use a custom RPC endpoint.
    ///
    /// ```rust
    /// use starkzap_rs::{StarkZapConfig, Network};
    ///
    /// let config = StarkZapConfig::sepolia()
    ///     .with_rpc("https://starknet-sepolia.g.alchemy.com/starknet/version/rpc/v0_8/YOUR_KEY");
    /// ```
    pub fn with_rpc(mut self, url: impl Into<String>) -> Self {
        self.rpc_url = Some(url.into());
        self
    }

    fn rpc_url(&self) -> &str {
        self.rpc_url
            .as_deref()
            .unwrap_or_else(|| self.network.default_rpc_url())
    }
}

// ── Onboard config ────────────────────────────────────────────────────────────

/// Signer strategy for [`StarkZap::onboard`].
///
/// Mirrors the TypeScript `OnboardStrategy` / signer selection.
pub enum OnboardConfig {
    /// Raw private-key signer (server-side / scripts).
    Signer(StarkSigner),

    /// Cartridge session-key signer (requires `cartridge` feature).
    #[cfg(feature = "cartridge")]
    Cartridge(CartridgeSigner),

    /// Privy server-side signer (requires `privy` feature).
    ///
    /// The `PrivySigner` must already have a wallet loaded
    /// (call [`PrivySigner::create_wallet`] or [`PrivySigner::with_wallet`] first).
    #[cfg(feature = "privy")]
    Privy(PrivySigner),
}

// ── StarkZap ──────────────────────────────────────────────────────────────────

/// The top-level SDK instance.
///
/// # Example
///
/// ```rust,no_run
/// use starkzap_rs::{StarkZap, StarkZapConfig, OnboardConfig, signer::StarkSigner};
///
/// # async fn example() -> starkzap_rs::error::Result<()> {
/// let sdk = StarkZap::new(StarkZapConfig::sepolia());
///
/// let signer = StarkSigner::new(
///     &std::env::var("PRIVATE_KEY").unwrap(),
///     &std::env::var("ACCOUNT_ADDRESS").unwrap(),
/// )?;
///
/// let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
/// println!("Wallet: {}", wallet.address_hex());
/// # Ok(())
/// # }
/// ```
pub struct StarkZap {
    config: StarkZapConfig,
    provider: Arc<JsonRpcClient<HttpTransport>>,
}

impl StarkZap {
    /// Construct a new SDK instance with the given config.
    ///
    /// # Panics
    ///
    /// Panics if the RPC URL in the config is not a valid URL. Use a valid
    /// HTTP/HTTPS URL (e.g. from Alchemy, Infura, or the default public endpoint).
    pub fn new(config: StarkZapConfig) -> Self {
        let url = Url::parse(config.rpc_url())
            .unwrap_or_else(|_| panic!("Invalid RPC URL: {}", config.rpc_url()));

        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(url)));

        info!(network = %config.network, rpc = config.rpc_url(), "StarkZap initialised");

        Self { config, provider }
    }

    /// Connect a wallet using the given signer strategy.
    ///
    /// Returns a [`Wallet`] ready for token, staking, and execution operations.
    pub async fn onboard(&self, config: OnboardConfig) -> Result<Wallet> {
        match config {
            OnboardConfig::Signer(signer) => self.build_wallet(signer.wallet, signer.address),

            #[cfg(feature = "cartridge")]
            OnboardConfig::Cartridge(signer) => {
                self.build_wallet(signer.wallet, signer.address)
            }

            #[cfg(feature = "privy")]
            OnboardConfig::Privy(signer) => {
                let address = signer
                    .address
                    .ok_or_else(|| StarkzapError::Other(
                        "PrivySigner has no wallet loaded. Call create_wallet() or with_wallet() first.".into(),
                    ))?;

                // Privy signs via REST, not locally. We use a dummy LocalWallet
                // as the starknet-rs account signer — real signing is intercepted
                // at the wallet.execute() layer via the paymaster-style pattern.
                //
                // TODO: For full Privy signing support, implement a custom
                //       starknet::signers::Signer that delegates to PrivySigner::sign_hash().
                //       Tracked in: https://github.com/your-org/starkzap-rs/issues/1
                Err(StarkzapError::Other(
                    "Privy signing delegation not yet wired to starknet-rs Account. \
                     Use StarkSigner for now and see issue #1.".into(),
                ))
            }
        }
    }

    /// The underlying provider — use this for direct RPC calls if needed.
    pub fn provider(&self) -> Arc<JsonRpcClient<HttpTransport>> {
        Arc::clone(&self.provider)
    }

    /// The current network configuration.
    pub fn network(&self) -> Network {
        self.config.network
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn build_wallet(&self, wallet: LocalWallet, address: Felt) -> Result<Wallet> {
        let account = SingleOwnerAccount::new(
            Arc::clone(&self.provider),
            wallet,
            address,
            self.config.network.chain_id(),
            ExecutionEncoding::New,
        );

        Ok(Wallet {
            account: Arc::new(account),
            provider: Arc::clone(&self.provider),
            address,
            network: self.config.network,
        })
    }
}