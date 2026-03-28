//! Staking position queries and reward claiming.

use starknet::{
    core::{
        types::{BlockId, BlockTag, Call, Felt, FunctionCall},
        utils::get_selector_from_name,
    },
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

impl Wallet {
    /// Query the current staking position in a pool.
    ///
    /// Calls `pooler_info` on the pool contract, which returns the staked
    /// amount and accumulated rewards for this wallet address.
    ///
    /// # Arguments
    ///
    /// * `pool_contract` — the pool contract address
    /// * `token` — the staking token (needed to construct `Amount`)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let pos = wallet.get_pool_position(validator.pool_address, &strk).await?;
    /// println!("Staked: {}", pos.staked);
    /// println!("Rewards: {}", pos.rewards);
    /// ```
    pub async fn get_pool_position(&self, pool_contract: Felt, token: &Token) -> Result<PoolPosition> {
        let selector = get_selector_from_name("pooler_info")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let result = self
            .provider
            .call(
                FunctionCall {
                    contract_address: pool_contract,
                    entry_point_selector: selector,
                    calldata: vec![self.address],
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await
            .map_err(StarkzapError::Provider)?;

        // pooler_info returns:
        //   [staked_low, staked_high, rewards_low, rewards_high, ...]
        let staked_low = result.get(0).copied().unwrap_or(Felt::ZERO);
        let rewards_low = result.get(2).copied().unwrap_or(Felt::ZERO);

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
    pub async fn claim_rewards(&self, pool_contract: Felt, fee_mode: FeeMode) -> Result<Tx> {
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