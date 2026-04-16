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
//!    # Optional: reuse an existing wallet instead of creating one each run
//!    PRIVY_WALLET_ID=clwlt_...
//!    PRIVY_WALLET_ADDRESS=0x...
//!    PRIVY_PUBLIC_KEY=0x...
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
use starkzap_rs::{OnboardConfig, StarkZap, StarkZapConfig, tokens::sepolia};

#[cfg(not(feature = "privy"))]
fn main() {
    eprintln!("This example requires the `privy` feature:");
    eprintln!("  cargo run --example privy_signer --features privy");
}

#[cfg(feature = "privy")]
#[tokio::main]
async fn main() -> starkzap_rs::error::Result<()> {
    use dotenvy::dotenv;
    use starknet::core::types::Felt;
    use tracing::info;

    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("info,starkzap_rs=info")
        .init();

    // ── 1. Load existing wallet metadata or create a new one ──────────────────
    //
    // This mirrors the official StarkZap TS server example more closely:
    // persist walletId + address + publicKey, then reuse them on later runs.
    let privy = if let (Ok(wallet_id), Ok(address_hex), Ok(public_key_hex)) = (
        std::env::var("PRIVY_WALLET_ID"),
        std::env::var("PRIVY_WALLET_ADDRESS"),
        std::env::var("PRIVY_PUBLIC_KEY"),
    ) {
        let address = Felt::from_hex(&address_hex).expect("Invalid PRIVY_WALLET_ADDRESS");
        let public_key = Felt::from_hex(&public_key_hex).expect("Invalid PRIVY_PUBLIC_KEY");
        info!("Using existing Privy wallet: {}", wallet_id);
        PrivySigner::from_env()?.with_wallet_and_public_key(wallet_id, address, public_key)
    } else {
        let mut privy = PrivySigner::from_env()?;
        let user_id = std::env::var("PRIVY_USER_ID").unwrap_or_else(|_| "demo-user-001".into());
        let wallet = privy.create_wallet_info(&user_id).await?;

        info!("Created Privy signer wallet: {:#x}", wallet.address);
        info!("Persist these values for reuse:");
        info!("PRIVY_WALLET_ID={}", wallet.wallet_id);
        info!("PRIVY_WALLET_ADDRESS={:#x}", wallet.address);

        let public_key = wallet.public_key.ok_or_else(|| {
            starkzap_rs::StarkzapError::Other(
                "Privy created a wallet but did not return a Stark public key. Reuse the wallet only after fetching its public key from your Privy backend.".into(),
            )
        })?;
        info!("PRIVY_PUBLIC_KEY={:#x}", public_key);

        privy.with_wallet_and_public_key(wallet.wallet_id, wallet.address, public_key)
    };

    // ── 2. Wire into SDK ──────────────────────────────────────────────────────
    //
    // Privy uses ArgentX v0.5.0 in the official StarkZap TS SDK, so we onboard
    // it directly with the Privy signer metadata.

    let sdk = StarkZap::new(StarkZapConfig::sepolia());
    let wallet = sdk.onboard(OnboardConfig::Privy(privy.clone())).await?;

    let strk = sepolia::strk();
    let balance = wallet.balance_of(&strk).await?;

    let signer_address = privy.address().expect("Privy signer missing address");
    info!("Privy signer wallet address: {:#x}", signer_address);
    info!("Derived Starknet account address: {}", wallet.address_hex());
    info!("Privy account STRK balance: {}", balance);

    Ok(())
}
