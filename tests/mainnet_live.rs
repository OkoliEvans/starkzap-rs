//! Live mainnet integration tests.
//!
//! These tests are intentionally `#[ignore]` because they can touch real funds.
//! Run them explicitly with:
//!
//! ```sh
//! cargo test --test mainnet_live -- --ignored --nocapture
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
//! - `MAINNET_TRANSFER_AMOUNT` (default `0.001`)
//! - `RUN_MAINNET_STAKING_WRITES=1`
//! - `MAINNET_STAKING_AMOUNT` (default `0.01`)

use starknet::core::{types::{Call, Felt}, utils::get_selector_from_name};
use starkzap_rs::{
    Amount, OnboardConfig, StarkZap, StarkZapConfig, StarkzapError,
    paymaster::{FeeMode, PaymasterConfig},
    signer::StarkSigner,
    staking::presets::mainnet_validators,
    tokens::mainnet,
    wallet::{DeployPolicy, Recipient, StarknetProvider, Wallet},
};
use std::sync::OnceLock;
use tokio::sync::Mutex;

fn write_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn env_var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} env var not set"))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
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

#[tokio::test]
#[ignore = "requires live mainnet RPC and a funded account"]
async fn mainnet_strk_balance_and_deployment() {
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let strk = mainnet::strk();

    let deployed = wallet.is_deployed().await.expect("is_deployed failed");
    let balance = wallet.balance_of(&strk).await.expect("balance_of failed");

    println!("wallet: {}", wallet.address_hex());
    println!("deployed: {deployed}");
    println!("balance: {balance}");

    assert!(deployed);
    assert!(balance.raw() < u128::MAX);
}

#[tokio::test]
#[ignore = "requires live mainnet RPC and real STRK transfer"]
async fn mainnet_transfer_strk() {
    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let strk = mainnet::strk();
    let amount_raw = std::env::var("MAINNET_TRANSFER_AMOUNT").unwrap_or_else(|_| "0.001".into());
    let amount = Amount::parse(&amount_raw, &strk).expect("invalid MAINNET_TRANSFER_AMOUNT");

    let tx = wallet
        .transfer(&strk, vec![Recipient::new(recipient_felt(), amount)])
        .await
        .expect("transfer failed");

    println!("transfer tx: {}", tx);
    let receipt = tx.wait().await.expect("wait failed");
    println!("confirmed in block {}", receipt.block.block_number());
}

#[tokio::test]
#[ignore = "requires live mainnet RPC and raw execute transfer"]
async fn mainnet_execute_transfer_strk_user_pays() {
    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let strk = mainnet::strk();
    let amount_raw = std::env::var("MAINNET_TRANSFER_AMOUNT").unwrap_or_else(|_| "0.001".into());
    let amount = Amount::parse(&amount_raw, &strk).expect("invalid MAINNET_TRANSFER_AMOUNT");
    let [low, high] = amount.to_u256_felts();

    let tx = wallet
        .execute(
            vec![Call {
                to: strk.address,
                selector: get_selector_from_name("transfer").expect("selector"),
                calldata: vec![recipient_felt(), low, high],
            }],
            FeeMode::UserPays,
        )
        .await
        .expect("execute failed");

    println!("execute tx: {}", tx);
    let receipt = tx.wait().await.expect("wait failed");
    println!("confirmed in block {}", receipt.block.block_number());
}

#[tokio::test]
#[ignore = "requires AVNU_API_KEY and live mainnet RPC"]
async fn mainnet_paymaster_transfer_strk() {
    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let strk = mainnet::strk();

    let Some(api_key) = std::env::var("AVNU_API_KEY").ok().filter(|v| !v.is_empty()) else {
        println!("skipping: AVNU_API_KEY not set");
        return;
    };

    let amount_raw = std::env::var("MAINNET_TRANSFER_AMOUNT").unwrap_or_else(|_| "0.001".into());
    let amount = Amount::parse(&amount_raw, &strk).expect("invalid MAINNET_TRANSFER_AMOUNT");
    let [low, high] = amount.to_u256_felts();

    let tx = wallet
        .execute(
            vec![Call {
                to: strk.address,
                selector: get_selector_from_name("transfer").expect("selector"),
                calldata: vec![recipient_felt(), low, high],
            }],
            FeeMode::Paymaster(PaymasterConfig::with_api_key(api_key)),
        )
        .await
        .expect("paymaster execution failed");

    println!("paymaster tx: {}", tx);
    let receipt = tx.wait().await.expect("wait failed");
    println!("confirmed in block {}", receipt.block.block_number());
}

#[tokio::test]
#[ignore = "requires live mainnet RPC and performs read-only staking checks"]
async fn mainnet_staking_readonly() {
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let strk = mainnet::strk();
    let validator = mainnet_validators()
        .into_iter()
        .next()
        .expect("mainnet validators should not be empty");

    let pools = wallet
        .get_staker_pools(validator.staker_address)
        .await
        .expect("get_staker_pools failed");
    let pool = pools.first().expect("validator should have at least one pool");
    let position = wallet
        .get_pool_position(pool.address, &strk)
        .await
        .expect("get_pool_position failed");

    println!("validator: {}", validator.name);
    println!("pool: {:#x}", pool.address);
    println!("staked: {}", position.staked);
    println!("rewards: {}", position.rewards);
}

#[tokio::test]
#[ignore = "requires RUN_MAINNET_STAKING_WRITES=1 and performs real staking writes"]
async fn mainnet_staking_write_flow() {
    if !env_flag("RUN_MAINNET_STAKING_WRITES") {
        println!("skipping: set RUN_MAINNET_STAKING_WRITES=1 to enable");
        return;
    }

    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let strk = mainnet::strk();
    let validator = mainnet_validators()
        .into_iter()
        .next()
        .expect("mainnet validators should not be empty");
    let pools = wallet
        .get_staker_pools(validator.staker_address)
        .await
        .expect("get_staker_pools failed");
    let pool = pools.first().expect("validator should have at least one pool");

    let amount_raw = std::env::var("MAINNET_STAKING_AMOUNT").unwrap_or_else(|_| "0.01".into());
    let amount = Amount::parse(&amount_raw, &strk).expect("invalid MAINNET_STAKING_AMOUNT");

    let stake_tx = wallet
        .enter_pool(&strk, pool.address, amount, wallet.address(), FeeMode::UserPays)
        .await
        .expect("enter_pool failed");
    println!("stake tx: {}", stake_tx);
    stake_tx.wait().await.expect("stake wait failed");

    let position = wallet
        .get_pool_position(pool.address, &strk)
        .await
        .expect("get_pool_position failed");
    println!("updated staked: {}", position.staked);
    println!("updated rewards: {}", position.rewards);

    if !position.rewards.is_zero() {
        let claim_tx = wallet
            .claim_rewards(pool.address, FeeMode::UserPays)
            .await
            .expect("claim_rewards failed");
        println!("claim tx: {}", claim_tx);
        claim_tx.wait().await.expect("claim wait failed");
    }

    if !position.staked.is_zero() {
        let exit_tx = wallet
            .exit_pool_intent(pool.address, position.staked, FeeMode::UserPays)
            .await
            .expect("exit_pool_intent failed");
        println!("exit intent tx: {}", exit_tx);
        exit_tx.wait().await.expect("exit intent wait failed");
    }
}

#[tokio::test]
#[ignore = "smoke test that runs the main mainnet STRK flows in sequence"]
async fn mainnet_smoke_strk_systems() {
    let _guard = write_test_lock().lock().await;
    let wallet = mainnet_wallet().await.expect("wallet setup failed");
    let strk = mainnet::strk();
    let balance = wallet.balance_of(&strk).await.expect("balance_of failed");
    println!("wallet: {}", wallet.address_hex());
    println!("balance: {balance}");

    let validator = mainnet_validators()
        .into_iter()
        .next()
        .expect("mainnet validators should not be empty");
    let pools = wallet
        .get_staker_pools(validator.staker_address)
        .await
        .expect("get_staker_pools failed");
    let pool = pools.first().expect("validator should have a pool");
    let _ = wallet
        .get_pool_position(pool.address, &strk)
        .await
        .expect("get_pool_position failed");

    let amount_raw = std::env::var("MAINNET_TRANSFER_AMOUNT").unwrap_or_else(|_| "0.001".into());
    let amount = Amount::parse(&amount_raw, &strk).expect("invalid MAINNET_TRANSFER_AMOUNT");
    let transfer_tx = wallet
        .transfer(&strk, vec![Recipient::new(recipient_felt(), amount.clone())])
        .await
        .expect("transfer failed");
    println!("transfer tx: {}", transfer_tx);
    transfer_tx.wait().await.expect("transfer wait failed");

    if let Some(api_key) = std::env::var("AVNU_API_KEY").ok().filter(|v| !v.is_empty()) {
        let [low, high] = amount.to_u256_felts();
        let paymaster_tx = wallet
            .execute(
                vec![Call {
                    to: strk.address,
                    selector: get_selector_from_name("transfer").expect("selector"),
                    calldata: vec![recipient_felt(), low, high],
                }],
                FeeMode::Paymaster(PaymasterConfig::with_api_key(api_key)),
            )
            .await
            .map_err(|err| match err {
                StarkzapError::PaymasterRequest { .. } => err,
                _ => err,
            })
            .expect("paymaster execute failed");
        println!("paymaster tx: {}", paymaster_tx);
        paymaster_tx.wait().await.expect("paymaster wait failed");
    } else {
        println!("AVNU_API_KEY not set; skipping paymaster leg");
    }
}
