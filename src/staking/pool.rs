//! Delegation pool entry and exit operations.
//!
//! All operations follow the Starknet native staking protocol:
//! <https://docs.starknet.io/staking/>

use starknet::{
    core::{types::Call, utils::get_selector_from_name},
    core::types::Felt,
};

use crate::{
    amount::Amount,
    error::{Result, StarkzapError},
    paymaster::FeeMode,
    tokens::Token,
    tx::Tx,
    wallet::Wallet,
};

impl Wallet {
    /// Enter a delegation pool (stake tokens).
    ///
    /// This batches two calls atomically:
    /// 1. ERC-20 `approve(pool_contract, amount)` — authorise the pool to take tokens
    /// 2. `enter_delegation_pool(reward_address, amount)` — delegate stake
    ///
    /// # Arguments
    ///
    /// * `token` — the staking token (STRK on mainnet)
    /// * `pool_contract` — the pool contract address (from validator presets)
    /// * `amount` — the amount to stake
    /// * `reward_address` — where staking rewards will be sent (usually `self.address()`)
    /// * `fee_mode` — fee payment strategy
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use starkzap_rs::{Amount, paymaster::FeeMode, staking::presets::mainnet_validators, tokens::mainnet};
    ///
    /// let strk = mainnet::strk();
    /// let validator = &mainnet_validators()[0];
    /// let amount = Amount::parse("100", &strk)?;
    ///
    /// let tx = wallet.enter_pool(&strk, validator.pool_address, amount, wallet.address(), FeeMode::UserPays).await?;
    /// tx.wait().await?;
    /// ```
    pub async fn enter_pool(
        &self,
        token: &Token,
        pool_contract: Felt,
        amount: Amount,
        reward_address: Felt,
        fee_mode: FeeMode,
    ) -> Result<Tx> {
        let [amount_low, amount_high] = amount.to_u256_felts();

        let approve_call = approve_call(token.address, pool_contract, amount_low, amount_high)?;

        let enter_selector = get_selector_from_name("enter_delegation_pool")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let enter_call = Call {
            to: pool_contract,
            selector: enter_selector,
            calldata: vec![reward_address, amount_low, amount_high],
        };

        self.execute(vec![approve_call, enter_call], fee_mode).await
    }

    /// Add more tokens to an existing delegation pool position.
    ///
    /// Requires an active position (you must have already called [`enter_pool`]).
    ///
    /// Batches: `approve` + `add_to_delegation_pool`
    pub async fn add_to_pool(
        &self,
        token: &Token,
        pool_contract: Felt,
        amount: Amount,
        fee_mode: FeeMode,
    ) -> Result<Tx> {
        let [amount_low, amount_high] = amount.to_u256_felts();

        let approve_call = approve_call(token.address, pool_contract, amount_low, amount_high)?;

        let add_selector = get_selector_from_name("add_to_delegation_pool")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let add_call = Call {
            to: pool_contract,
            selector: add_selector,
            calldata: vec![self.address, amount_low, amount_high],
        };

        self.execute(vec![approve_call, add_call], fee_mode).await
    }

    /// Signal intent to exit a delegation pool.
    ///
    /// This does **not** immediately return tokens. Starknet staking has a
    /// cooldown period (currently ~21 days on mainnet). After the cooldown,
    /// call [`exit_pool`] to claim the tokens.
    ///
    /// # Arguments
    ///
    /// * `pool_contract` — the pool contract address
    /// * `amount` — the amount to unstake (pass full position to unstake completely)
    pub async fn exit_pool_intent(
        &self,
        pool_contract: Felt,
        amount: Amount,
        fee_mode: FeeMode,
    ) -> Result<Tx> {
        let [amount_low, amount_high] = amount.to_u256_felts();

        let selector = get_selector_from_name("exit_delegation_pool_intent")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let call = Call {
            to: pool_contract,
            selector,
            calldata: vec![amount_low, amount_high],
        };

        self.execute(vec![call], fee_mode).await
    }

    /// Finalise exit from a delegation pool after the cooldown period.
    ///
    /// Can only be called after [`exit_pool_intent`] and the cooldown has elapsed.
    /// Tokens are returned to the wallet.
    pub async fn exit_pool(&self, pool_contract: Felt, fee_mode: FeeMode) -> Result<Tx> {
        let selector = get_selector_from_name("exit_delegation_pool_action")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let call = Call {
            to: pool_contract,
            selector,
            calldata: vec![self.address],
        };

        self.execute(vec![call], fee_mode).await
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn approve_call(
    token_address: Felt,
    spender: Felt,
    amount_low: Felt,
    amount_high: Felt,
) -> Result<Call> {
    let selector = get_selector_from_name("approve")
        .map_err(|e| StarkzapError::Staking(e.to_string()))?;

    Ok(Call {
        to: token_address,
        selector,
        calldata: vec![spender, amount_low, amount_high],
    })
}