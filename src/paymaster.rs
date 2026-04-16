//! Paymaster integration.
//!
//! The public API mirrors the concepts used by StarkZap TS:
//! - `PaymasterDetails`
//! - `build_paymaster_transaction`
//! - `execute_paymaster_transaction`
//!
//! The backend implementation mirrors StarkZap TS by speaking to AVNU's
//! paymaster JSON-RPC endpoint (`PaymasterRpc` / SNIP-29 style).

use reqwest::{Client, header::{CONTENT_TYPE, HeaderMap, HeaderValue}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use starknet::core::types::{typed_data::TypeReference, Call, Felt, TypedData};
use starknet_crypto::poseidon_hash_many;
use tracing::trace;

use crate::{
    error::{Result, StarkzapError},
    network::Network,
};

/// Paymaster fee mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymasterFeeMode {
    /// The dApp sponsors the execution.
    Sponsored,
    /// The user pays fees in the given gas token.
    Gasless(Felt),
}

/// Optional execution time window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeBounds {
    pub execute_after: u64,
    pub execute_before: u64,
}

impl TimeBounds {
    pub const fn new(execute_after: u64, execute_before: u64) -> Self {
        Self {
            execute_after,
            execute_before,
        }
    }
}

/// Optional account deployment payload used by paymaster-backed onboarding flows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDeploymentData {
    pub address: Felt,
    pub class_hash: Felt,
    pub salt: Felt,
    pub constructor_calldata: Vec<Felt>,
}

impl AccountDeploymentData {
    pub fn new(
        address: Felt,
        class_hash: Felt,
        salt: Felt,
        constructor_calldata: Vec<Felt>,
    ) -> Self {
        Self {
            address,
            class_hash,
            salt,
            constructor_calldata,
        }
    }
}

/// High-level paymaster configuration matching the TS SDK mental model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymasterDetails {
    pub fee_mode: PaymasterFeeMode,
    pub deployment_data: Option<AccountDeploymentData>,
    pub time_bounds: Option<TimeBounds>,
    pub max_fee_in_gas_token: Option<Felt>,
}

impl PaymasterDetails {
    pub const fn sponsored() -> Self {
        Self {
            fee_mode: PaymasterFeeMode::Sponsored,
            deployment_data: None,
            time_bounds: None,
            max_fee_in_gas_token: None,
        }
    }

    pub const fn gasless(gas_token: Felt) -> Self {
        Self {
            fee_mode: PaymasterFeeMode::Gasless(gas_token),
            deployment_data: None,
            time_bounds: None,
            max_fee_in_gas_token: None,
        }
    }

    pub fn with_deployment_data(mut self, deployment_data: AccountDeploymentData) -> Self {
        self.deployment_data = Some(deployment_data);
        self
    }

    pub fn with_time_bounds(mut self, time_bounds: TimeBounds) -> Self {
        self.time_bounds = Some(time_bounds);
        self
    }

    pub fn with_max_fee_in_gas_token(mut self, max_fee_in_gas_token: Felt) -> Self {
        self.max_fee_in_gas_token = Some(max_fee_in_gas_token);
        self
    }
}

/// Backward-compatible convenience config for `FeeMode::Paymaster`.
#[derive(Debug, Clone)]
pub struct PaymasterConfig {
    pub api_key: Option<String>,
    pub gas_token: Option<Felt>,
}

impl PaymasterConfig {
    pub fn new() -> Self {
        Self::sepolia_free()
    }

    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            api_key: Some(api_key.into()),
            gas_token: None,
        }
    }

    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("AVNU_API_KEY").ok(),
            gas_token: None,
        }
    }

    pub fn sepolia_free() -> Self {
        Self {
            api_key: None,
            gas_token: None,
        }
    }

    pub fn gasless(gas_token: Felt) -> Self {
        Self {
            api_key: None,
            gas_token: Some(gas_token),
        }
    }

    pub fn details(&self) -> PaymasterDetails {
        match self.gas_token {
            Some(token) => PaymasterDetails::gasless(token),
            None => PaymasterDetails::sponsored(),
        }
    }
}

/// Fee payment mode for [`crate::wallet::Wallet::execute`].
#[derive(Debug, Clone)]
pub enum FeeMode {
    UserPays,
    Paymaster(PaymasterConfig),
}

/// Prepared paymaster transaction returned by `build_paymaster_transaction`.
#[derive(Debug, Clone)]
pub struct PreparedPaymasterTransaction {
    calls: Vec<Call>,
    details: PaymasterDetails,
    typed_data: Value,
    typed_data_hash: Felt,
    transaction_type: PaymasterTransactionType,
    execution_parameters: Value,
    deployment_payload: Option<Value>,
}

impl PreparedPaymasterTransaction {
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }

    pub fn details(&self) -> &PaymasterDetails {
        &self.details
    }

    pub fn typed_data(&self) -> &Value {
        &self.typed_data
    }

    pub const fn typed_data_hash(&self) -> Felt {
        self.typed_data_hash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaymasterTransactionType {
    Invoke,
    DeployAndInvoke,
}

const PAYMASTER_RPC_VERSION: &str = "0x1";

// ── AVNU paymaster JSON-RPC request/response shapes ──────────────────────────

#[derive(Serialize)]
struct JsonRpcRequest<'a, T> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<T>,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Value,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Serialize)]
struct BuildTransactionParams {
    transaction: BuildTransaction,
    parameters: ExecutionParameters,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BuildTransaction {
    Invoke {
        invoke: BuildInvokeTransaction,
    },
    DeployAndInvoke {
        deployment: AccountDeploymentPayload,
        invoke: BuildInvokeTransaction,
    },
}

#[derive(Serialize)]
struct AvnuCall {
    to: String,
    selector: String,
    calldata: Vec<String>,
}

#[derive(Serialize)]
struct BuildInvokeTransaction {
    user_address: String,
    calls: Vec<AvnuCall>,
}

#[derive(Serialize)]
struct AccountDeploymentPayload {
    address: String,
    class_hash: String,
    salt: String,
    calldata: Vec<String>,
    version: &'static str,
}

#[derive(Serialize)]
struct ExecutionParameters {
    version: &'static str,
    fee_mode: PaymasterRpcFeeMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_bounds: Option<PaymasterRpcTimeBounds>,
}

#[derive(Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum PaymasterRpcFeeMode {
    Sponsored,
    Default {
        gas_token: String,
    },
}

#[derive(Serialize)]
struct PaymasterRpcTimeBounds {
    execute_after: u64,
    execute_before: u64,
}

#[derive(Serialize)]
struct ExecutableInvokeTransaction {
    user_address: String,
    typed_data: Value,
    signature: Vec<String>,
}

#[derive(Deserialize)]
struct ExecuteResponse {
    transaction_hash: String,
}

// ── PaymasterClient ────────────────────────────────────────────────────────────

pub(crate) struct PaymasterClient {
    client: Client,
    base_url: String,
    next_request_id: std::sync::atomic::AtomicU64,
}

impl PaymasterClient {
    pub fn new(network: &Network, api_key: Option<String>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(key) = api_key.as_deref() {
            if let Ok(value) = HeaderValue::from_str(key) {
                headers.insert("x-paymaster-api-key", value.clone());
                headers.insert("x-api-key", value);
            }
        }

        Self {
            client: Client::builder()
                .default_headers(headers)
                .build()
                .expect("paymaster reqwest client"),
            base_url: network.avnu_paymaster_url().to_string(),
            next_request_id: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub async fn build_transaction(
        &self,
        account_address: Felt,
        calls: Vec<Call>,
        details: PaymasterDetails,
    ) -> Result<PreparedPaymasterTransaction> {
        self.validate_details(&details)?;

        let deployment_data = details.deployment_data.clone();
        let request = BuildTransactionParams {
            transaction: match deployment_data.as_ref() {
                Some(deployment) => BuildTransaction::DeployAndInvoke {
                    deployment: serialize_deployment_data(deployment),
                    invoke: BuildInvokeTransaction {
                        user_address: format!("{:#x}", account_address),
                        calls: serialize_calls(&calls),
                    },
                },
                None => BuildTransaction::Invoke {
                    invoke: BuildInvokeTransaction {
                        user_address: format!("{:#x}", account_address),
                        calls: serialize_calls(&calls),
                    },
                },
            },
            parameters: serialize_execution_parameters(&details),
        };

        let response = self
            .rpc("paymaster_buildTransaction", &request)
            .await?;
        let transaction_type = match response
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("invoke")
        {
            "deploy_and_invoke" => PaymasterTransactionType::DeployAndInvoke,
            _ => PaymasterTransactionType::Invoke,
        };
        let typed_data = response
            .get("typed_data")
            .cloned()
            .ok_or_else(|| StarkzapError::PaymasterMalformed {
                field: "result.typed_data".into(),
            })?;
        let execution_parameters = response
            .get("parameters")
            .cloned()
            .ok_or_else(|| StarkzapError::PaymasterMalformed {
                field: "result.parameters".into(),
            })?;
        let deployment_payload = response.get("deployment").cloned();
        let typed_data_hash = hash_typed_data(&typed_data, account_address)?;

        Ok(PreparedPaymasterTransaction {
            calls,
            details,
            typed_data,
            typed_data_hash,
            transaction_type,
            execution_parameters,
            deployment_payload,
        })
    }

    fn validate_details(&self, details: &PaymasterDetails) -> Result<()> {
        if details.max_fee_in_gas_token.is_some() {
            return Err(StarkzapError::PaymasterUnsupported {
                feature: "max_fee_in_gas_token is not yet supported by the current paymaster transport".into(),
            });
        }

        Ok(())
    }

    pub async fn execute_prepared<F, Fut>(
        &self,
        account_address: Felt,
        prepared: PreparedPaymasterTransaction,
        sign: F,
    ) -> Result<Felt>
    where
        F: FnOnce(Felt) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<Felt>>>,
    {
        let signature = sign(prepared.typed_data_hash).await?;
        let invoke = ExecutableInvokeTransaction {
            user_address: format!("{:#x}", account_address),
            typed_data: prepared.typed_data,
            signature: signature.iter().map(|felt| format!("{:#x}", felt)).collect(),
        };
        let transaction = match (prepared.transaction_type, prepared.deployment_payload) {
            (PaymasterTransactionType::Invoke, _) => Ok(json!({
                "type": "invoke",
                "invoke": invoke,
            })),
            (PaymasterTransactionType::DeployAndInvoke, Some(deployment)) => Ok(json!({
                "type": "deploy_and_invoke",
                "deployment": deployment,
                "invoke": invoke,
            })),
            (PaymasterTransactionType::DeployAndInvoke, None) => Err(
                StarkzapError::PaymasterMalformed {
                    field: "missing deployment payload for deploy_and_invoke transaction".into(),
                },
            ),
        }?;
        let request = json!({
            "transaction": transaction,
            "parameters": prepared.execution_parameters,
        });

        let response: ExecuteResponse = serde_json::from_value(
            self.rpc("paymaster_executeTransaction", &request).await?,
        )
        .map_err(|e| StarkzapError::PaymasterMalformed {
            field: e.to_string(),
        })?;

        Felt::from_hex(&response.transaction_hash).map_err(|_| StarkzapError::PaymasterMalformed {
            field: format!("invalid transaction hash: {}", response.transaction_hash),
        })
    }

    async fn rpc<T: Serialize>(&self, method: &str, params: &T) -> Result<Value> {
        let request_id = self
            .next_request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: request_id,
            method,
            params: Some(params),
        };

        let response = self.client.post(&self.base_url).json(&request).send().await?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        trace!("paymaster {} {} -> HTTP {} body: {}", self.base_url, method, status, text);

        if !(200..=299).contains(&(status as usize)) {
            return Err(StarkzapError::PaymasterRequest { status, body: text });
        }

        let body: JsonRpcResponse =
            serde_json::from_str(&text).map_err(|e| StarkzapError::PaymasterMalformed {
                field: e.to_string(),
            })?;

        if let Some(error) = body.error {
            let payload = json!({
                "code": error.code,
                "message": error.message,
                "data": error.data,
            });
            return Err(StarkzapError::PaymasterRequest {
                status,
                body: payload.to_string(),
            });
        }

        body.result.ok_or_else(|| StarkzapError::PaymasterMalformed {
            field: format!("missing result for method {method}"),
        })
    }
}

fn serialize_calls(calls: &[Call]) -> Vec<AvnuCall> {
    calls.iter()
        .map(|call| AvnuCall {
            to: format!("{:#x}", call.to),
            selector: format!("{:#x}", call.selector),
            calldata: call.calldata.iter().map(|felt| format!("{:#x}", felt)).collect(),
        })
        .collect()
}

fn serialize_execution_parameters(details: &PaymasterDetails) -> ExecutionParameters {
    ExecutionParameters {
        version: PAYMASTER_RPC_VERSION,
        fee_mode: match details.fee_mode {
            PaymasterFeeMode::Sponsored => PaymasterRpcFeeMode::Sponsored,
            PaymasterFeeMode::Gasless(token) => PaymasterRpcFeeMode::Default {
                gas_token: format!("{:#x}", token),
            },
        },
        time_bounds: details.time_bounds.map(|bounds| PaymasterRpcTimeBounds {
            execute_after: if bounds.execute_after == 0 {
                1
            } else {
                bounds.execute_after
            },
            execute_before: bounds.execute_before,
        }),
    }
}

fn serialize_deployment_data(data: &AccountDeploymentData) -> AccountDeploymentPayload {
    AccountDeploymentPayload {
        address: format!("{:#x}", data.address),
        class_hash: format!("{:#x}", data.class_hash),
        salt: format!("{:#x}", data.salt),
        calldata: data
            .constructor_calldata
            .iter()
            .map(|felt| format!("{:#x}", felt))
            .collect(),
        version: "0x1",
    }
}

pub(crate) fn hash_typed_data(typed_data: &Value, account_address: Felt) -> Result<Felt> {
    let typed_data: TypedData =
        serde_json::from_value(typed_data.clone()).map_err(|e| StarkzapError::PaymasterMalformed {
            field: format!("cannot deserialize typed data: {}", e),
        })?;

    if is_outside_execution_typed_data(&typed_data) {
        return hash_outside_execution_typed_data(typed_data, account_address);
    }

    typed_data
        .message_hash(account_address)
        .map_err(|e| StarkzapError::PaymasterMalformed {
            field: format!("typed data hash error: {}", e),
        })
}

fn is_outside_execution_typed_data(typed_data: &TypedData) -> bool {
    typed_data.primary_type().signature_ref_repr() == "OutsideExecution"
}

fn hash_outside_execution_typed_data(
    typed_data: TypedData,
    account_address: Felt,
) -> Result<Felt> {
    const STARKNET_MESSAGE_PREFIX: Felt = Felt::from_raw([
        257012186512350467,
        18446744073709551605,
        10480951322775611302,
        16156019428408348868,
    ]);
    const OUTSIDE_EXECUTION_TYPE_HASH: Felt = Felt::from_hex_unchecked(
        "0x312b56c05a7965066ddbda31c016d8d05afc305071c0ca3cdc2192c3c2f1f0f",
    );

    let domain_hash = typed_data.encoder().domain().encoded_hash();
    let message = typed_data.message();
    let message_object = match message {
        starknet::core::types::typed_data::Value::Object(object) => object,
        _ => {
            return Err(StarkzapError::PaymasterMalformed {
                field: "typed_data.message must be an object".into(),
            });
        }
    };

    let caller = object_field_felt(message_object, "Caller")?;
    let nonce = object_field_felt(message_object, "Nonce")?;
    let execute_after = object_field_felt(message_object, "Execute After")?;
    let execute_before = object_field_felt(message_object, "Execute Before")?;
    let calls = object_field_array(message_object, "Calls")?;

    let call_hashes = calls
        .iter()
        .map(hash_outside_execution_call)
        .collect::<Result<Vec<_>>>()?;
    let outside_hash = poseidon_hash_many(&[
        OUTSIDE_EXECUTION_TYPE_HASH,
        caller,
        nonce,
        execute_after,
        execute_before,
        poseidon_hash_many(&call_hashes),
    ]);

    Ok(poseidon_hash_many(&[
        STARKNET_MESSAGE_PREFIX,
        domain_hash,
        account_address,
        outside_hash,
    ]))
}

fn hash_outside_execution_call(
    value: &starknet::core::types::typed_data::Value,
) -> Result<Felt> {
    const CALL_TYPE_HASH: Felt = Felt::from_hex_unchecked(
        "0x3635c7f2a7ba93844c0d064e18e487f35ab90f7c39d00f186a781fc3f0c2ca9",
    );

    let object = match value {
        starknet::core::types::typed_data::Value::Object(object) => object,
        _ => {
            return Err(StarkzapError::PaymasterMalformed {
                field: "typed_data.message.Calls[] must be objects".into(),
            });
        }
    };

    let to = object_field_felt(object, "To")?;
    let selector = object_field_felt(object, "Selector")?;
    let calldata = object_field_array(object, "Calldata")?
        .iter()
        .map(value_to_felt)
        .collect::<Result<Vec<_>>>()?;

    Ok(poseidon_hash_many(&[
        CALL_TYPE_HASH,
        to,
        selector,
        poseidon_hash_many(&calldata),
    ]))
}

fn object_field_felt(
    object: &starknet::core::types::typed_data::ObjectValue,
    name: &str,
) -> Result<Felt> {
    let value = object.fields.get(name).ok_or_else(|| StarkzapError::PaymasterMalformed {
        field: format!("typed_data.message.{name}"),
    })?;
    value_to_felt(value)
}

fn object_field_array<'a>(
    object: &'a starknet::core::types::typed_data::ObjectValue,
    name: &str,
) -> Result<&'a [starknet::core::types::typed_data::Value]> {
    let value = object.fields.get(name).ok_or_else(|| StarkzapError::PaymasterMalformed {
        field: format!("typed_data.message.{name}"),
    })?;

    match value {
        starknet::core::types::typed_data::Value::Array(array) => Ok(&array.elements),
        _ => Err(StarkzapError::PaymasterMalformed {
            field: format!("typed_data.message.{name} must be an array"),
        }),
    }
}

fn value_to_felt(value: &starknet::core::types::typed_data::Value) -> Result<Felt> {
    match value {
        starknet::core::types::typed_data::Value::String(string) => Felt::from_hex(string)
            .or_else(|_| Felt::from_dec_str(string))
            .map_err(|_| StarkzapError::PaymasterMalformed {
                field: format!("invalid felt value: {string}"),
            }),
        starknet::core::types::typed_data::Value::UnsignedInteger(value) => Ok((*value).into()),
        _ => Err(StarkzapError::PaymasterMalformed {
            field: "expected felt-like value".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::hash_typed_data;
    use serde_json::json;
    use starknet::core::types::Felt;

    #[test]
    fn paymaster_outside_execution_hash_matches_starknet_js() {
        let typed_data = json!({
            "types": {
                "StarknetDomain": [
                    {"name": "name", "type": "shortstring"},
                    {"name": "version", "type": "shortstring"},
                    {"name": "chainId", "type": "shortstring"},
                    {"name": "revision", "type": "shortstring"}
                ],
                "OutsideExecution": [
                    {"name": "Caller", "type": "ContractAddress"},
                    {"name": "Nonce", "type": "felt"},
                    {"name": "Execute After", "type": "u128"},
                    {"name": "Execute Before", "type": "u128"},
                    {"name": "Calls", "type": "Call*"}
                ],
                "Call": [
                    {"name": "To", "type": "ContractAddress"},
                    {"name": "Selector", "type": "selector"},
                    {"name": "Calldata", "type": "felt*"}
                ]
            },
            "domain": {
                "name": "Account.execute_from_outside",
                "version": "2",
                "chainId": "SN_SEPOLIA",
                "revision": "1"
            },
            "primaryType": "OutsideExecution",
            "message": {
                "Caller": "0x75a180e18e56da1b1cae181c92a288f586f5fe22c18df21cf97886f1e4b316c",
                "Nonce": "0x1e8595ecbf1167aeb043d877bb7bf365",
                "Execute After": "0x1",
                "Execute Before": "0x69e00ac1",
                "Calls": [
                    {
                        "To": "0x4718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d",
                        "Selector": "0x83afd3f4caedc6eebf44246fe54e38c95e3179a5ec9ea81740eca5b482d12e",
                        "Calldata": [
                            "0x4c14d3284fc6b7236a08c2cce94e09ea774c749230402d2aa7ea94c58f38ca0",
                            "0x38d7ea4c68000",
                            "0x0"
                        ]
                    }
                ]
            }
        });

        let account = Felt::from_hex_unchecked(
            "0x40e9753e4a2079f0ecc266c14caaa82ff02c37951a496ba567a439b5c2275ee",
        );
        let expected = Felt::from_hex_unchecked(
            "0x3a8c954abadbd829713790effcc6b15d01df404dd4d2169538387bf8e919e50",
        );

        let actual = hash_typed_data(&typed_data, account).unwrap();
        assert_eq!(actual, expected);
    }
}
