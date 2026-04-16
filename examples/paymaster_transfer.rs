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
use starknet::core::{types::{Call, Felt}, utils::get_selector_from_name};
use starkzap_rs::{
    Amount, OnboardConfig, StarkZap, StarkZapConfig,
    paymaster::{FeeMode, PaymasterConfig, PaymasterDetails},
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

    // ── Build the call ────────────────────────────────────────────────────────
    //
    // Transfer 0.001 STRK via the ERC-20 `transfer` entrypoint.
    // wallet.execute accepts any Vec<Call>, so you can batch multiple
    // token transfers or arbitrary contract calls in a single gasless tx.
    let strk = sepolia::strk();
    let amount = Amount::parse("0.001", &strk)?;
    let [low, high] = amount.to_u256_felts();

    let calls = vec![Call {
        to: strk.address,
        selector: get_selector_from_name("transfer").unwrap(),
        calldata: vec![recipient, low, high],
    }];

    info!("Gasless transfer of {} to {:#x}", amount, recipient);

    // ── Execute with paymaster ────────────────────────────────────────────────
    let tx = wallet.execute(calls, FeeMode::Paymaster(pm_config)).await?;

    info!("Tx submitted: {}", tx);
    let receipt = tx.wait().await?;
    info!("Confirmed in block: {:?}", receipt.block.block_hash());

    // TS-style explicit API is also available. Opt in with:
    //   PAYMASTER_BUILD_ONLY=1 cargo run --example paymaster_transfer
    //
    // Some account classes can execute via the fallback path above but still
    // reject explicit sponsored build requests, so we keep this as an optional
    // demonstration instead of running it unconditionally.
    let build_only = std::env::var("PAYMASTER_BUILD_ONLY")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if build_only {
        let _ = wallet
            .build_paymaster_transaction(
                vec![Call {
                    to: strk.address,
                    selector: get_selector_from_name("transfer").unwrap(),
                    calldata: vec![recipient, low, high],
                }],
                PaymasterDetails::sponsored(),
                std::env::var("AVNU_API_KEY").ok(),
            )
            .await?;
    }

    Ok(())
}
