//! Cartridge session signer example (requires `cartridge` feature).
//!
//! Demonstrates using a Cartridge-issued session bundle to execute
//! policy-approved transactions from a Rust backend.
//!
//! # The flow
//!
//! ```text
//! Browser (JavaScript / Controller)    Rust backend (starkzap-rs)
//! ─────────────────────────────────    ─────────────────────────────────────
//! 1. User connects Cartridge wallet
//! 2. Define policies in Controller and approve them
//! 3. Create a session:
//!
//!    import Controller from "@cartridge/controller";
//!    const controller = new Controller({
//!      policies: [{ target: STRK_TOKEN, method: "transfer" }],
//!    });
//!    const account = await controller.connect();
//!    // Session material is managed by Controller / SessionProvider
//!    // and forwarded to your backend for server-side signing.
//!
//! 4. Export the full session bundle from the helper page ────────────────▶
//!                                          5. CartridgeSigner::from_env()
//!                                          6. wallet.transfer(...) within policy
//! ```
//!
//! # Setup
//!
//! ```sh
//! CARTRIDGE_SESSION_BUNDLE_B64=... \
//! RECIPIENT_ADDRESS=0x... \
//! CARTRIDGE_RUN_TRANSFER=1 \
//! cargo run --example cartridge_signer --features cartridge
//! ```

#[cfg(not(feature = "cartridge"))]
fn main() {
    eprintln!("This example requires the `cartridge` feature:");
    eprintln!("  cargo run --example cartridge_signer --features cartridge");
}

#[cfg(feature = "cartridge")]
#[tokio::main]
async fn main() -> starkzap_rs::error::Result<()> {
    use dotenvy::dotenv;
    use starkzap_rs::{
        Amount, OnboardConfig, Recipient, StarkZap, StarkZapConfig,
        signer::CartridgeSigner,
        tokens::sepolia,
    };
    use starknet::core::types::Felt;
    use tracing::info;

    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("info,starkzap_rs=info")
        .init();

    // ── Load full session bundle from env ────────────────────────────────────
    let signer = CartridgeSigner::from_env()?;
    info!("Cartridge account: {:#x}", signer.address());

    // ── Connect wallet ────────────────────────────────────────────────────────
    let sdk = StarkZap::new(StarkZapConfig::sepolia());
    let wallet = sdk.onboard(OnboardConfig::Cartridge(signer)).await?;

    // ── Query balance ─────────────────────────────────────────────────────────
    let strk = sepolia::strk();
    let balance = wallet.balance_of(&strk).await?;
    info!("STRK balance: {}", balance);

    // ── Execute within session policy (optional) ──────────────────────────────
    //
    // The session is only valid for policies approved in the browser-side
    // Cartridge flow. To test an actual signed tx, set:
    //   RECIPIENT_ADDRESS=0x...
    //   CARTRIDGE_RUN_TRANSFER=1
    // and make sure the approved session policy includes STRK `transfer`.
    let should_transfer = std::env::var("CARTRIDGE_RUN_TRANSFER")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if should_transfer {
        let recipient_hex = std::env::var("RECIPIENT_ADDRESS")
            .expect("RECIPIENT_ADDRESS not set");
        let recipient = Felt::from_hex(&recipient_hex).expect("Invalid RECIPIENT_ADDRESS");
        let amount = Amount::parse(
            &std::env::var("CARTRIDGE_TRANSFER_AMOUNT").unwrap_or_else(|_| "0.001".into()),
            &strk,
        )?;

        info!("Attempting policy-approved transfer of {} to {}", amount, recipient_hex);
        let tx = wallet
            .transfer(&strk, vec![Recipient::new(recipient, amount)])
            .await?;
        info!("Submitted: {}", tx);

        let receipt = tx.wait().await?;
        info!("Confirmed in block {}", receipt.block.block_number());
    } else {
        info!("Session key signer is ready. Set CARTRIDGE_RUN_TRANSFER=1 to test a real policy-approved transfer.");
    }

    Ok(())
}
