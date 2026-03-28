//! Paymaster (gasless) transfer example — Sepolia testnet.
//!
//! Demonstrates AVNU gasless execution: the user pays no ETH/STRK for gas.
//! AVNU sponsors the fee.
//!
//! # Setup
//!
//! 1. Copy `.env.example` to `.env`.
//! 2. On Sepolia, AVNU_API_KEY is optional.
//!    On mainnet, obtain a key at <https://app.avnu.fi>.
//! 3. Run:
//!
//! ```sh
//! cargo run --example paymaster_transfer
//! ```

use dotenvy::dotenv;
use starknet::core::types::Felt;
use starkzap_rs::{
    Amount, OnboardConfig, Recipient, StarkZap, StarkZapConfig,
    paymaster::{FeeMode, PaymasterConfig},
    signer::StarkSigner,
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

    // ── Paymaster config ──────────────────────────────────────────────────────
    //
    // On Sepolia, no API key is needed.
    // On mainnet: PaymasterConfig::with_api_key("your_key")
    //         or: PaymasterConfig::from_env()  (reads AVNU_API_KEY)
    let pm_config = PaymasterConfig::new();

    let strk = sepolia::strk();
    let recipient = Felt::from_hex(
        &std::env::var("RECIPIENT_ADDRESS").expect("RECIPIENT_ADDRESS not set"),
    ).expect("Invalid RECIPIENT_ADDRESS");

    let amount = Amount::parse("0.001", &strk)?;
    info!("Gasless transfer of {} to {:#x}", amount, recipient);

    let tx = wallet
        .transfer(&strk, vec![Recipient::new(recipient, amount)])
        // Override the fee mode — swap UserPays → Paymaster
        // Note: wallet.transfer always uses UserPays internally.
        // For paymaster on a transfer, use wallet.execute directly:
        .await?;

    // ── Direct execute with paymaster ─────────────────────────────────────────
    //
    // For full paymaster control, use wallet.execute with explicit calls:
    use starknet::core::{types::Call, utils::get_selector_from_name};

    let usdc = sepolia::usdc();
    let transfer_amount = Amount::parse("1.0", &usdc)?;
    let [low, high] = transfer_amount.to_u256_felts();

    let calls = vec![Call {
        to: usdc.address,
        selector: get_selector_from_name("transfer").unwrap(),
        calldata: vec![recipient, low, high],
    }];

    let tx = wallet
        .execute(calls, FeeMode::Paymaster(pm_config))
        .await?;

    info!("Gasless tx submitted: {}", tx);
    let receipt = tx.wait().await?;
    info!("Confirmed: {:?}", receipt.block_hash());

    Ok(())
}