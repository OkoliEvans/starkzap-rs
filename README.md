# starkzap-rs

[![crates.io](https://img.shields.io/crates/v/starkzap-rs.svg)](https://crates.io/crates/starkzap-rs)
[![docs.rs](https://docs.rs/starkzap-rs/badge.svg)](https://docs.rs/starkzap-rs)

A Rust SDK for seamless Starknet wallet integration — the faithful Rust mirror of [starkzap](https://github.com/keep-starknet-strange/starkzap).

Built for the Starknet community.

---

## Installation

```toml
[dependencies]
starkzap-rs = "0.1.0"

# Optional signers / helpers
starkzap-rs = { version = "0.1.0", features = ["privy"] }
starkzap-rs = { version = "0.1.0", features = ["cartridge"] }
starkzap-rs = { version = "0.1.0", features = ["full"] }
```

Git install is still available if you want the latest repository version:
```toml
starkzap-rs = { git = "https://github.com/OkoliEvans/starkzap-rs" }
```

---

## Quick Start

```rust
use starkzap_rs::{
    Amount, OnboardConfig, Recipient, StarkZap, StarkZapConfig,
    signer::StarkSigner,
    tokens::sepolia,
    wallet::DeployPolicy,
};

#[tokio::main]
async fn main() -> starkzap_rs::error::Result<()> {
    // 1. Initialise SDK — network preset, optional rpc_url override
    let sdk = StarkZap::new(StarkZapConfig::sepolia());

    // 2. Connect wallet
    let signer = StarkSigner::new(
        &std::env::var("PRIVATE_KEY").unwrap(),
        &std::env::var("ACCOUNT_ADDRESS").unwrap(),
    )?;
    let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;

    // Auto-deploy a funded fresh account on first use.
    wallet.ensure_ready(DeployPolicy::IfNeeded).await?;

    // 3. Check balance
    let strk = sepolia::strk();
    let balance = wallet.balance_of(&strk).await?;
    println!("Balance: {}", balance); // "150.25 STRK"

    // 4. Transfer
    use starknet::core::types::Felt;
    let recipient = Felt::from_hex("0xRECIPIENT").unwrap();
    let tx = wallet.transfer(&strk, vec![
        Recipient::new(recipient, Amount::parse("10", &strk)?),
    ]).await?;
    tx.wait().await?;

    Ok(())
}
```

---

## Dependencies

The SDK depends on:

- [`starknet`](https://crates.io/crates/starknet) (`0.17`) for Starknet accounts, providers, and signing
- `tokio` for async runtime

These are installed automatically when you add `starkzap-rs`.

### Cargo features

Like StarkZap TS uses optional integrations and peer dependencies, `starkzap-rs`
uses optional Cargo features:

| Feature | What it enables |
|---|---|
| `privy` | Privy server-side signer |
| `cartridge` | Cartridge session signer |
| `full` | All optional signers |
| `wasm` | WASM-target crate builds |

The crate uses `default = []`, so the base SDK works without enabling any feature.

### Targets

- **Server / CLI** (tokio) — first-class target
- **Browser helper tooling** — see `examples/cartridge_session_web/` for the Cartridge session exporter
- **WASM crate build** — verified with `cargo check --features wasm --target wasm32-unknown-unknown`

---

## Networks

```rust
// Preset networks
let sdk = StarkZap::new(StarkZapConfig::mainnet());
let sdk = StarkZap::new(StarkZapConfig::sepolia());
let sdk = StarkZap::new(StarkZapConfig {
    network: Network::Devnet,
    rpc_url: None, // defaults to http://127.0.0.1:5050
});

// Custom RPC (Alchemy, Infura, etc.)
let sdk = StarkZap::new(StarkZapConfig::sepolia().with_rpc(
    "https://starknet-sepolia.g.alchemy.com/starknet/version/rpc/v0_8/YOUR_KEY"
));
```

**Default fallback endpoints** (used when no `rpc_url` is provided):
- Mainnet: `https://starknet.drpc.org`
- Sepolia: `https://starknet-sepolia.drpc.org`
- Devnet: `http://127.0.0.1:5050/rpc`

For production, always use your own RPC key — public endpoints are rate-limited.

---

## Signers

### StarkSigner (raw private key)

Suitable for server-side scripts, CI/CD, and developer demos.

```rust
let signer = StarkSigner::new("0xPRIVATE_KEY", "0xACCOUNT_ADDRESS")?;
let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
```

### PrivySigner (`privy` feature)

Server-side embedded wallets. Your backend never holds the private key.

```sh
# env vars required
PRIVY_APP_ID=clxxxxxxxxxxxxxxxx
PRIVY_APP_SECRET=privy_secret_...
```

```rust
use starkzap_rs::signer::PrivySigner;

let mut privy = PrivySigner::from_env()?;

// Create a new wallet for a user and persist the returned metadata
let wallet = privy.create_wallet_info("user-id-123").await?;

// Or load an existing wallet
let privy = PrivySigner::from_env()?.with_wallet_and_public_key(
    wallet.wallet_id,
    wallet.address,
    wallet.public_key.unwrap(),
);
```

**Setup:**
1. Create a Privy app at https://privy.io
2. Dashboard → Settings → API Keys → copy App ID and App Secret
3. Persist `wallet_id`, `address`, and `public_key` from the first wallet creation
4. Set `PRIVY_APP_ID` and `PRIVY_APP_SECRET`

### CartridgeSigner (`cartridge` feature)

Uses a full session bundle exported from the Cartridge Controller flow.

Use the helper app in [`examples/cartridge_session_web/`](examples/cartridge_session_web) to:
- authenticate with Cartridge
- approve session policies
- export `CARTRIDGE_SESSION_BUNDLE_B64`

**Rust backend:**
```rust
use starkzap_rs::signer::CartridgeSigner;

let signer = CartridgeSigner::from_env()?;
let wallet = sdk.onboard(OnboardConfig::Cartridge(signer)).await?;
```

> Cartridge's primary auth flow (passkeys, biometrics, username/password, social)
> is browser-native. Rust consumes the exported session bundle and executes through
> the registered session account model.

Helper app setup:

```sh
cd examples/cartridge_session_web
npm install
npm run dev
```

Then open the local Vite URL, create a session, and copy `CARTRIDGE_SESSION_BUNDLE_B64`
into your `.env`.

---

## Token Presets

```rust
use starkzap_rs::tokens::{mainnet, sepolia};

// Mainnet
let usdc  = mainnet::usdc();    // 6 decimals
let usdc_e = mainnet::usdc_e(); // 6 decimals (bridged)
let strk  = mainnet::strk();    // 18 decimals
let eth   = mainnet::eth();     // 18 decimals
let wbtc  = mainnet::wbtc();    // 8 decimals
let tbtc  = mainnet::tbtc();    // 18 decimals
let lbtc  = mainnet::lbtc();    // 8 decimals
let xwbtc = mainnet::xwbtc();   // 8 decimals
let solvbtc = mainnet::solvbtc(); // 18 decimals
let wsteth = mainnet::wsteth(); // 18 decimals

// Sepolia
let usdc = sepolia::usdc();
let strk = sepolia::strk();
let eth  = sepolia::eth();
let wbtc = sepolia::wbtc();
let tbtc = sepolia::tbtc();
let lbtc = sepolia::lbtc();

// By symbol
let tok = mainnet::by_symbol("USDC").unwrap();
```

Token and validator preset data is generated at build time from:
- `codegen/presets/tokens.json`
- `codegen/presets/validators.json`

That keeps the Rust API stable while making preset updates data-driven.
BTC-family Starknet assets from StarkZap TS V1 are included in these generated presets.

---

## Amount

```rust
let usdc = mainnet::usdc();
let amount = Amount::parse("10.5", &usdc)?;

amount.raw();              // 10_500_000 (raw u128, 6 decimals)
amount.to_formatted();     // "10.5 USDC"
amount.to_decimal_string(); // "10.5"
amount.to_u256_felts();    // [Felt(10_500_000), Felt::ZERO]
amount.is_zero();          // false
```

---

## Paymaster (AVNU Gasless)

```rust
use starkzap_rs::paymaster::{FeeMode, PaymasterConfig, PaymasterDetails};

// Sepolia — no API key needed
let config = PaymasterConfig::new();

// Mainnet — API key required
let config = PaymasterConfig::with_api_key("your_key");
// or from env: AVNU_API_KEY=...
let config = PaymasterConfig::from_env();

// Execute any calls gaslessly
let tx = wallet.execute(calls, FeeMode::Paymaster(config)).await?;
tx.wait().await?;

// TS-style explicit paymaster flow
let details = PaymasterDetails::sponsored();
let hash = wallet
    .execute_paymaster_transaction(calls, details, std::env::var("AVNU_API_KEY").ok())
    .await?;
```

Obtain a mainnet API key at https://app.avnu.fi.

If an account class is not compatible with sponsored execution, the SDK now
falls back to normal `user_pays` execution instead of failing the entire flow.

---

## Live Test Harnesses

For deliberate real-network verification, the repo includes ignored live tests:

```sh
# Sepolia / generic live checks
cargo test --test integration_tests -- --ignored --nocapture

# Mainnet STRK systems
cargo test --test mainnet_live -- --ignored --nocapture

# Mainnet WBTC systems
cargo test --test mainnet_wbtc_live -- --ignored --nocapture
```

Notes:
- these tests can submit real transactions
- `tests/mainnet_live.rs` includes optional staking writes gated behind `RUN_MAINNET_STAKING_WRITES=1`
- `tests/mainnet_wbtc_live.rs` uses `MAINNET_WBTC_TRANSFER_AMOUNT` for live WBTC checks
- paymaster tests run only when `AVNU_API_KEY` is set

---

## Staking

```rust
use starkzap_rs::{
    staking::presets::{mainnet_validators, sepolia_validators},
    paymaster::FeeMode,
    tokens::mainnet,
};

let strk = mainnet::strk();
let validators = mainnet_validators();
let pool = wallet.get_staker_pools(validators[0].staker_address).await?[0].address;

// Enter pool (stake)
let tx = wallet.enter_pool(
    &strk,
    pool,
    Amount::parse("100", &strk)?,
    wallet.address(),      // reward address
    FeeMode::UserPays,
).await?;
tx.wait().await?;

// Check position
let pos = wallet.get_pool_position(pool, &strk).await?;
println!("Staked: {}  Rewards: {}", pos.staked, pos.rewards);

// Add more
let tx = wallet.add_to_pool(&strk, pool, Amount::parse("50", &strk)?, FeeMode::UserPays).await?;

// Claim rewards
let tx = wallet.claim_rewards(pool, FeeMode::UserPays).await?;

// Exit (two-step: intent → wait cooldown → finalise)
let tx = wallet.exit_pool_intent(pool, pos.staked, FeeMode::UserPays).await?;
// ... wait ~21 days on mainnet ...
let tx = wallet.exit_pool(pool, FeeMode::UserPays).await?;

// Discover pools
let staker_addrs: Vec<_> = mainnet_validators().into_iter().map(|v| v.staker_address).collect();
let pools = wallet.discover_my_pools(staker_addrs).await?;
```

---

## Transactions

```rust
let tx = wallet.transfer(...).await?;

// Wait for confirmation (polls with exponential backoff)
let receipt = tx.wait().await?;

// Custom timeout
let receipt = tx.wait_with_options(60, std::time::Duration::from_secs(3)).await?;

// Poll current status
let status = tx.status().await?; // TxStatus::Pending | Accepted | Reverted | Rejected

// Stream updates
use tokio_stream::StreamExt;
let mut stream = tx.watch(std::time::Duration::from_secs(3));
while let Some(status) = stream.next().await {
    println!("{:?}", status);
    if status.is_final() { break; }
}

// Hash
println!("{}", tx.hash_hex()); // "0x..."
```

---

## Documentation

The main usage guide today is this README plus the runnable examples in `examples/`.

For the closest reference model and API design, see the official StarkZap TypeScript SDK:
- https://github.com/keep-starknet-strange/starkzap

---

## Examples

The repo includes runnable examples for the main SDK flows:

- `basic_transfer`
- `paymaster_transfer`
- `staking_flow`
- `privy_signer`
- `cartridge_signer`

For Cartridge session export, use the helper app in `examples/cartridge_session_web/`.

---

## Setup & Development

```sh
# Clone
git clone https://github.com/your-org/starkzap-rs
cd starkzap-rs

# Copy env
cp .env.example .env
# Fill in PRIVATE_KEY, ACCOUNT_ADDRESS, etc.

# Build
cargo build

# Type check
cargo check

# Run all unit tests (no network required)
cargo test

# Run live integration tests (requires RPC or devnet)
# Start devnet first: starknet-devnet --seed 0
cargo test -- --ignored

# Run examples
cargo run --example basic_transfer
cargo run --example staking_flow
cargo run --example paymaster_transfer
cargo run --example privy_signer --features privy
cargo run --example cartridge_signer --features cartridge

# Run the Cartridge session exporter helper
cd examples/cartridge_session_web
npm install
npm run dev

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

### Installing starknet-devnet (for local testing)

```sh
# Python (easier)
pip install starknet-devnet

# or Rust binary
cargo install starknet-devnet

# Start with a fixed seed (reproducible preloaded accounts)
starknet-devnet --seed 0

# Devnet gives you preloaded accounts at http://127.0.0.1:5050
# Check account list at: http://127.0.0.1:5050/predeployed_accounts
```

---

## Roadmap

- [x] Core: StarkSigner, tokens, transfer, execute
- [x] AVNU paymaster
- [x] Staking: enter/exit/rewards/discovery
- [x] Privy signer flow
- [x] Cartridge session flow
- [x] Auto account deploy
- [x] Token/validator preset codegen
- [x] WASM build verification
- [ ] Published to crates.io

---

## License

MIT
