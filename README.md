# starkzap-rs

A Rust SDK for seamless Starknet wallet integration — the faithful Rust mirror of [starkzap](https://github.com/keep-starknet-strange/starkzap).

Built for the Starknet community.

---

## Installation

```toml
[dependencies]
starkzap-rs = { git = "https://github.com/your-org/starkzap-rs" }

# Optional signers
starkzap-rs = { git = "...", features = ["privy"] }
starkzap-rs = { git = "...", features = ["cartridge"] }
starkzap-rs = { git = "...", features = ["full"] }  # all signers
```

Once published to crates.io:
```toml
starkzap-rs = "0.1"
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

## Features

| Feature | Description | Enabled by default |
|---|---|---|
| *(base)* | StarkSigner, tokens, transfers, staking, paymaster | ✅ |
| `privy` | Privy server-side signer | ❌ |
| `cartridge` | Cartridge session-key signer | ❌ |
| `full` | All optional signers | ❌ |
| `wasm` | WebAssembly target support | ❌ |

---

## Supported Targets

| Target | How |
|---|---|
| **Server / CLI** (tokio) | Default — no feature flags needed |
| **WASM / browser** | `--features wasm --target wasm32-unknown-unknown` |
| **React Native / mobile** | Via WASM or native Rust via FFI |

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

**Public fallback endpoints** (BlastAPI, used when no `rpc_url` is provided):
- Mainnet: `https://starknet-mainnet.public.blastapi.io/rpc/v0_8`
- Sepolia: `https://starknet-sepolia.public.blastapi.io/rpc/v0_8`
- Devnet: `http://127.0.0.1:5050/rpc`

For production, always use your own Alchemy/Infura key — public endpoints are rate-limited.

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

// Create a new wallet for a user
let address = privy.create_wallet("user-id-123").await?;

// Or load an existing wallet
let privy = PrivySigner::from_env()?.with_wallet("wallet_id", address_felt);
```

**Setup:**
1. Create a Privy app at https://privy.io
2. Dashboard → Settings → API Keys → copy App ID and App Secret
3. Set `PRIVY_APP_ID` and `PRIVY_APP_SECRET`

### CartridgeSigner (`cartridge` feature)

Uses a session key pre-issued by the Cartridge Controller browser wallet.

**Browser flow (JavaScript):**
```javascript
const session = await controller.createSession({
    expiresAt: Date.now() + 86400000, // 24h
    policies: [{ contractAddress: "0x...", selector: "transfer" }],
})
// Send to your Rust backend:
const sessionKey = session.sessionKeyPair.privateKey
const accountAddress = controller.address
```

**Rust backend:**
```rust
use starkzap_rs::signer::CartridgeSigner;

let signer = CartridgeSigner::new(&session_key, &account_address)?;
let wallet = sdk.onboard(OnboardConfig::Cartridge(signer)).await?;
```

> Cartridge's primary auth flow (passkeys, biometrics) is browser-native and
> cannot be replicated server-side. The session key pattern is Cartridge's own
> designed mechanism for delegating to backends.

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
let wsteth = mainnet::wsteth(); // 18 decimals

// Sepolia
let usdc = sepolia::usdc();
let strk = sepolia::strk();
let eth  = sepolia::eth();

// By symbol
let tok = mainnet::by_symbol("USDC").unwrap();
```

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
use starkzap_rs::paymaster::{FeeMode, PaymasterConfig};

// Sepolia — no API key needed
let config = PaymasterConfig::new();

// Mainnet — API key required
let config = PaymasterConfig::with_api_key("your_key");
// or from env: AVNU_API_KEY=...
let config = PaymasterConfig::from_env();

// Execute any calls gaslessly
let tx = wallet.execute(calls, FeeMode::Paymaster(config)).await?;
tx.wait().await?;
```

Obtain a mainnet API key at https://app.avnu.fi.

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
let pool = validators[0].pool_address;

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

# Run an example
cargo run --example basic_transfer
cargo run --example staking_flow
cargo run --example privy_signer --features privy
cargo run --example cartridge_signer --features cartridge

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
- [x] Privy server signer (wallet creation)
- [x] Cartridge session-key signer
- [ ] Full Privy → starknet-rs signing delegation (#1)
- [ ] Auto account deploy (UDC, ArgentX v0.4/v0.5)
- [ ] Token/validator preset codegen script
- [ ] WASM build verification
- [ ] Published to crates.io

---

## License

MIT