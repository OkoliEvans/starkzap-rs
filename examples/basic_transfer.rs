//! Basic transfer example — Sepolia testnet.
//!
//! Demonstrates the minimal happy path:
//! connect a wallet → check balance → transfer STRK to a recipient.
//!
//! # Setup
//!
//! 1. Copy `.env.example` to `.env` and fill in your values.
//! 2. Ensure your Sepolia account has STRK and ETH for gas.
//! 3. Run:
//!
//! ```sh
//! cargo run --example basic_transfer
//! ```

use dotenvy::dotenv;
use starknet::core::types::Felt;
use starkzap_rs::{
    Amount, OnboardConfig, Recipient, StarkZap, StarkZapConfig,
    signer::StarkSigner,
    tokens::sepolia,
    wallet::DeployPolicy,
};
use tracing::info;

#[tokio::main]
async fn main() -> starkzap_rs::error::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("starkzap_rs=debug,info")
        .init();

    // ── 1. Initialise SDK ─────────────────────────────────────────────────────
    //
    // Uses Sepolia with the default public BlastAPI endpoint.
    // To use Alchemy: StarkZapConfig::sepolia().with_rpc(
    //   "https://starknet-sepolia.g.alchemy.com/starknet/version/rpc/v0_8/YOUR_KEY"
    // )
    let sdk = StarkZap::new(StarkZapConfig::sepolia());

    // ── 2. Connect wallet ─────────────────────────────────────────────────────
    let signer = StarkSigner::new(
        &std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set"),
        &std::env::var("ACCOUNT_ADDRESS").expect("ACCOUNT_ADDRESS not set"),
    )?;

    let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    info!("Wallet: {}", wallet.address_hex());

    // ── 3. (Optional) Ensure account is deployed ──────────────────────────────
    //
    // For a fresh account, remove this or set DeployPolicy::IfNeeded.
    // Pre-deploying via ArgentX or starkli is recommended for v0.1.
    wallet.ensure_ready(DeployPolicy::Never).await?;

    // ── 4. Check balance ──────────────────────────────────────────────────────
    let strk = sepolia::strk();
    let balance = wallet.balance_of(&strk).await?;
    info!("STRK balance: {}", balance);

    // ── 5. Transfer ───────────────────────────────────────────────────────────
    let recipient_hex = std::env::var("RECIPIENT_ADDRESS")
        .expect("RECIPIENT_ADDRESS not set");
    let recipient = Felt::from_hex(&recipient_hex)
        .expect("Invalid RECIPIENT_ADDRESS");

    let amount = Amount::parse("0.01", &strk)?;
    info!("Transferring {} to {}", amount, recipient_hex);

    let tx = wallet
        .transfer(&strk, vec![Recipient::new(recipient, amount)])
        .await?;

    info!("Submitted: {}", tx);

    // ── 6. Wait for confirmation ──────────────────────────────────────────────
    let receipt = tx.wait().await?;
    info!("Confirmed in block {:?}", receipt.block_hash());

    Ok(())
}