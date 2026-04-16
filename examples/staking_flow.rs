//! Staking lifecycle example — Sepolia testnet.
//!
//! Demonstrates the staking flow:
//!   1. Discover available pools
//!   2. Inspect your current position
//!   3. Optionally execute stake / claim / exit-intent writes
//!
//! ⚠️  By default this example is read-only. To perform real staking writes, set:
//! `RUN_STAKING_WRITES=1`
//!
//! `exit_pool_intent` starts the cooldown period. Do not enable writes unless
//! you intend to change the wallet's staking position.
//!
//! # Setup
//!
//! ```sh
//! cargo run --example staking_flow
//!
//! # To allow writes
//! RUN_STAKING_WRITES=1 cargo run --example staking_flow
//! ```

use dotenvy::dotenv;
use starknet::core::types::Felt;
use starkzap_rs::{
    Amount, OnboardConfig, StarkZap, StarkZapConfig,
    paymaster::FeeMode,
    signer::StarkSigner,
    staking::presets::sepolia_validators,
    tokens::sepolia,
};
use tracing::info;

#[tokio::main]
async fn main() -> starkzap_rs::error::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("info,starkzap_rs=info")
        .init();

    let sdk = StarkZap::new(StarkZapConfig::sepolia());

    let signer = StarkSigner::new(
        &std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set"),
        &std::env::var("ACCOUNT_ADDRESS").expect("ACCOUNT_ADDRESS not set"),
    )?;

    let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    info!("Wallet: {}", wallet.address_hex());

    let strk = sepolia::strk();

    // ── 1. Discover pools ─────────────────────────────────────────────────────
    let validators = sepolia_validators();
    info!("Available validator stakers on Sepolia:");
    for v in &validators {
        info!("  {} — staker: {:#x}", v.name, v.staker_address);
    }

    let validator = &validators[0];
    let pool = match std::env::var("STAKING_POOL_ADDRESS") {
        Ok(pool) => Felt::from_hex(&pool).map_err(|_| {
            starkzap_rs::StarkzapError::Other("invalid STAKING_POOL_ADDRESS".into())
        })?,
        Err(_) => {
            let pools = wallet.get_staker_pools(validator.staker_address).await?;
            pools.first().map(|pool| pool.address).ok_or_else(|| {
                starkzap_rs::StarkzapError::Other("validator has no active pools".into())
            })?
        }
    };
    info!("Using validator: {} (pool {:#x})", validator.name, pool);

    // ── 2. Check position ─────────────────────────────────────────────────────
    let pos = wallet.get_pool_position(pool, &strk).await?;
    info!("Staked:  {}", pos.staked);
    info!("Rewards: {}", pos.rewards);

    let run_writes = std::env::var("RUN_STAKING_WRITES").ok().as_deref() == Some("1");
    if !run_writes {
        info!("Read-only mode: skipping stake / claim / exit-intent writes.");
        info!("Set RUN_STAKING_WRITES=1 to execute real staking transactions.");
        return Ok(());
    }

    // ── 3. Enter pool (stake) ─────────────────────────────────────────────────
    let stake_amount = Amount::parse("10", &strk)?;
    info!("Staking {}", stake_amount);

    let reward_address = wallet.address(); // rewards sent to self
    let tx = wallet
        .enter_pool(&strk, pool, stake_amount, reward_address, FeeMode::UserPays)
        .await?;
    info!("Stake tx submitted: {}", tx);
    tx.wait().await?;
    info!("Stake confirmed ✓");

    let pos = wallet.get_pool_position(pool, &strk).await?;
    info!("Updated staked:  {}", pos.staked);
    info!("Updated rewards: {}", pos.rewards);

    // ── 4. Claim rewards (if any) ─────────────────────────────────────────────
    if !pos.rewards.is_zero() {
        info!("Claiming rewards: {}", pos.rewards);
        let tx = wallet.claim_rewards(pool, FeeMode::UserPays).await?;
        tx.wait().await?;
        info!("Rewards claimed ✓");
    } else {
        info!("No rewards to claim yet (rewards accrue over time)");
    }

    // ── 5. Signal exit intent ─────────────────────────────────────────────────
    if !pos.staked.is_zero() {
        info!("Signalling exit intent for {} STRK", pos.staked);
        let tx = wallet
            .exit_pool_intent(pool, pos.staked, FeeMode::UserPays)
            .await?;
        tx.wait().await?;
        info!("Exit intent submitted ✓");
        info!("Wait for the cooldown period, then call wallet.exit_pool() to finalise.");
    }

    Ok(())
}
