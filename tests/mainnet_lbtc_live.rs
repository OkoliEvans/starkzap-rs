//! Live mainnet LBTC integration tests.
//!
//! These tests are intentionally `#[ignore]` because they can touch real funds.
//! Run them explicitly with:
//!
//! ```sh
//! cargo test --test mainnet_lbtc_live -- --ignored --nocapture
//! ```
//!
//! Required env vars:
//! - `PRIVATE_KEY`
//! - `ACCOUNT_ADDRESS`
//! - `RECIPIENT_ADDRESS`
//!
//! Optional env vars:
//! - `RPC_URL` or `MAINNET_RPC_URL`
//! - `AVNU_API_KEY`
//! - `MAINNET_LBTC_TRANSFER_AMOUNT` (default `0.00001`)

use std::sync::OnceLock;

use starknet::core::{
    types::{Call, Felt},
    utils::get_selector_from_name,
};
use starkzap_rs::{
    Amount, OnboardConfig, StarkZap, StarkZapConfig,
    paymaster::{FeeMode, PaymasterConfig},
    signer::StarkSigner,
    tokens::mainnet,
    wallet::{DeployPolicy, Recipient, StarknetProvider, Wallet},
};
use tokio::sync::Mutex;

fn write_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn env_var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} env var not set"))
}

fn mainnet_config() -> StarkZapConfig {
    let rpc_url = std::env::var("MAINNET_RPC_URL")
        .ok()
        .or_else(|| std::env::var("RPC_URL").ok());

    StarkZapConfig {
        network: starkzap_rs::Network::Mainnet,
        rpc_url,
    }
}

async fn mainnet_wallet() -> starkzap_rs::Result<Wallet<StarknetProvider>> {
    dotenvy::dotenv().ok();

    let sdk = StarkZap::new(mainnet_config());
    let signer = StarkSigner::new(&env_var("PRIVATE_KEY"), &env_var("ACCOUNT_ADDRESS"))?;
    let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
    wallet.ensure_ready(DeployPolicy::IfNeeded).await?;
    Ok(wallet)
}

fn recipient_felt() -> Felt {
    Felt::from_hex(&env_var("RECIPIENT_ADDRESS")).expect("RECIPIENT_ADDRESS must be valid felt")
}

fn lbtc_amount() -> Amount {
    let lbtc = mainnet::lbtc();
    let amount_raw =
        std::env::var("MAINNET_LBTC_TRANSFER_AMOUNT").unwrap_or_else(|_| "0.00001".into());
    Amount::parse(&amount_raw, &lbtc).expect("invalid MAINNET_LBTC_TRANSFER_AMOUNT")
}

fn balance_covers_two_transfers(balance: &Amount, amount: &Amount) -> bool {
    amount
        .checked_add(amount)
        .is_some_and(|required| balance.raw() >= required.raw())
}

#[tokio::test]
#[ignore = "requires live mainnet RPC and funded LBTC balance"]
async fn mainnet_lbtc_balance_and_deployment() {
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let lbtc = mainnet::lbtc();

    let deployed = wallet.is_deployed().await.expect("is_deployed failed");
    let balance = wallet.balance_of(&lbtc).await.expect("balance_of failed");

    println!("wallet: {}", wallet.address_hex());
    println!("deployed: {deployed}");
    println!("lbtc balance: {balance}");

    assert!(deployed);
    assert!(balance.raw() < u128::MAX);
}

#[tokio::test]
#[ignore = "requires live mainnet RPC and real LBTC transfer"]
async fn mainnet_transfer_lbtc() {
    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let lbtc = mainnet::lbtc();
    let amount = lbtc_amount();

    let tx = wallet
        .transfer(&lbtc, vec![Recipient::new(recipient_felt(), amount)])
        .await
        .expect("transfer failed");

    println!("lbtc transfer tx: {}", tx);
    let receipt = tx.wait().await.expect("wait failed");
    println!("confirmed in block {}", receipt.block.block_number());
}

#[tokio::test]
#[ignore = "requires live mainnet RPC and raw execute LBTC transfer"]
async fn mainnet_execute_transfer_lbtc_user_pays() {
    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let lbtc = mainnet::lbtc();
    let amount = lbtc_amount();
    let [low, high] = amount.to_u256_felts();

    let tx = wallet
        .execute(
            vec![Call {
                to: lbtc.address,
                selector: get_selector_from_name("transfer").expect("selector"),
                calldata: vec![recipient_felt(), low, high],
            }],
            FeeMode::UserPays,
        )
        .await
        .expect("execute failed");

    println!("lbtc execute tx: {}", tx);
    let receipt = tx.wait().await.expect("wait failed");
    println!("confirmed in block {}", receipt.block.block_number());
}

#[tokio::test]
#[ignore = "requires AVNU_API_KEY and live mainnet RPC"]
async fn mainnet_paymaster_transfer_lbtc() {
    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let lbtc = mainnet::lbtc();

    let Some(api_key) = std::env::var("AVNU_API_KEY").ok().filter(|v| !v.is_empty()) else {
        println!("skipping: AVNU_API_KEY not set");
        return;
    };

    let amount = lbtc_amount();
    let [low, high] = amount.to_u256_felts();

    let tx = wallet
        .execute(
            vec![Call {
                to: lbtc.address,
                selector: get_selector_from_name("transfer").expect("selector"),
                calldata: vec![recipient_felt(), low, high],
            }],
            FeeMode::Paymaster(PaymasterConfig::with_api_key(api_key)),
        )
        .await
        .expect("paymaster execution failed");

    println!("lbtc paymaster tx: {}", tx);
    let receipt = tx.wait().await.expect("wait failed");
    println!("confirmed in block {}", receipt.block.block_number());
}

#[tokio::test]
#[ignore = "smoke test that runs the main mainnet LBTC flows in sequence"]
async fn mainnet_smoke_lbtc_systems() {
    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let lbtc = mainnet::lbtc();
    let balance = wallet.balance_of(&lbtc).await.expect("balance_of failed");
    println!("wallet: {}", wallet.address_hex());
    println!("lbtc balance: {balance}");

    let amount = lbtc_amount();
    let transfer_tx = wallet
        .transfer(&lbtc, vec![Recipient::new(recipient_felt(), amount.clone())])
        .await
        .expect("transfer failed");
    println!("lbtc transfer tx: {}", transfer_tx);
    transfer_tx.wait().await.expect("transfer wait failed");

    if let Some(api_key) = std::env::var("AVNU_API_KEY").ok().filter(|v| !v.is_empty()) {
        if !balance_covers_two_transfers(&balance, &amount) {
            println!(
                "skipping paymaster leg: smoke test balance {} is too low for two {} transfers",
                balance, amount
            );
            return;
        }

        let [low, high] = amount.to_u256_felts();
        let paymaster_tx = wallet
            .execute(
                vec![Call {
                    to: lbtc.address,
                    selector: get_selector_from_name("transfer").expect("selector"),
                    calldata: vec![recipient_felt(), low, high],
                }],
                FeeMode::Paymaster(PaymasterConfig::with_api_key(api_key)),
            )
            .await
            .expect("paymaster execute failed");
        println!("lbtc paymaster tx: {}", paymaster_tx);
        paymaster_tx.wait().await.expect("paymaster wait failed");
    } else {
        println!("AVNU_API_KEY not set; skipping paymaster leg");
    }
}
