//! Account presets and deployment helpers.

use async_trait::async_trait;
use starknet::{
    accounts::{AccountFactory, PreparedAccountDeploymentV3, RawAccountDeploymentV3},
    core::{
        codec::Encode,
        types::{BlockId, BlockTag, Felt},
        utils::get_contract_address,
    },
    providers::Provider,
    signers::{Signer, SignerInteractivityContext},
};
use starknet_crypto::poseidon_hash_many;

/// Supported account presets mirrored from the official StarkZap SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountPreset {
    Devnet,
    OpenZeppelin,
    Argent,
    Braavos,
    ArgentXV050,
}

impl Default for AccountPreset {
    fn default() -> Self {
        Self::OpenZeppelin
    }
}

impl AccountPreset {
    pub fn from_class_hash(class_hash: Felt) -> Option<Self> {
        [
            Self::Devnet,
            Self::OpenZeppelin,
            Self::Argent,
            Self::Braavos,
            Self::ArgentXV050,
        ]
        .into_iter()
        .find(|preset| preset.class_hash() == class_hash)
    }

    pub fn class_hash(&self) -> Felt {
        match self {
            Self::Devnet => Felt::from_hex_unchecked(
                "0x5b4b537eaa2399e3aa99c4e2e0208ebd6c71bc1467938cd52c798c601e43564",
            ),
            Self::OpenZeppelin => Felt::from_hex_unchecked(
                "0x01d1777db36cdd06dd62cfde77b1b6ae06412af95d57a13dc40ac77b8a702381",
            ),
            Self::Argent => Felt::from_hex_unchecked(
                "0x036078334509b514626504edc9fb252328d1a240e4e948bef8d0c08dff45927f",
            ),
            Self::Braavos => Felt::from_hex_unchecked(
                "0x03d16c7a9a60b0593bd202f660a28c5d76e0403601d9ccc7e4fa253b6a70c201",
            ),
            Self::ArgentXV050 => Felt::from_hex_unchecked(
                "0x073414441639dcd11d1846f287650a00c60c416b9d3ba45d31c651672125b2c2",
            ),
        }
    }

    pub fn salt(&self, public_key: Felt) -> Felt {
        match self {
            Self::Devnet | Self::OpenZeppelin | Self::Argent | Self::Braavos | Self::ArgentXV050 => public_key,
        }
    }

    pub fn constructor_calldata(&self, public_key: Felt) -> Vec<Felt> {
        match self {
            Self::Devnet | Self::OpenZeppelin | Self::Braavos => vec![public_key],
            Self::Argent | Self::ArgentXV050 => {
                let mut calldata = Vec::new();
                ArgentAccountConstructorParams {
                    owner: ArgentSigner::Starknet(public_key),
                    guardian: None,
                }
                .encode(&mut calldata)
                .expect("encoding Argent constructor calldata should not fail");
                calldata
            }
        }
    }

    pub fn counterfactual_address(&self, public_key: Felt) -> Felt {
        get_contract_address(
            self.salt(public_key),
            self.class_hash(),
            &self.constructor_calldata(public_key),
            Felt::ZERO,
        )
    }

    pub fn is_braavos(&self) -> bool {
        matches!(self, Self::Braavos)
    }

    pub fn uses_legacy_execution_encoding(&self) -> bool {
        false
    }

    pub fn requires_invoke_v1(&self) -> bool {
        false
    }
}

/// Braavos implementation class hash used in deploy signatures.
pub const BRAAVOS_IMPL_CLASS_HASH: Felt =
    Felt::from_hex_unchecked("0x03957f9f5a1cbfe918cedc2015c85200ca51a5f7506ecb6de98a5207b759bf8a");

#[derive(Encode)]
#[starknet(core = "starknet::core")]
struct ArgentAccountConstructorParams {
    owner: ArgentSigner,
    guardian: Option<ArgentSigner>,
}

#[derive(Encode)]
#[starknet(core = "starknet::core")]
enum ArgentSigner {
    Starknet(Felt),
}

/// Generic account factory driven by a StarkZap account preset.
#[derive(Debug, Clone)]
pub(crate) struct PresetAccountFactory<S, P> {
    preset: AccountPreset,
    chain_id: Felt,
    public_key: Felt,
    signer: S,
    provider: P,
    block_id: BlockId,
}

impl<S, P> PresetAccountFactory<S, P>
where
    S: Signer,
{
    pub async fn new(
        preset: AccountPreset,
        chain_id: Felt,
        signer: S,
        provider: P,
    ) -> Result<Self, S::GetPublicKeyError> {
        let public_key = signer.get_public_key().await?.scalar();
        Ok(Self {
            preset,
            chain_id,
            public_key,
            signer,
            provider,
            block_id: BlockId::Tag(BlockTag::Latest),
        })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S, P> AccountFactory for PresetAccountFactory<S, P>
where
    S: Signer + Sync + Send,
    P: Provider + Sync + Send,
{
    type Provider = P;
    type SignError = S::SignError;

    fn class_hash(&self) -> Felt {
        self.preset.class_hash()
    }

    fn calldata(&self) -> Vec<Felt> {
        self.preset.constructor_calldata(self.public_key)
    }

    fn chain_id(&self) -> Felt {
        self.chain_id
    }

    fn provider(&self) -> &Self::Provider {
        &self.provider
    }

    fn is_signer_interactive(&self) -> bool {
        self.signer.is_interactive(SignerInteractivityContext::Other)
    }

    fn block_id(&self) -> BlockId {
        self.block_id
    }

    async fn sign_deployment_v3(
        &self,
        deployment: &RawAccountDeploymentV3,
        query_only: bool,
    ) -> Result<Vec<Felt>, Self::SignError> {
        let tx_hash = PreparedAccountDeploymentV3::from_raw(deployment.clone(), self)
            .transaction_hash(query_only);
        let signature = self.signer.sign_hash(&tx_hash).await?;
        let mut output = vec![signature.r, signature.s];

        if self.preset.is_braavos() {
            let aux_data = [
                BRAAVOS_IMPL_CLASS_HASH,
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
                self.chain_id,
            ];
            let aux_hash = poseidon_hash_many(&aux_data);
            let aux_signature = self.signer.sign_hash(&aux_hash).await?;
            output.extend_from_slice(&aux_data);
            output.push(aux_signature.r);
            output.push(aux_signature.s);
        }

        Ok(output)
    }
}
