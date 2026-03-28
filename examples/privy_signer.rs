//! Privy server-side signer example (requires `privy` feature).
//!
//! Demonstrates creating a Privy embedded wallet and using it as a signer.
//!
//! # Setup
//!
//! 1. Create a Privy app at <https://privy.io>
//! 2. In Dashboard → Settings → API Keys → copy App ID and App Secret
//! 3. Add to `.env`:
//!    ```
//!    PRIVY_APP_ID=clxxxxxxxxxxxxxxxx
//!    PRIVY_APP_SECRET=privy_secret_...
//!    ```
//! 4. Run with the `privy` feature:
//!    ```sh
//!    cargo run --example privy_signer --features privy
//!    ```
//!
//! # How it works
//!
//! Privy's server API creates and manages wallets. Your backend never holds
//! the private key — signing happens inside Privy's infrastructure. This
//! is ideal for onboarding users via social login (email, Google, Apple).

#[cfg(feature = "privy")]
use starkzap_rs::signer::PrivySigner;
use starkzap_rs::{StarkZap, StarkZapConfig};

#[cfg(not(feature = "privy"))]
fn main() {
    eprintln!("This example requires the `privy` feature:");
    eprintln!("  cargo run --example privy_signer --features privy");
}

#[cfg(feature = "privy")]
#[tokio::main]
async fn main() -> starkzap_rs::error::Result<()> {
    use dotenvy::dotenv;
    use tracing::info;

    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("starkzap_rs=debug,info")
        .init();

    // ── 1. Initialise Privy signer from env vars ──────────────────────────────
    let mut privy = PrivySigner::from_env()?;

    // ── 2. Create a new embedded Starknet wallet for a user ───────────────────
    //
    // In production, `user_id` is your app's user identifier (e.g. database ID,
    // email hash). Privy associates the wallet with this user.
    let user_id = "demo-user-001";
    let address = privy.create_wallet(user_id).await?;
    info!("Created Privy wallet: {:#x}", address);

    // ── 3. For an existing wallet, load it instead ────────────────────────────
    //
    // If the user already has a wallet from a previous session:
    //   let privy = PrivySigner::from_env()?
    //       .with_wallet("wallet_id_from_db", address_felt);

    // ── 4. Wire into SDK ──────────────────────────────────────────────────────
    //
    // NOTE: Full Privy → starknet-rs signing delegation is tracked in issue #1.
    // For now, this example shows wallet creation. The balance query below uses
    // the provider directly (no signing required).

    let sdk = StarkZap::new(StarkZapConfig::sepolia());
    let provider = sdk.provider();

    use starknet::core::types::{BlockId, BlockTag};
    use starknet::providers::Provider;

    let block = provider.block_number().await.unwrap_or(0);
    info!("Connected to Sepolia, latest block: {}", block);

    info!("Privy wallet address {:#x} is ready for use.", address);
    info!("See issue #1 for full signing delegation support.");

    Ok(())
}