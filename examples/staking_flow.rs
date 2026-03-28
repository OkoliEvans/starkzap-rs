//! Staking lifecycle example — Sepolia testnet.
//!
//! Demonstrates the full staking flow:
//!   1. Discover available pools
//!   2. Enter a pool (stake)
//!   3. Check position
//!   4. Signal exit intent
//!   5. Claim rewards
//!
//! ⚠️  exitPool() (step 4 finalisation) requires waiting through the cooldown
//! period (~21 days on mainnet). This example only demonstrates `exit_pool_intent`.
//!
//! # Setup
//!
//! ```sh
//! cargo run --example staking_flow
//! ```

use dotenvy::dotenv;
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
        .with_env_filter("starkzap_rs=debug,info")
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
    info!("Available validators on Sepolia:");
    for v in &validators {
        info!("  {} — pool: {:#x}", v.name, v.pool_address);
    }

    let validator = &validators[0];
    let pool = validator.pool_address;
    info!("Using validator: {}", validator.name);

    // ── 2. Enter pool (stake) ─────────────────────────────────────────────────
    let stake_amount = Amount::parse("10", &strk)?;
    info!("Staking {}", stake_amount);

    let reward_address = wallet.address(); // rewards sent to self
    let tx = wallet
        .enter_pool(&strk, pool, stake_amount, reward_address, FeeMode::UserPays)
        .await?;
    info!("Stake tx submitted: {}", tx);
    tx.wait().await?;
    info!("Stake confirmed ✓");

    // ── 3. Check position ─────────────────────────────────────────────────────
    let pos = wallet.get_pool_position(pool, &strk).await?;
    info!("Staked:  {}", pos.staked);
    info!("Rewards: {}", pos.rewards);

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
    //
    // This starts the cooldown. Actual token return requires calling
    // wallet.exit_pool(pool, ...) after the cooldown period.
    info!("Signalling exit intent for {} STRK", pos.staked);
    let tx = wallet
        .exit_pool_intent(pool, pos.staked, FeeMode::UserPays)
        .await?;
    tx.wait().await?;
    info!("Exit intent submitted ✓");
    info!("Wait for the cooldown period, then call wallet.exit_pool() to finalise.");

    Ok(())
}