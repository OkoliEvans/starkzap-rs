/// ```rust,no_run
    /// # use starkzap_rs::{OnboardConfig, StarkZap, StarkZapConfig,
    /// #     signer::StarkSigner, staking::presets::mainnet_validators,
    /// #     tokens::mainnet};
    /// # async fn example() -> starkzap_rs::error::Result<()> {
    /// # let sdk = StarkZap::new(StarkZapConfig::mainnet());
    /// # let signer = StarkSigner::new("0xprivkey", "0xaddress")?;
    /// # let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    /// # let validator = &mainnet_validators()[0];
    /// let strk = mainnet::strk();
    ///
    /// let pool = wallet.get_staker_pools(validator.staker_address).await?[0].address;
    /// let pos = wallet.get_pool_position(pool, &strk).await?;
    /// println!("Staked: {}", pos.staked);
    /// println!("Rewards: {}", pos.rewards);
    /// # Ok(())
    /// # }
    /// ```
 
use starknet::{
    core::{
        types::{BlockId, BlockTag, Call, Felt, FunctionCall},
        utils::get_selector_from_name,
    },
    providers::Provider,
};
use tokio::time::{Duration, sleep};

use crate::{
    amount::Amount,
    error::{Result, StarkzapError},
    paymaster::FeeMode,
    tokens::Token,
    tx::Tx,
    wallet::Wallet,
};

async fn provider_call_with_retry<P>(
    provider: &P,
    call: FunctionCall,
) -> std::result::Result<Vec<Felt>, starknet::providers::ProviderError>
where
    P: Provider + Send + Sync,
{
    let mut attempts = 0usize;

    loop {
        match provider.call(call.clone(), BlockId::Tag(BlockTag::Latest)).await {
            Ok(result) => return Ok(result),
            Err(error) if attempts < 2 && should_retry_provider_error(&error) => {
                attempts += 1;
                sleep(Duration::from_millis(250 * attempts as u64)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn should_retry_provider_error(error: &starknet::providers::ProviderError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("dns error")
        || message.contains("transporterror")
        || message.contains("connection reset")
        || message.contains("connection refused")
        || message.contains("timed out")
}

/// The current staking position for a delegator in a given pool.
#[derive(Debug, Clone)]
pub struct PoolPosition {
    /// Amount currently staked (in smallest token unit).
    pub staked: Amount,
    /// Accumulated rewards not yet claimed (in smallest token unit).
    pub rewards: Amount,
    /// Pool contract address.
    pub pool_address: Felt,
}

impl PoolPosition {
    /// Returns `true` if there is no staked balance.
    pub fn is_empty(&self) -> bool {
        self.staked.is_zero()
    }
}

impl<P> Wallet<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    /// Query the current staking position in a pool.
    ///
    /// Calls `get_pool_member_info_v1` on the pool contract, which returns the
    /// staked amount and accumulated rewards for this wallet address.
    ///
    /// # Arguments
    ///
    /// * `pool_contract` — the pool contract address
    /// * `token` — the staking token (needed to construct `Amount`)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use starkzap_rs::{OnboardConfig, StarkZap, StarkZapConfig,
    /// #     signer::StarkSigner, staking::presets::mainnet_validators,
    /// #     tokens::mainnet};
    /// # async fn example() -> starkzap_rs::error::Result<()> {
    /// # let sdk = StarkZap::new(StarkZapConfig::mainnet());
    /// # let signer = StarkSigner::new("0xprivkey", "0xaddress")?;
    /// # let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    /// # let validator = &mainnet_validators()[0];
    /// let strk = mainnet::strk();
    ///
    /// let pool = wallet.get_staker_pools(validator.staker_address).await?[0].address;
    /// let pos = wallet.get_pool_position(pool, &strk).await?;
    /// println!("Staked: {}", pos.staked);
    /// println!("Rewards: {}", pos.rewards);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_pool_position(
        &self,
        pool_contract: Felt,
        token: &Token,
    ) -> Result<PoolPosition> {
        let selector = get_selector_from_name("get_pool_member_info_v1")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let result = provider_call_with_retry(
            self.provider.as_ref(),
            FunctionCall {
                contract_address: pool_contract,
                entry_point_selector: selector,
                calldata: vec![self.address],
            },
        )
            .await
            .map_err(StarkzapError::Provider)?;

        // `get_pool_member_info_v1` returns:
        //   Option<PoolMemberInfoV1>
        //
        // ABI flattening:
        // - Some => [0, reward_address, amount_low, amount_high, rewards_low, rewards_high, commission, unpool_low, unpool_high, unpool_time_tag, ...]
        // - None => [1]
        if result.first().copied().unwrap_or(Felt::ONE) != Felt::ZERO {
            return Ok(PoolPosition {
                staked: Amount::from_raw(0, token),
                rewards: Amount::from_raw(0, token),
                pool_address: pool_contract,
            });
        }

        let staked_low = result.get(2).copied().unwrap_or(Felt::ZERO);
        let rewards_low = result.get(4).copied().unwrap_or(Felt::ZERO);

        let staked_raw: u128 = staked_low
            .to_biguint()
            .try_into()
            .map_err(|_| StarkzapError::AmountOverflow)?;

        let rewards_raw: u128 = rewards_low
            .to_biguint()
            .try_into()
            .map_err(|_| StarkzapError::AmountOverflow)?;

        Ok(PoolPosition {
            staked: Amount::from_raw(staked_raw, token),
            rewards: Amount::from_raw(rewards_raw, token),
            pool_address: pool_contract,
        })
    }

    /// Claim accumulated staking rewards from a pool.
    ///
    /// Rewards are sent to the `reward_address` specified when you first entered
    /// the pool. This does not affect the staked principal.
    ///
    /// # Arguments
    ///
    /// * `pool_contract` — the pool contract address
    /// * `fee_mode` — fee payment strategy
    pub async fn claim_rewards(&self, pool_contract: Felt, fee_mode: FeeMode) -> Result<Tx<P>> {
        let selector = get_selector_from_name("claim_rewards")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let call = Call {
            to: pool_contract,
            selector,
            calldata: vec![self.address],
        };

        self.execute(vec![call], fee_mode).await
    }
}
