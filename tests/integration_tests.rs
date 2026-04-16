//! Integration tests — require a live RPC or local devnet.
//!
//! # Running against devnet (recommended for CI)
//!
//! ```sh
//! # Install starknet-devnet
//! pip install starknet-devnet
//! # or: cargo install starknet-devnet
//!
//! # Start devnet (seeds preloaded accounts)
//! starknet-devnet --seed 0 &
//!
//! # Run tests
//! cargo test --test integration_tests
//! ```
//!
//! # Running against Sepolia
//!
//! Set environment variables (see .env.example) and run:
//! ```sh
//! TEST_NETWORK=sepolia cargo test --test integration_tests
//! ```
//!
//! # Skipping
//!
//! Tests that require a live RPC are gated behind `#[ignore]` by default.
//! Run them with `cargo test -- --ignored`.

use starknet::core::types::Felt;
use starkzap_rs::{
    Amount, Network, OnboardConfig, StarkZap, StarkZapConfig,
    error::Result,
    signer::StarkSigner,
    tokens::{mainnet, sepolia},
    wallet::{StarknetProvider, Wallet},
};

// ── Amount unit tests (no network required) ───────────────────────────────────

#[test]
fn amount_parse_whole() {
    let tok = sepolia::usdc();
    let a = Amount::parse("10", &tok).unwrap();
    assert_eq!(a.raw(), 10_000_000);
}

#[test]
fn amount_parse_decimal() {
    let tok = sepolia::usdc();
    let a = Amount::parse("1.5", &tok).unwrap();
    assert_eq!(a.raw(), 1_500_000);
}

#[test]
fn amount_format_roundtrip() {
    let tok = sepolia::strk();
    let a = Amount::parse("100.123456789", &tok).unwrap();
    let s = a.to_decimal_string();
    let b = Amount::parse(&s, &tok).unwrap();
    assert_eq!(a.raw(), b.raw());
}

#[test]
fn amount_to_u256_felts_low_high() {
    let tok = sepolia::usdc();
    let a = Amount::parse("1", &tok).unwrap();
    let [low, high] = a.to_u256_felts();
    assert_eq!(low, Felt::from(1_000_000u128));
    assert_eq!(high, Felt::ZERO);
}

#[test]
fn amount_zero() {
    let tok = sepolia::strk();
    let a = Amount::parse("0", &tok).unwrap();
    assert!(a.is_zero());
}

#[test]
fn amount_invalid_input() {
    let tok = sepolia::usdc();
    assert!(Amount::parse("abc", &tok).is_err());
    assert!(Amount::parse("", &tok).is_err());
    assert!(Amount::parse("1.2.3", &tok).is_err());
}

// ── Token preset tests (no network required) ──────────────────────────────────

#[test]
fn token_presets_have_correct_decimals() {
    assert_eq!(mainnet::usdc().decimals, 6);
    assert_eq!(mainnet::strk().decimals, 18);
    assert_eq!(mainnet::eth().decimals, 18);
    assert_eq!(mainnet::wbtc().decimals, 8);
    assert_eq!(mainnet::tbtc().decimals, 18);
    assert_eq!(mainnet::lbtc().decimals, 8);
    assert_eq!(mainnet::xwbtc().decimals, 8);
    assert_eq!(mainnet::solvbtc().decimals, 18);
}

#[test]
fn token_by_symbol_case_insensitive() {
    assert!(mainnet::by_symbol("usdc").is_some());
    assert!(mainnet::by_symbol("USDC").is_some());
    assert!(mainnet::by_symbol("wbtc").is_some());
    assert!(mainnet::by_symbol("tbtc").is_some());
    assert!(mainnet::by_symbol("lbtc").is_some());
    assert!(mainnet::by_symbol("doesnotexist").is_none());
}

// ── Network tests (no network required) ───────────────────────────────────────

#[test]
fn network_rpc_urls_are_valid() {
    use starknet::providers::Url;
    for net in [Network::Mainnet, Network::Sepolia, Network::Devnet] {
        Url::parse(net.default_rpc_url()).unwrap_or_else(|_| panic!("Invalid URL for {:?}", net));
    }
}

#[test]
fn network_display() {
    assert_eq!(Network::Mainnet.to_string(), "mainnet");
    assert_eq!(Network::Sepolia.to_string(), "sepolia");
    assert_eq!(Network::Devnet.to_string(), "devnet");
}

// ── Signer construction (no network required) ─────────────────────────────────

#[test]
fn stark_signer_invalid_key_errors() {
    let err = StarkSigner::new("not_a_hex_key", "0x1234");
    assert!(err.is_err());
}

#[test]
fn stark_signer_invalid_address_errors() {
    let err = StarkSigner::new("0x1", "not_an_address");
    assert!(err.is_err());
}

#[test]
fn stark_signer_valid() {
    // A valid (dummy) private key and address.
    let signer = StarkSigner::new(
        "0x02bbf4f9fd0bbb2e60b0316c1fe0b76cf7a4d0198bd493ced9b8df2a3a24d68b",
        "0x064b48806902a367c8598f4f95c305e8c1a1acba5f082d294a43793113115691",
    );
    assert!(signer.is_ok());
}

// ── Live network tests (require RPC — gated by #[ignore]) ────────────────────

/// Build a test wallet pointing at Sepolia devnet or the public RPC.
///
/// Returns the concrete `Wallet<StarknetProvider>` — no generics needed at
/// call sites because `StarknetProvider` is a type alias for
/// `JsonRpcClient<HttpTransport>`.
async fn test_wallet() -> Result<Wallet<StarknetProvider>> {
    dotenvy::dotenv().ok();
    let rpc = std::env::var("TEST_RPC_URL")
        .unwrap_or_else(|_| Network::Devnet.default_rpc_url().to_string());

    let config = StarkZapConfig {
        network: Network::Sepolia,
        rpc_url: Some(rpc),
    };

    let pk = std::env::var("TEST_PRIVATE_KEY").expect("TEST_PRIVATE_KEY required for live tests");
    let addr = std::env::var("TEST_ACCOUNT_ADDRESS")
        .expect("TEST_ACCOUNT_ADDRESS required for live tests");

    let signer = StarkSigner::new(&pk, &addr)?;
    let sdk = StarkZap::new(config);
    sdk.onboard(OnboardConfig::Signer(signer)).await
}

#[tokio::test]
#[ignore = "requires live RPC — run with: cargo test -- --ignored"]
async fn live_balance_query() {
    let wallet = test_wallet().await.expect("wallet setup failed");
    let strk = sepolia::strk();
    let balance = wallet
        .balance_of(&strk)
        .await
        .expect("balance query failed");
    println!("Live STRK balance: {}", balance);
    assert!(balance.raw() < u128::MAX);
}

#[tokio::test]
#[ignore = "requires live RPC and funded account"]
async fn live_is_deployed() {
    let wallet = test_wallet().await.expect("wallet setup failed");
    let deployed = wallet.is_deployed().await.expect("is_deployed failed");
    println!("Account deployed: {}", deployed);
}

#[tokio::test]
#[ignore = "requires live RPC, funded account, and RECIPIENT_ADDRESS env var"]
async fn live_transfer_strk() {
    let wallet = test_wallet().await.expect("wallet setup failed");
    let strk = sepolia::strk();

    let recipient =
        std::env::var("TEST_RECIPIENT_ADDRESS").expect("TEST_RECIPIENT_ADDRESS required");
    let recipient_felt = Felt::from_hex(&recipient).expect("Invalid address");

    let amount = Amount::parse("0.001", &strk).unwrap();
    let tx = wallet
        .transfer(
            &strk,
            vec![starkzap_rs::wallet::Recipient::new(recipient_felt, amount)],
        )
        .await
        .expect("transfer failed");

    println!("Tx: {}", tx);
    tx.wait().await.expect("wait failed");
    println!("Confirmed ✓");
}
