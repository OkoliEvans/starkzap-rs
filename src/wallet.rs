//! Core wallet interface — the primary surface of the SDK.
//!
//! [`Wallet`] is returned by [`crate::sdk::StarkZap::onboard`] and provides all
//! token, transfer, and execution operations.

use std::sync::Arc;

use reqwest::Client;
use serde_json::{Value, json};
use starknet::{
    accounts::{Account, AccountFactory, ExecutionEncoder, SingleOwnerAccount},
    core::crypto::compute_hash_on_elements,
    core::{
        types::{BlockId, BlockTag, Call, ExecuteInvocation, Felt, FunctionCall, TransactionTrace},
        utils::get_selector_from_name,
    },
    providers::{JsonRpcClient, Provider, jsonrpc::HttpTransport},
    signers::Signer,
};
use tracing::{debug, info};

use crate::{
    account::{AccountPreset, PresetAccountFactory},
    amount::Amount,
    error::{Result, StarkzapError},
    network::Network,
    paymaster::{
        AccountDeploymentData, FeeMode, PaymasterClient, PaymasterDetails,
        PreparedPaymasterTransaction,
    },
    signer::AnySigner,
    tokens::Token,
    tx::Tx,
};

/// When to deploy the account contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployMode {
    /// Never deploy. Fail if the account is not already on-chain.
    Never,
    /// Deploy only if the account is not yet on-chain.
    IfNeeded,
    /// Mirrors StarkZap TS. Currently behaves like [`DeployMode::IfNeeded`]:
    /// if the account is already deployed, no deployment is attempted.
    Always,
}

/// Backward-compatible alias for older examples and code.
pub type DeployPolicy = DeployMode;

/// Progress steps emitted during `ensure_ready_with_options`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStep {
    Connected,
    CheckDeployed,
    Deploying,
    Failed,
    Ready,
}

/// Progress event emitted during `ensure_ready_with_options`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressEvent {
    pub step: ProgressStep,
}

/// Options for `wallet.ensure_ready_with_options()`.
#[derive(Debug, Clone)]
pub struct EnsureReadyOptions {
    pub deploy: DeployMode,
    pub fee_mode: Option<FeeMode>,
}

impl Default for EnsureReadyOptions {
    fn default() -> Self {
        Self {
            deploy: DeployMode::IfNeeded,
            fee_mode: None,
        }
    }
}

/// Options for `wallet.execute_with_options()`.
#[derive(Debug, Clone)]
pub struct ExecuteOptions {
    pub fee_mode: Option<FeeMode>,
}

impl Default for ExecuteOptions {
    fn default() -> Self {
        Self { fee_mode: None }
    }
}

/// Options for `wallet.preflight()`.
#[derive(Debug, Clone)]
pub struct PreflightOptions {
    pub calls: Vec<Call>,
    pub fee_mode: Option<FeeMode>,
}

/// Result of a preflight check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightResult {
    pub ok: bool,
    pub reason: Option<String>,
}

impl PreflightResult {
    pub const fn ok() -> Self {
        Self {
            ok: true,
            reason: None,
        }
    }

    pub fn err(reason: impl Into<String>) -> Self {
        Self {
            ok: false,
            reason: Some(reason.into()),
        }
    }
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

// ── Internal provider alias ────────────────────────────────────────────────────
pub type StarknetProvider = JsonRpcClient<HttpTransport>;

// ── Wallet ────────────────────────────────────────────────────────────────────

/// The primary wallet handle — returned by [`StarkZap::onboard`].
///
/// All methods are `async`. [`Wallet`] is cheaply cloneable via inner `Arc`s.
#[derive(Clone)]
pub struct Wallet<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    pub(crate) account: Arc<SingleOwnerAccount<Arc<P>, Arc<AnySigner>>>,
    pub(crate) provider: Arc<P>,
    pub(crate) signer: Arc<AnySigner>,
    pub(crate) address: Felt,
    pub(crate) network: Network,
    pub(crate) account_preset: AccountPreset,
    pub(crate) rpc_url: String,
}

impl<P> Wallet<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    /// The on-chain address of this wallet.
    pub fn address(&self) -> Felt {
        self.address
    }

    /// The address as a `0x`-prefixed hex string.
    pub fn address_hex(&self) -> String {
        format!("{:#x}", self.address)
    }

    /// Ensure the account is deployed on-chain.
    pub async fn ensure_ready(&self, policy: DeployPolicy) -> Result<()> {
        self.ensure_ready_with_options(
            EnsureReadyOptions {
                deploy: policy,
                fee_mode: None,
            },
            None::<fn(ProgressEvent)>,
        )
        .await
    }

    /// TS-style readiness check with deployment policy and optional progress events.
    pub async fn ensure_ready_with_options<F>(
        &self,
        options: EnsureReadyOptions,
        mut on_progress: Option<F>,
    ) -> Result<()>
    where
        F: FnMut(ProgressEvent),
    {
        let emit = |cb: &mut Option<F>, step| {
            if let Some(callback) = cb.as_mut() {
                callback(ProgressEvent { step });
            }
        };

        emit(&mut on_progress, ProgressStep::Connected);
        emit(&mut on_progress, ProgressStep::CheckDeployed);

        if self.is_deployed().await? {
            debug!(address = %self.address_hex(), "account already deployed");
            emit(&mut on_progress, ProgressStep::Ready);
            return Ok(());
        }

        match options.deploy {
            DeployMode::Never => {
                emit(&mut on_progress, ProgressStep::Failed);
                Err(StarkzapError::NotDeployed)
            }
            DeployMode::IfNeeded | DeployMode::Always => {
                emit(&mut on_progress, ProgressStep::Deploying);
                let result = match options.fee_mode {
                    Some(FeeMode::UserPays) | None => self.deploy_account().await,
                    Some(FeeMode::Paymaster(_)) => Err(StarkzapError::PaymasterUnsupported {
                        feature: "sponsored deployment is not yet supported by the current Rust wallet path".into(),
                    }),
                };

                match result {
                    Ok(()) => {
                        emit(&mut on_progress, ProgressStep::Ready);
                        Ok(())
                    }
                    Err(error) => {
                        emit(&mut on_progress, ProgressStep::Failed);
                        Err(error)
                    }
                }
            }
        }
    }

    /// Returns `true` if the account contract is deployed on-chain.
    ///
    /// Uses `latest` for cross-provider compatibility.
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
    pub async fn balance_of(&self, token: &Token) -> Result<Amount> {
        let selector =
            get_selector_from_name("balanceOf").map_err(|e| StarkzapError::Other(e.to_string()))?;

        let result = self
            .provider
            .call(
                FunctionCall {
                    contract_address: token.address,
                    entry_point_selector: selector,
                    calldata: vec![self.address],
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await
            .map_err(StarkzapError::Provider)?;

        let low = result.first().copied().unwrap_or(Felt::ZERO);
        let high = result.get(1).copied().unwrap_or(Felt::ZERO);

        if high != Felt::ZERO {
            return Err(StarkzapError::AmountOverflow);
        }

        let raw: u128 = low
            .to_biguint()
            .try_into()
            .map_err(|_| StarkzapError::AmountOverflow)?;

        Ok(Amount::from_raw(raw, token))
    }

    /// Transfer tokens to one or more recipients in a single multicall.
    pub async fn transfer(&self, token: &Token, recipients: Vec<Recipient>) -> Result<Tx<P>> {
        if recipients.is_empty() {
            return Err(StarkzapError::Other("No recipients provided".into()));
        }

        let selector =
            get_selector_from_name("transfer").map_err(|e| StarkzapError::Other(e.to_string()))?;

        let calls: Vec<Call> = recipients
            .iter()
            .map(|r| {
                let [low, high] = r.amount.to_u256_felts();
                Call {
                    to: token.address,
                    selector,
                    calldata: vec![r.to, low, high],
                }
            })
            .collect();

        self.execute(calls, FeeMode::UserPays).await
    }

    /// Execute a raw list of Starknet calls atomically.
    pub async fn execute(&self, calls: Vec<Call>, fee_mode: FeeMode) -> Result<Tx<P>> {
        self.execute_with_options(
            calls,
            ExecuteOptions {
                fee_mode: Some(fee_mode),
            },
        )
        .await
    }

    /// Execute a raw list of Starknet calls atomically, mirroring StarkZap TS `ExecuteOptions`.
    pub async fn execute_with_options(
        &self,
        calls: Vec<Call>,
        options: ExecuteOptions,
    ) -> Result<Tx<P>> {
        let fee_mode = options.fee_mode.unwrap_or(FeeMode::UserPays);
        let hash = match fee_mode {
            FeeMode::UserPays => {
                if !self.is_deployed().await? {
                    return Err(StarkzapError::NotDeployed);
                }

                debug!("Executing {} call(s) with user-pays fee", calls.len());
                #[cfg(feature = "cartridge")]
                if let AnySigner::Cartridge(signer) = self.signer.as_ref() {
                    let calls_json = crate::signer::cartridge_signer::calls_to_cartridge_json(&calls)?;
                    signer.execute_via_session(&calls_json)?
                } else if self.account_preset.requires_invoke_v1() {
                    self.execute_user_pays_v1(calls).await?
                } else {
                    let result = self
                        .account
                        .execute_v3(calls)
                        .send()
                        .await
                        .map_err(|e| StarkzapError::Account(e.to_string()))?;
                    result.transaction_hash
                }
                #[cfg(not(feature = "cartridge"))]
                if self.account_preset.requires_invoke_v1() {
                    self.execute_user_pays_v1(calls).await?
                } else {
                    let result = self
                        .account
                        .execute_v3(calls)
                        .send()
                        .await
                        .map_err(|e| StarkzapError::Account(e.to_string()))?;
                    result.transaction_hash
                }
            }

            FeeMode::Paymaster(config) => {
                debug!("Executing {} call(s) via paymaster", calls.len());
                match self
                    .execute_paymaster_transaction(calls.clone(), config.details(), config.api_key.clone())
                    .await
                {
                    Ok(hash) => hash,
                    Err(error) if is_paymaster_compatibility_error(&error.to_string()) => {
                        info!(
                            reason = %error,
                            "paymaster not supported for this account flow; falling back to user-pays execution"
                        );

                        if !self.is_deployed().await? {
                            return Err(StarkzapError::NotDeployed);
                        }

                        if self.account_preset.requires_invoke_v1() {
                            self.execute_user_pays_v1(calls).await?
                        } else {
                            let result = self
                                .account
                                .execute_v3(calls)
                                .send()
                                .await
                                .map_err(|e| StarkzapError::Account(e.to_string()))?;
                            result.transaction_hash
                        }
                    }
                    Err(error) => return Err(error),
                }
            }
        };

        info!(hash = %format!("{:#x}", hash), "transaction submitted");

        Ok(Tx::new(hash, Arc::clone(&self.provider)))
    }

    /// Simulate whether a transaction would succeed without sending it.
    ///
    /// Routes to v1 or v3 simulation based on the account preset so that
    /// old Argent accounts (which require invoke v1) are simulated correctly.
    pub async fn preflight(&self, options: PreflightOptions) -> PreflightResult {
        let fee_mode = options.fee_mode.unwrap_or(FeeMode::UserPays);

        let deployed = match self.is_deployed().await {
            Ok(value) => value,
            Err(error) => return PreflightResult::err(error.to_string()),
        };

        if !deployed {
            return match fee_mode {
                FeeMode::Paymaster(_) => PreflightResult::ok(),
                FeeMode::UserPays => PreflightResult::err("Account not deployed"),
            };
        }

        if self.account_preset.requires_invoke_v1() {
            return match self.preflight_user_pays_v1(options.calls).await {
                Ok(()) => PreflightResult::ok(),
                Err(error) => PreflightResult::err(error.to_string()),
            };
        }

        let simulation = self.account.execute_v3(options.calls).simulate(false, false).await;

        match simulation {
            Ok(simulated) => {
                let reason = match simulated.transaction_trace {
                    TransactionTrace::Invoke(trace) => match trace.execute_invocation {
                        ExecuteInvocation::Reverted(reverted) => Some(reverted.revert_reason),
                        ExecuteInvocation::Success(_) => None,
                    },
                    _ => None,
                };
                match reason {
                    Some(reason) => PreflightResult::err(reason),
                    None => PreflightResult::ok(),
                }
            }
            Err(error) => PreflightResult::err(error.to_string()),
        }
    }

    /// Expose the underlying account, mirroring StarkZap TS.
    pub fn get_account(&self) -> &SingleOwnerAccount<Arc<P>, Arc<AnySigner>> {
        &self.account
    }

    /// Expose the underlying provider, mirroring StarkZap TS.
    pub fn get_provider(&self) -> Arc<P> {
        Arc::clone(&self.provider)
    }

    /// Get the account class hash selected for this wallet.
    pub fn get_class_hash(&self) -> Felt {
        self.account_preset.class_hash()
    }

    /// Get the current network.
    pub fn get_network(&self) -> Network {
        self.network
    }

    /// Build a paymaster-backed transaction, mirroring the TS SDK flow.
    pub async fn build_paymaster_transaction(
        &self,
        calls: Vec<Call>,
        mut details: PaymasterDetails,
        api_key: Option<String>,
    ) -> Result<PreparedPaymasterTransaction> {
        if !self.is_deployed().await? && details.deployment_data.is_none() {
            details.deployment_data = Some(self.paymaster_deployment_data().await?);
        }

        let client = PaymasterClient::new(&self.network, api_key);
        client
            .build_transaction(self.address, calls, details)
            .await
    }

    /// Execute a paymaster-backed transaction, mirroring the TS SDK flow.
    pub async fn execute_paymaster_transaction(
        &self,
        calls: Vec<Call>,
        details: PaymasterDetails,
        api_key: Option<String>,
    ) -> Result<Felt> {
        let prepared = self
            .build_paymaster_transaction(calls, details, api_key.clone())
            .await?;
        let signer = Arc::clone(&self.signer);

        PaymasterClient::new(&self.network, api_key)
            .execute_prepared(self.address, prepared, |hash| async move {
                let sig = signer
                    .sign_hash(&hash)
                    .await
                    .map_err(|e| StarkzapError::Signer(e.to_string()))?;
                Ok(vec![sig.r, sig.s])
            })
            .await
    }

    // ── Private ───────────────────────────────────────────────────────────────

    async fn paymaster_deployment_data(&self) -> Result<AccountDeploymentData> {
        let public_key = self
            .signer
            .get_public_key()
            .await
            .map_err(|e| StarkzapError::DeployFailed(e.to_string()))?
            .scalar();

        Ok(AccountDeploymentData::new(
            self.account_preset.counterfactual_address(public_key),
            self.account_preset.class_hash(),
            self.account_preset.salt(public_key),
            self.account_preset.constructor_calldata(public_key),
        ))
    }

    /// Execute an invoke v1 transaction manually via raw RPC.
    ///
    /// Required for old Argent accounts (pre-0.4.0) that do not accept
    /// STRK-denominated invoke v3 transactions.
    ///
    /// Nonce is fetched at `latest` for broad RPC compatibility.
    async fn execute_user_pays_v1(&self, calls: Vec<Call>) -> Result<Felt> {
        let nonce = self
            .provider
            .get_nonce(BlockId::Tag(BlockTag::Latest), self.address)
            .await
            .map_err(StarkzapError::Provider)?;

        let calldata = self.account.encode_calls(&calls);

        let estimate_signature = self
            .sign_invoke_v1_hash(&calldata, nonce, Felt::ZERO, true)
            .await?;
        let estimate_request =
            self.invoke_v1_request(&calldata, estimate_signature, nonce, Felt::ZERO, true);

        // starknet_estimateFee params are positional (array), per OpenRPC spec:
        //   [0] request          — array of broadcasted txns
        //   [1] simulation_flags — array of flags (empty = default behaviour)
        //   [2] block_id         — block tag object
        //
        // Alchemy v0.9/v0.10 strictly enforces positional params and rejects
        // named-object params with -32602 Invalid params.
        let fee_estimate = self
            .raw_rpc(
                "starknet_estimateFee",
                json!([
                    [estimate_request],
                    [],
                    "latest"
                ]),
            )
            .await?;

        let fee_estimate = fee_estimate
            .as_array()
            .and_then(|items| items.first())
            .cloned()
            .unwrap_or(fee_estimate);
        let overall_fee_hex = fee_estimate["overall_fee"]
            .as_str()
            .ok_or_else(|| StarkzapError::PaymasterMalformed {
                field: "overall_fee".into(),
            })?;
        let overall_fee = Felt::from_hex(overall_fee_hex)
            .map_err(|_| StarkzapError::Other(format!("invalid overall_fee: {overall_fee_hex}")))?;
        let overall_fee_bytes = overall_fee.to_bytes_le();
        if overall_fee_bytes.iter().skip(8).any(|&byte| byte != 0) {
            return Err(StarkzapError::Account("estimated max_fee exceeds u64".into()));
        }
        let overall_fee = u64::from_le_bytes(overall_fee_bytes[..8].try_into().unwrap());
        let max_fee = Felt::from((overall_fee as f64 * 1.1) as u64);

        let signature = self
            .sign_invoke_v1_hash(&calldata, nonce, max_fee, false)
            .await?;
        let request = self.invoke_v1_request(&calldata, signature, nonce, max_fee, false);

        // starknet_addInvokeTransaction params are also positional:
        //   [0] invoke_transaction — the broadcasted invoke txn object
        let result = self
            .raw_rpc(
                "starknet_addInvokeTransaction",
                json!([request]),
            )
            .await?;

        let hash_hex = result["transaction_hash"]
            .as_str()
            .ok_or_else(|| StarkzapError::PaymasterMalformed {
                field: "transaction_hash".into(),
            })?;

        Felt::from_hex(hash_hex)
            .map_err(|_| StarkzapError::Other(format!("invalid transaction hash: {hash_hex}")))
    }

    async fn preflight_user_pays_v1(&self, calls: Vec<Call>) -> Result<()> {
        let nonce = self
            .provider
            .get_nonce(BlockId::Tag(BlockTag::Latest), self.address)
            .await
            .map_err(StarkzapError::Provider)?;
        let calldata = self.account.encode_calls(&calls);
        let estimate_signature = self
            .sign_invoke_v1_hash(&calldata, nonce, Felt::ZERO, true)
            .await?;
        let estimate_request =
            self.invoke_v1_request(&calldata, estimate_signature, nonce, Felt::ZERO, true);

        // Same positional-array params as execute_user_pays_v1.
        self.raw_rpc(
            "starknet_estimateFee",
            json!([
                [estimate_request],
                [],
                "latest"
            ]),
        )
        .await?;

        Ok(())
    }

    async fn sign_invoke_v1_hash(
        &self,
        calldata: &[Felt],
        nonce: Felt,
        max_fee: Felt,
        query_only: bool,
    ) -> Result<Vec<Felt>> {
        const PREFIX_INVOKE: Felt = Felt::from_raw([
            513398556346534256,
            18446744073709551615,
            18446744073709551615,
            18443034532770911073,
        ]);
        const QUERY_VERSION_ONE: Felt = Felt::from_raw([
            576460752142433776,
            18446744073709551584,
            17407,
            18446744073700081633,
        ]);

        let tx_hash = compute_hash_on_elements(&[
            PREFIX_INVOKE,
            if query_only {
                QUERY_VERSION_ONE
            } else {
                Felt::ONE
            },
            self.address,
            Felt::ZERO,
            compute_hash_on_elements(calldata),
            max_fee,
            self.network.chain_id(),
            nonce,
        ]);

        let signature = self
            .signer
            .sign_hash(&tx_hash)
            .await
            .map_err(|e| StarkzapError::Signer(e.to_string()))?;

        Ok(vec![signature.r, signature.s])
    }

    fn invoke_v1_request(
        &self,
        calldata: &[Felt],
        signature: Vec<Felt>,
        nonce: Felt,
        max_fee: Felt,
        query_only: bool,
    ) -> Value {
        const QUERY_VERSION_ONE: Felt = Felt::from_raw([
            576460752142433776,
            18446744073709551584,
            17407,
            18446744073700081633,
        ]);

        json!({
            "type": "INVOKE",
            "sender_address": format!("{:#x}", self.address),
            "calldata": calldata.iter().map(|felt| format!("{:#x}", felt)).collect::<Vec<_>>(),
            "signature": signature.iter().map(|felt| format!("{:#x}", felt)).collect::<Vec<_>>(),
            "nonce": format!("{:#x}", nonce),
            "max_fee": format!("{:#x}", max_fee),
            "version": if query_only {
                format!("{:#x}", QUERY_VERSION_ONE)
            } else {
                "0x1".to_string()
            },
        })
    }

    /// Send a raw JSON-RPC request.
    ///
    /// `params` MUST be a JSON array (positional params per OpenRPC spec).
    /// Alchemy v0.9/v0.10 rejects named-object params with -32602.
    async fn raw_rpc(&self, method: &str, params: Value) -> Result<Value> {
        debug_assert!(
            params.is_array(),
            "raw_rpc: params must be a JSON array (positional), got object — \
             this will cause -32602 Invalid params on Alchemy v0.9+"
        );

        let client = {
            let builder = Client::builder();
            #[cfg(not(target_arch = "wasm32"))]
            let builder = builder.http1_only();
            builder
        };

        let response = client
            .build()
            .map_err(StarkzapError::Http)?
            .post(&self.rpc_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params,
            }))
            .send()
            .await
            .map_err(StarkzapError::Http)?;
        let value: Value = response.json().await.map_err(StarkzapError::Http)?;

        if let Some(error) = value.get("error") {
            let code = error.get("code").and_then(Value::as_i64);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown RPC error");

            if method == "starknet_addInvokeTransaction" && code == Some(61) {
                return Err(StarkzapError::Other(
                    "this deployed Argent account requires legacy invoke v1 compatibility, but the configured RPC backend rejected transaction version 0x1; use an RPC that still accepts invoke v1 for older Argent accounts, or migrate the account to a newer contract version".into(),
                ));
            }

            return Err(StarkzapError::Other(format!(
                "RPC {method} failed ({code:?}): {message}"
            )));
        }

        value.get("result")
            .cloned()
            .ok_or_else(|| StarkzapError::Other(format!("RPC {method} returned no result")))
    }

    async fn deploy_account(&self) -> Result<()> {
        let public_key = self
            .signer
            .get_public_key()
            .await
            .map_err(|e| StarkzapError::DeployFailed(e.to_string()))?
            .scalar();
        let expected_address = self.account_preset.counterfactual_address(public_key);

        if self.address != expected_address {
            return Err(StarkzapError::AddressMismatch {
                provided: format!("{:#x}", self.address),
                expected: format!("{:#x}", expected_address),
            });
        }

        let factory = PresetAccountFactory::new(
            self.account_preset,
            self.network.chain_id(),
            Arc::clone(&self.signer),
            Arc::clone(&self.provider),
        )
        .await
        .map_err(|e| StarkzapError::DeployFailed(e.to_string()))?;

        let salt = self.account_preset.salt(public_key);
        let result = match factory.deploy_v3(salt).send().await {
            Ok(result) => result,
            Err(error) => {
                let message = error.to_string();
                if message.to_lowercase().contains("already deployed")
                    || message.to_lowercase().contains("already exists")
                {
                    return Ok(());
                }
                return Err(StarkzapError::DeployFailed(message));
            }
        };

        Tx::new(result.transaction_hash, Arc::clone(&self.provider))
            .wait()
            .await
            .map_err(|e| StarkzapError::DeployFailed(e.to_string()))?;

        Ok(())
    }
}

fn is_paymaster_compatibility_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("invalid version")
        || lower.contains("snip-9")
        || lower.contains("src9")
        || lower.contains("outside execution")
        || lower.contains("not compatible")
}

impl<P> std::fmt::Debug for Wallet<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet")
            .field("address", &self.address_hex())
            .field("network", &self.network)
            .finish()
    }
}
