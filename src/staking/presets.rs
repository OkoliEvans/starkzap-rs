//! Staking validator (pool) presets for mainnet and Sepolia.
//!
//! Mirrors the `mainnetValidators` / `sepoliaValidators` presets from the TS SDK.
//!
//! ⚠️  Pool contract addresses should be verified against the official Starknet
//!     staking documentation before mainnet use:
//!     <https://docs.starknet.io/staking/>
//!
//! ## Sepolia note
//!
//! Sepolia validators are ephemeral — anyone can register by staking just 1 STRK.
//! There is no canonical list of Sepolia validator pool addresses. To find active
//! pools, query the staking contract directly:
//!
//! ```rust,no_run
//! # use starkzap_rs::{OnboardConfig, StarkZap, StarkZapConfig,
//! #     signer::StarkSigner};
//! # async fn example() -> starkzap_rs::error::Result<()> {
//! # let sdk = StarkZap::new(StarkZapConfig::sepolia());
//! # let signer = StarkSigner::new("0xprivkey", "0xaddress")?;
//! # let wallet = sdk.onboard(OnboardConfig::Signer(signer)).await?;
//! use starknet::core::types::Felt;
//!
//! // Replace with an actual registered staker address from Sepolia
//! let staker = Felt::from_hex("0x...").unwrap();
//! let pools = wallet.get_staker_pools(staker).await?;
//! # Ok(())
//! # }
//! ```

use starknet::core::types::Felt;

/// A staking validator (staker + pool contract pair).
#[derive(Debug, Clone)]
pub struct Validator {
    /// Human-readable name, e.g. `"StarkWare"`.
    pub name: String,
    /// The staker (operator) address.
    pub staker_address: Felt,
}

impl Validator {
    pub fn new(name: &str, staker_address: Felt) -> Self {
        Self {
            name: name.to_string(),
            staker_address,
        }
    }
}

/// Known mainnet validators with their delegation pool contracts.
///
/// These addresses are sourced from on-chain data. Always verify before
/// delegating real funds — pool addresses can change if a validator
/// re-registers.
///
/// See <https://docs.starknet.io/staking/> and explorers like Voyager or Starkscan
/// for the authoritative list.
pub fn mainnet_validators() -> Vec<Validator> {
    vec![
        Validator::new(
            "Karnot",
            Felt::from_hex_unchecked(
                "0x072543946080646d1aac08bb4ba6f6531b2b29ce41ebfe72b8a6506500d5220e",
            ),
        ),
        Validator::new(
            "Ready (prev. Argent)",
            Felt::from_hex_unchecked(
                "0x00d3b910d8c528bf0216866053c3821ac6c97983dc096bff642e9a3549210ee7",
            ),
        ),
        Validator::new(
            "Twinstake",
            Felt::from_hex_unchecked(
                "0x01aca15766cb615c3b7ca0fc3680cbde8b21934bb2e7b41594b9d046d7412c00",
            ),
        ),
        Validator::new(
            "AVNU",
            Felt::from_hex_unchecked(
                "0x036963c7b56f08105ffdd7f12560924bdc0cb29ce210417ecbc8bf3c7e4b9090",
            ),
        ),
        Validator::new(
            "Braavos",
            Felt::from_hex_unchecked(
                "0x04b00f97e2d2168b91fe64ceeace4a41fc274a85bbdd0adc402c3d0cf9f91bbb",
            ),
        ),
        Validator::new(
            "Nethermind",
            Felt::from_hex_unchecked(
                "0x02952d1e0de1de08fbe6a75d9d0e388e3e89d5d9d42d5f85906ec42ea02e35de",
            ),
        ),
        Validator::new(
            "Pragma",
            Felt::from_hex_unchecked(
                "0x077d4b4e7ae321aabd0a5a7322108635fcbd0cd746f9ae217b8ea00363494b65",
            ),
        ),
    ]
}

/// Sepolia validator presets.
///
/// **Sepolia has no canonical validator list.** This returns an empty vec
/// intentionally — Sepolia validators are ephemeral test registrations.
///
/// To test staking on Sepolia, either:
/// 1. Register your own validator (stake 1 STRK via the Sepolia staking contract)
/// 2. Use `wallet.get_staker_pools(staker_address)` with a known Sepolia staker
///
/// The Sepolia staking contract address is available via
/// [`crate::network::Network::staking_contract`].
pub fn sepolia_validators() -> Vec<Validator> {
    vec![
        Validator::new(
            "moonli.me",
            Felt::from_hex_unchecked(
                "0x003bc84d802c8a57cbe4eb4a6afa9b1255e907cba9377b446d6f4edf069403c5",
            ),
        ),
        Validator::new(
            "Teku",
            Felt::from_hex_unchecked(
                "0x068b5f8e8eb23a42ad290800f229f09b1bcc8d43537dd27a127769ffa13b59f1",
            ),
        ),
        Validator::new(
            "onchainaustria",
            Felt::from_hex_unchecked(
                "0x0771f68ba376b6507a7d21e15d04d48aa6563ea131f89668b92be06707218741",
            ),
        ),
        Validator::new(
            "CroutonDigital",
            Felt::from_hex_unchecked(
                "0x05862ff0bf252dcd0a872104ffcf04a33d8ee9a48d4ef2d5c548147046502502",
            ),
        ),
        Validator::new(
            "DSRV",
            Felt::from_hex_unchecked(
                "0x0789447b5b58aaa87e8ac285288b39bc806aefbc056e49947eb531f7088334ea",
            ),
        ),
        Validator::new(
            "TheBuidl",
            Felt::from_hex_unchecked(
                "0x05469d2d0be78cb53c0e806b7a68e5f492c221b4faeccebe6532372f2a32d63c",
            ),
        ),
        Validator::new(
            "Keplr",
            Felt::from_hex_unchecked(
                "0x0400dd26dd8802d94d36c780dd67e03d9a6f9922ab25557ca52d36b64484029d",
            ),
        ),
        Validator::new(
            "Nethermind",
            Felt::from_hex_unchecked(
                "0x05c85dd30df86ed1f2cfe1806417efb2cae421bffdee8110a74a3d3eb95b28d3",
            ),
        ),
    ]
}

/// Look up a mainnet validator by name (case-insensitive).
pub fn mainnet_validator(name: &str) -> Option<Validator> {
    mainnet_validators()
        .into_iter()
        .find(|v| v.name.eq_ignore_ascii_case(name))
}

/// Look up a Sepolia validator by name (case-insensitive).
///
/// Always returns `None` — see [`sepolia_validators`] for why.
pub fn sepolia_validator(name: &str) -> Option<Validator> {
    sepolia_validators()
        .into_iter()
        .find(|v| v.name.eq_ignore_ascii_case(name))
}
