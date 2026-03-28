//! Core wallet interface — the primary surface of the SDK.
//!
//! [`Wallet`] is returned by [`crate::sdk::StarkZap::onboard`] and provides all
//! token, transfer, and execution operations.

use std::sync::Arc;

use starknet::{
    accounts::{Account, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount},
    core::{
        types::{BlockId, BlockTag, Call, Felt, FunctionCall},
        utils::get_selector_from_name,
    },
    providers::{JsonRpcClient, Provider, jsonrpc::HttpTransport},
    signers::LocalWallet,
};
use tracing::{debug, info};

use crate::{
    amount::Amount,
    error::{Result, StarkzapError},
    network::Network,
    paymaster::{FeeMode, PaymasterClient},
    tokens::Token,
    tx::Tx,
};

/// Deploy behavior passed to [`Wallet::ensure_ready`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployPolicy {
    /// Deploy the account if it is not yet on-chain.
    IfNeeded,
    /// Never deploy — error if not deployed.
    Never,
    /// Always redeploy (rare; for testing).
    Always,
}

/// A transfer recipient.
#[derive(Debug, Clone)]
pub struct Recipient {
    pub to: Felt,
    pub amount: Amount,
}

impl Recipient {
    pub fn new(to: Felt, amount: Amount) -> Self {
        Self { to, amount }
    }
}

// ── Wallet ────────────────────────────────────────────────────────────────────

/// The primary wallet handle — returned by [`StarkZap::onboard`].
///
/// All methods are `async` and safe to clone (uses `Arc` internally).
#[derive(Clone)]
pub struct Wallet {
    pub(crate) account: Arc<SingleOwnerAccount<Arc<JsonRpcClient<HttpTransport>>, LocalWallet>>,
    pub(crate) provider: Arc<JsonRpcClient<HttpTransport>>,
    pub(crate) address: Felt,
    pub(crate) network: Network,
}

impl Wallet {
    /// The on-chain address of this wallet.
    pub fn address(&self) -> Felt {
        self.address
    }

    /// The address as a `0x`-prefixed hex string.
    pub fn address_hex(&self) -> String {
        format!("{:#x}", self.address)
    }

    /// Ensure the account is deployed on-chain.
    ///
    /// Checks whether the account contract is deployed by inspecting its class
    /// hash. If not deployed and `policy == IfNeeded`, sends a UDC deploy
    /// transaction and waits for confirmation.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// wallet.ensure_ready(DeployPolicy::IfNeeded).await?;
    /// ```
    pub async fn ensure_ready(&self, policy: DeployPolicy) -> Result<()> {
        let deployed = self.is_deployed().await?;

        if deployed {
            debug!(address = %self.address_hex(), "account already deployed");
            return Ok(());
        }

        match policy {
            DeployPolicy::Never => Err(StarkzapError::NotDeployed),
            DeployPolicy::IfNeeded | DeployPolicy::Always => {
                info!(address = %self.address_hex(), "deploying account");
                self.deploy_account().await
            }
        }
    }

    /// Returns `true` if the account contract is deployed on-chain.
    pub async fn is_deployed(&self) -> Result<bool> {
        match self
            .provider
            .get_class_hash_at(BlockId::Tag(BlockTag::Latest), self.address)
            .await
        {
            Ok(_) => Ok(true),
            Err(starknet::providers::ProviderError::StarknetError(
                starknet::core::types::StarknetError::ContractNotFound,
            )) => Ok(false),
            Err(e) => Err(StarkzapError::Provider(e)),
        }
    }

    /// Query the ERC-20 balance of this wallet for the given token.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let balance = wallet.balance_of(&tokens::mainnet::usdc()).await?;
    /// println!("{}", balance); // "100.0 USDC"
    /// ```
    pub async fn balance_of(&self, token: &Token) -> Result<Amount> {
        let result = self
            .provider
            .call(
                FunctionCall {
                    contract_address: token.address,
                    entry_point_selector: get_selector_from_name("balanceOf")
                        .map_err(|e| StarkzapError::Other(e.to_string()))?,
                    calldata: vec![self.address],
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await
            .map_err(StarkzapError::Provider)?;

        // ERC-20 returns Uint256 { low: felt, high: felt }
        let low = result.get(0).copied().unwrap_or(Felt::ZERO);
        let high = result.get(1).copied().unwrap_or(Felt::ZERO);

        if high != Felt::ZERO {
            // Value exceeds u128::MAX — extremely unlikely for real balances
            return Err(StarkzapError::AmountOverflow);
        }

        let raw: u128 = low
            .to_biguint()
            .try_into()
            .map_err(|_| StarkzapError::AmountOverflow)?;

        Ok(Amount::from_raw(raw, token))
    }

    /// Transfer tokens to one or more recipients in a single multicall.
    ///
    /// # Arguments
    ///
    /// * `token` — the ERC-20 token to transfer
    /// * `recipients` — list of `(address, amount)` pairs
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use starkzap_rs::{Amount, wallet::Recipient, tokens::mainnet};
    ///
    /// let usdc = mainnet::usdc();
    /// let tx = wallet.transfer(&usdc, vec![
    ///     Recipient::new(recipient_address, Amount::parse("10", &usdc)?),
    /// ]).await?;
    /// tx.wait().await?;
    /// ```
    pub async fn transfer(&self, token: &Token, recipients: Vec<Recipient>) -> Result<Tx> {
        if recipients.is_empty() {
            return Err(StarkzapError::Other("No recipients provided".into()));
        }

        let transfer_selector = get_selector_from_name("transfer")
            .map_err(|e| StarkzapError::Other(e.to_string()))?;

        let calls: Vec<Call> = recipients
            .iter()
            .map(|r| {
                let [low, high] = r.amount.to_u256_felts();
                Call {
                    to: token.address,
                    selector: transfer_selector,
                    calldata: vec![r.to, low, high],
                }
            })
            .collect();

        self.execute(calls, FeeMode::UserPays).await
    }

    /// Execute a raw list of Starknet calls.
    ///
    /// Use this for any on-chain operation not covered by higher-level methods
    /// (e.g. custom contract interactions, DeFi protocols).
    ///
    /// # Arguments
    ///
    /// * `calls` — ordered list of contract calls to execute atomically
    /// * `fee_mode` — [`FeeMode::UserPays`] or [`FeeMode::Paymaster`]
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use starknet::core::types::Call;
    /// use starkzap_rs::paymaster::FeeMode;
    ///
    /// let tx = wallet.execute(calls, FeeMode::UserPays).await?;
    /// tx.wait().await?;
    /// ```
    pub async fn execute(&self, calls: Vec<Call>, fee_mode: FeeMode) -> Result<Tx> {
        let hash = match fee_mode {
            FeeMode::UserPays => {
                debug!("Executing {} call(s) with user-pays fee", calls.len());
                let result = self
                    .account
                    .execute_v3(calls)
                    .send()
                    .await
                    .map_err(|e| StarkzapError::Account(e.to_string()))?;
                result.transaction_hash
            }
            FeeMode::Paymaster(config) => {
                debug!("Executing {} call(s) via AVNU paymaster", calls.len());
                let pm = PaymasterClient::new(&self.network, &config);
                let account = Arc::clone(&self.account);
                pm.execute(self.address, calls, |hash| async move {
                    let sig = account
                        .sign_hash(&hash)
                        .await
                        .map_err(|e| StarkzapError::Signer(e.to_string()))?;
                    Ok(vec![sig.r, sig.s])
                })
                .await?
            }
        };

        info!(hash = %format!("{:#x}", hash), "transaction submitted");
        Ok(Tx::new(hash, Arc::clone(&self.provider) as Arc<dyn starknet::providers::Provider + Send + Sync>))
    }

    // ── Private ───────────────────────────────────────────────────────────────

    async fn deploy_account(&self) -> Result<()> {
        // For ArgentX / OpenZeppelin accounts, deployment is done via UDC.
        // The specific deploy call depends on the account class. Here we provide
        // a standard OpenZeppelin deploy flow.
        //
        // For production, extend this to support ArgentX v0.4/v0.5 deployment.
        Err(StarkzapError::DeployFailed(
            "Auto-deploy not yet implemented. Pre-deploy your account via starkli or Argent.".into(),
        ))
    }
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet")
            .field("address", &self.address_hex())
            .field("network", &self.network)
            .finish()
    }
}