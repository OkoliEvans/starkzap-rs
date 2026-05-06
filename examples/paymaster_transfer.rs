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
    Amount, ExecuteOptions, OnboardConfig, Recipient, StarkZap, StarkZapConfig,
    paymaster::{FeeMode, PaymasterConfig},
    signer::StarkSigner,
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

    // ── Paymaster config ──────────────────────────────────────────────────────
    //
    // On Sepolia, no API key is needed.
    // On mainnet: PaymasterConfig::with_api_key("your_key")
    //         or: PaymasterConfig::from_env()  (reads AVNU_API_KEY)
    let pm_config = PaymasterConfig::from_env();

    let recipient =
        Felt::from_hex(&std::env::var("RECIPIENT_ADDRESS").expect("RECIPIENT_ADDRESS not set"))
            .expect("Invalid RECIPIENT_ADDRESS");

    // ── Execute with paymaster ────────────────────────────────────────────────
    //
    // `transfer_with_options` mirrors the StarkZap TS transfer API:
    // the SDK formats the ERC-20 u256 amount, builds the transfer call,
    // and routes execution according to the supplied fee mode.
    let strk = sepolia::strk();
    let amount = Amount::parse("0.001", &strk)?;

    info!("Gasless transfer of {} to {:#x}", amount, recipient);
    let tx = wallet
        .transfer_with_options(
            &strk,
            vec![Recipient::new(recipient, amount)],
            ExecuteOptions {
                fee_mode: Some(FeeMode::Paymaster(pm_config)),
            },
        )
        .await?;

    info!("Tx submitted: {}", tx);
    let receipt = tx.wait().await?;
    info!("Confirmed in block: {:?}", receipt.block.block_hash());

    Ok(())
}
