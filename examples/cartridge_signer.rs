//! Cartridge session-key signer example (requires `cartridge` feature).
//!
//! Demonstrates using a Cartridge-issued session key to sign transactions
//! from a Rust backend.
//!
//! # The flow
//!
//! ```text
//! Browser (JavaScript)                 Rust backend (starkzap-rs)
//! ─────────────────────────────────    ─────────────────────────────────────
//! 1. User connects Cartridge wallet
//! 2. Request a session key:
//!
//!    import { SessionSigner } from "@argent/x-sessions"
//!    const session = await controller.createSession({
//!      expiresAt: Date.now() + 86400000,  // 24h
//!      policies: [{ contractAddress: "0x...", selector: "transfer" }],
//!    })
//!    const sessionKey = session.sessionKeyPair.privateKey
//!    const accountAddress = controller.address
//!
//! 3. POST { sessionKey, accountAddress } to your Rust API ────────────────▶
//!                                          4. CartridgeSigner::new(session_key, address)
//!                                          5. wallet.transfer(...) ← signed with session key
//! ```
//!
//! # Setup
//!
//! ```sh
//! CARTRIDGE_SESSION_KEY=0x... \
//! CARTRIDGE_ACCOUNT_ADDRESS=0x... \
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
        OnboardConfig, StarkZap, StarkZapConfig,
        signer::CartridgeSigner,
        tokens::sepolia,
    };
    use tracing::info;

    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter("starkzap_rs=debug,info")
        .init();

    // ── Load session key from env (in production: from your API request body) ─
    let session_key = std::env::var("CARTRIDGE_SESSION_KEY")
        .expect("CARTRIDGE_SESSION_KEY not set");
    let account_address = std::env::var("CARTRIDGE_ACCOUNT_ADDRESS")
        .expect("CARTRIDGE_ACCOUNT_ADDRESS not set");

    // ── Build signer ──────────────────────────────────────────────────────────
    let signer = CartridgeSigner::new(&session_key, &account_address)?;
    info!("Cartridge account: {:#x}", signer.address());

    // ── Connect wallet ────────────────────────────────────────────────────────
    let sdk = StarkZap::new(StarkZapConfig::sepolia());
    let wallet = sdk.onboard(OnboardConfig::Cartridge(signer)).await?;

    // ── Query balance ─────────────────────────────────────────────────────────
    let strk = sepolia::strk();
    let balance = wallet.balance_of(&strk).await?;
    info!("STRK balance: {}", balance);

    // ── Execute within session policy ─────────────────────────────────────────
    //
    // The session key is only valid for the policies approved by the user
    // in the browser. Attempting calls outside those policies will be rejected
    // by the Cartridge account on-chain.
    info!("Session key signer is ready. Attach to your API endpoint.");

    Ok(())
}