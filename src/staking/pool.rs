/// ```rust,no_run
    /// # use starkzap_rs::{Amount, OnboardConfig, StarkZap, StarkZapConfig,
    /// #     paymaster::FeeMode, signer::StarkSigner, staking::presets::mainnet_validators,
    /// #     tokens::mainnet};
    /// # async fn example() -> starkzap_rs::error::Result<()> {
    /// # let sdk = StarkZap::new(StarkZapConfig::mainnet());
    /// # let signer = StarkSigner::new("0xprivkey", "0xaddress")?;
    /// # let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    /// let strk = mainnet::strk();
    /// let validator = &mainnet_validators()[0];
    /// let pool = wallet.get_staker_pools(validator.staker_address).await?[0].address;
    /// let amount = Amount::parse("100", &strk)?;
    ///
    /// let tx = wallet
    ///     .enter_pool(&strk, pool, amount, wallet.address(), FeeMode::UserPays)
    ///     .await?;
    /// tx.wait().await?;
    /// # Ok(())
    /// # }
    /// ```

use starknet::{
    core::{types::Call, utils::get_selector_from_name},
    core::types::Felt,
    providers::Provider,
};

use crate::{
    amount::Amount,
    error::{Result, StarkzapError},
    paymaster::FeeMode,
    tokens::Token,
    tx::Tx,
    wallet::Wallet,
};

impl<P> Wallet<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
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
    /// # use starkzap_rs::{Amount, OnboardConfig, StarkZap, StarkZapConfig,
    /// #     paymaster::FeeMode, signer::StarkSigner, staking::presets::mainnet_validators,
    /// #     tokens::mainnet};
    /// # async fn example() -> starkzap_rs::error::Result<()> {
    /// # let sdk = StarkZap::new(StarkZapConfig::mainnet());
    /// # let signer = StarkSigner::new("0xprivkey", "0xaddress")?;
    /// # let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    /// let strk = mainnet::strk();
    /// let validator = &mainnet_validators()[0];
    /// let pool = wallet.get_staker_pools(validator.staker_address).await?[0].address;
    /// let amount = Amount::parse("100", &strk)?;
    ///
    /// let tx = wallet
    ///     .enter_pool(&strk, pool, amount, wallet.address(), FeeMode::UserPays)
    ///     .await?;
    /// tx.wait().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn enter_pool(
        &self,
        token: &Token,
        pool_contract: Felt,
        amount: Amount,
        reward_address: Felt,
        fee_mode: FeeMode,
    ) -> Result<Tx<P>> {
        let [amount_low, amount_high] = amount.to_u256_felts();
        let staking_amount = felt_to_u128(amount_low)?;
        if amount_high != Felt::ZERO {
            return Err(StarkzapError::AmountOverflow);
        }

        let approve_call = approve_call(token.address, pool_contract, amount_low, amount_high)?;

        let enter_selector = get_selector_from_name("enter_delegation_pool")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let enter_call = Call {
            to: pool_contract,
            selector: enter_selector,
            calldata: vec![reward_address, staking_amount],
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
    ) -> Result<Tx<P>> {
        let [amount_low, amount_high] = amount.to_u256_felts();
        let staking_amount = felt_to_u128(amount_low)?;
        if amount_high != Felt::ZERO {
            return Err(StarkzapError::AmountOverflow);
        }

        let approve_call = approve_call(token.address, pool_contract, amount_low, amount_high)?;

        let add_selector = get_selector_from_name("add_to_delegation_pool")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let add_call = Call {
            to: pool_contract,
            selector: add_selector,
            calldata: vec![self.address, staking_amount],
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
    ) -> Result<Tx<P>> {
        let [amount_low, amount_high] = amount.to_u256_felts();
        let staking_amount = felt_to_u128(amount_low)?;
        if amount_high != Felt::ZERO {
            return Err(StarkzapError::AmountOverflow);
        }

        let selector = get_selector_from_name("exit_delegation_pool_intent")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let call = Call {
            to: pool_contract,
            selector,
            calldata: vec![staking_amount],
        };

        self.execute(vec![call], fee_mode).await
    }

    /// Finalise exit from a delegation pool after the cooldown period.
    ///
    /// Can only be called after [`exit_pool_intent`] and the cooldown has elapsed.
    /// Tokens are returned to the wallet.
    pub async fn exit_pool(&self, pool_contract: Felt, fee_mode: FeeMode) -> Result<Tx<P>> {
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

fn felt_to_u128(value: Felt) -> Result<Felt> {
    let raw: u128 = value
        .to_biguint()
        .try_into()
        .map_err(|_| StarkzapError::AmountOverflow)?;
    Ok(Felt::from(raw))
}
