/// ```rust,no_run
    /// # use starkzap_rs::{OnboardConfig, StarkZap, StarkZapConfig,
    /// #     signer::StarkSigner, staking::presets::mainnet_validators};
    /// # async fn example() -> starkzap_rs::error::Result<()> {
    /// # let sdk = StarkZap::new(StarkZapConfig::mainnet());
    /// # let signer = StarkSigner::new("0xprivkey", "0xaddress")?;
    /// # let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    /// let staker_addrs: Vec<_> = mainnet_validators()
    ///     .into_iter()
    ///     .map(|v| v.staker_address)
    ///     .collect();
    ///
    /// let pools = wallet.discover_my_pools(staker_addrs).await?;
    /// # Ok(())
    /// # }
    /// ```
    
use starknet::{
    core::{
        types::{BlockId, BlockTag, Felt, FunctionCall},
        utils::get_selector_from_name,
    },
    providers::Provider,
};
use tokio::time::{Duration, sleep};

use crate::{
    error::{Result, StarkzapError},
    wallet::Wallet,
};

/// A discovered pool contract address for a given staker.
#[derive(Debug, Clone)]
pub struct DiscoveredPool {
    /// The pool's contract address.
    pub address: Felt,
}

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

impl<P> Wallet<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    /// Discover all delegation pools for a given staker address.
    ///
    /// Calls `staker_pool_info` on the Starknet staking contract.
    ///
    /// # Arguments
    ///
    /// * `staker_address` — the operator/staker whose pools you want to find
    ///
    /// # Errors
    ///
    /// Returns [`StarkzapError::NoPoolsFound`] if the staker has no active pools.
    pub async fn get_staker_pools(&self, staker_address: Felt) -> Result<Vec<DiscoveredPool>> {
        let staking_contract = self.network.staking_contract();

        let selector = get_selector_from_name("staker_pool_info")
            .map_err(|e| StarkzapError::Staking(e.to_string()))?;

        let result = provider_call_with_retry(
            self.provider.as_ref(),
            FunctionCall {
                contract_address: staking_contract,
                entry_point_selector: selector,
                calldata: vec![staker_address],
            },
        )
            .await
            .map_err(StarkzapError::Provider)?;

        if result.is_empty() {
            return Err(StarkzapError::NoPoolsFound {
                address: format!("{:#x}", staker_address),
            });
        }

        // `staker_pool_info` returns:
        //   StakerPoolInfoV2 {
        //     commission: Option<u16>,
        //     pools: Span<PoolInfo>,
        //   }
        //
        // Cairo ABI flattening here is:
        // - `Option<u16>` => [tag] or [tag, value]
        // - `Span<PoolInfo>` => [len, pool0..., pool1...]
        // - `PoolInfo` can flatten as:
        //   - [pool_contract, token_address, amount] on mainnet today
        //   - [pool_contract, token_address, amount_low, amount_high] on other layouts
        let (len_index, pools_start) = if result[0] == Felt::ZERO {
            (2usize, 3usize) // commission = Some(value)
        } else {
            (1usize, 2usize) // commission = None
        };

        let len: usize = result
            .get(len_index)
            .ok_or_else(|| StarkzapError::PaymasterMalformed {
                field: "staking pools length".into(),
            })?
            .to_biguint()
            .try_into()
            .unwrap_or(0usize);

        let remaining = result.len().saturating_sub(pools_start);
        let stride = match len {
            0 => 0,
            _ if remaining == len * 3 => 3,
            _ if remaining == len * 4 => 4,
            _ => {
                return Err(StarkzapError::Staking(format!(
                    "malformed staker_pool_info response: expected {} pool entries with stride 3 or 4, got {} trailing felts",
                    len, remaining
                )));
            }
        };

        let mut pools = Vec::with_capacity(len);
        for index in 0..len {
            let base = pools_start + (index * stride);
            let address = *result.get(base).ok_or_else(|| StarkzapError::Staking(
                "malformed staker_pool_info response".into(),
            ))?;
            pools.push(DiscoveredPool { address });
        }

        if pools.is_empty() {
            return Err(StarkzapError::NoPoolsFound {
                address: format!("{:#x}", staker_address),
            });
        }

        Ok(pools)
    }

    /// Discover all pools the current wallet has a position in.
    ///
    /// Iterates the provided list of known staker addresses and queries each.
    /// Use the validator presets as input:
    ///
    /// ```rust,no_run
    /// # use starkzap_rs::{OnboardConfig, StarkZap, StarkZapConfig,
    /// #     signer::StarkSigner, staking::presets::mainnet_validators};
    /// # async fn example() -> starkzap_rs::error::Result<()> {
    /// # let sdk = StarkZap::new(StarkZapConfig::mainnet());
    /// # let signer = StarkSigner::new("0xprivkey", "0xaddress")?;
    /// # let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    /// let staker_addrs: Vec<_> = mainnet_validators()
    ///     .into_iter()
    ///     .map(|v| v.staker_address)
    ///     .collect();
    ///
    /// let pools = wallet.discover_my_pools(staker_addrs).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn discover_my_pools(
        &self,
        staker_addresses: Vec<Felt>,
    ) -> Result<Vec<DiscoveredPool>> {
        let mut all = Vec::new();
        for staker in staker_addresses {
            match self.get_staker_pools(staker).await {
                Ok(pools) => all.extend(pools),
                Err(StarkzapError::NoPoolsFound { .. }) => {} // not an error — staker has no pools
                Err(e) => return Err(e),
            }
        }
        Ok(all)
    }
}
