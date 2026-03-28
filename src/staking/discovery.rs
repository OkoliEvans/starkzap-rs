//! Pool and validator discovery — find all pools for a given staker.

use starknet::{
    core::{
        types::{BlockId, BlockTag, Felt, FunctionCall},
        utils::get_selector_from_name,
    },
    providers::Provider,
};

use crate::{
    error::{Result, StarkzapError},
    network::Network,
    wallet::Wallet,
};

/// A discovered pool contract address for a given staker.
#[derive(Debug, Clone)]
pub struct DiscoveredPool {
    /// The pool's contract address.
    pub address: Felt,
}

impl Wallet {
    /// Discover all delegation pools for a given staker address.
    ///
    /// Calls `get_staker_pools` on the Starknet staking contract.
    ///
    /// # Arguments
    ///
    /// * `staker_address` — the operator/staker whose pools you want to find
    ///
    /// # Errors
    ///
    /// Returns [`StarkzapError::NoPoolsFound`] if the staker has no active pools.
    pub async fn get_staker_pools(
        &self,
        staker_address: Felt,
    ) -> Result<Vec<DiscoveredPool>> {
        let staking_contract = self.network.staking_contract();

        let selector = get_selector_from_name("get_staker_pools")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let result = self
            .provider
            .call(
                FunctionCall {
                    contract_address: staking_contract,
                    entry_point_selector: selector,
                    calldata: vec![staker_address],
                },
                BlockId::Tag(BlockTag::Latest),
            )
            .await
            .map_err(StarkzapError::Provider)?;

        if result.is_empty() {
            return Err(StarkzapError::NoPoolsFound {
                address: format!("{:#x}", staker_address),
            });
        }

        // The return is an array of pool addresses: [len, addr0, addr1, ...]
        let len = result[0]
            .to_biguint()
            .try_into()
            .unwrap_or(0usize);

        let pools = result
            .iter()
            .skip(1)
            .take(len)
            .map(|&address| DiscoveredPool { address })
            .collect();

        Ok(pools)
    }

    /// Discover all pools the current wallet has a position in.
    ///
    /// Iterates the provided list of known staker addresses and queries each.
    /// Use the validator presets as input:
    ///
    /// ```rust,no_run
    /// use starkzap_rs::staking::presets::mainnet_validators;
    ///
    /// let staker_addrs: Vec<_> = mainnet_validators()
    ///     .into_iter()
    ///     .map(|v| v.staker_address)
    ///     .collect();
    ///
    /// let pools = wallet.discover_my_pools(staker_addrs).await?;
    /// ```
    pub async fn discover_my_pools(
        &self,
        staker_addresses: Vec<Felt>,
    ) -> Result<Vec<DiscoveredPool>> {
        let mut all = Vec::new();
        for staker in staker_addresses {
            match self.get_staker_pools(staker).await {
                Ok(pools) => all.extend(pools),
                Err(StarkzapError::NoPoolsFound { .. }) => {} // skip, not an error
                Err(e) => return Err(e),
            }
        }
        Ok(all)
    }
}