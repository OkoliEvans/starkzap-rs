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
    Amount, OnboardConfig, Recipient, StarkZap, StarkZapConfig, signer::StarkSigner,
    tokens::sepolia, wallet::DeployPolicy,
};
use tracing::info;

#[tokio::main]
async fn main() -> starkzap_rs::error::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("info,starkzap_rs=info")
        .init();

    // ── 1. Initialise SDK ─────────────────────────────────────────────────────
    //
    // RPC URL is read from the RPC_URL env var if set, otherwise falls back
    // to the SDK default endpoint. For starknet 0.17, use a JSON-RPC v0.9
    // compatible backend in your .env, e.g.:
    //   RPC_URL=https://starknet-sepolia.g.alchemy.com/v2/YOUR_API_KEY
    let sdk = StarkZap::new(StarkZapConfig::sepolia());

    // ── 2. Connect wallet ─────────────────────────────────────────────────────
    let signer = StarkSigner::new(
        &std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY not set"),
        &std::env::var("ACCOUNT_ADDRESS").expect("ACCOUNT_ADDRESS not set"),
    )?;

    // Preset is auto-detected from the deployed account class hash when possible.
    let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    info!("Wallet: {}", wallet.address_hex());

    // ── 3. (Optional) Ensure account is deployed ──────────────────────────────
    //
    // For a fresh funded account, auto-deploy the preset-derived account
    // before first use.
    wallet.ensure_ready(DeployPolicy::IfNeeded).await?;

    // ── 4. Check balance ──────────────────────────────────────────────────────
    let strk = sepolia::strk();
    let balance = wallet.balance_of(&strk).await?;
    info!("STRK balance: {}", balance);

    // ── 5. Transfer ───────────────────────────────────────────────────────────
    let recipient_hex = std::env::var("RECIPIENT_ADDRESS").expect("RECIPIENT_ADDRESS not set");
    let recipient = Felt::from_hex(&recipient_hex).expect("Invalid RECIPIENT_ADDRESS");

    let amount = Amount::parse("0.01", &strk)?;
    info!("Transferring {} to {}", amount, recipient_hex);

    let tx = wallet
        .transfer(&strk, vec![Recipient::new(recipient, amount)])
        .await?;

    info!("Submitted: {}", tx);

    // ── 6. Wait for confirmation ──────────────────────────────────────────────
    let receipt = tx.wait().await?;
    info!("Confirmed in block {}", receipt.block.block_number());

    Ok(())
}
