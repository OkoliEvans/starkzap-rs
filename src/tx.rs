//! Transaction handle returned by all mutating wallet operations.
//!
//! [`Tx`] wraps a transaction hash and a shared provider reference, providing
//! ergonomic methods to wait for confirmation or stream status updates.

use std::{sync::Arc, time::Duration};

use starknet::{
    core::types::{ExecutionResult, Felt, TransactionReceipt, TransactionReceiptWithBlockInfo},
    providers::Provider,
};
use tokio::time::sleep;
use tokio_stream::{wrappers::IntervalStream, StreamExt};
use tracing::{debug, warn};

use crate::error::{Result, StarkzapError};

/// The current status of a submitted transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxStatus {
    /// Transaction is pending — in the mempool or not yet visible.
    Pending,
    /// Included in a block and execution succeeded.
    Accepted,
    /// Included in a block but execution was reverted.
    Reverted { reason: String },
    /// Transaction was rejected before making it on-chain.
    Rejected { reason: String },
}

impl TxStatus {
    /// Returns `true` if the transaction is in a terminal (non-retryable) state.
    pub fn is_final(&self) -> bool {
        !matches!(self, TxStatus::Pending)
    }
}

/// A submitted transaction handle.
///
/// Returned by [`crate::wallet::Wallet::transfer`],
/// [`crate::wallet::Wallet::execute`], and all staking operations.
#[derive(Debug, Clone)]
pub struct Tx<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    /// The transaction hash.
    pub hash: Felt,
    provider: Arc<P>,
}

impl<P> Tx<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    pub(crate) fn new(hash: Felt, provider: Arc<P>) -> Self {
        Self { hash, provider }
    }

    /// The transaction hash as a `0x`-prefixed hex string.
    pub fn hash_hex(&self) -> String {
        format!("{:#x}", self.hash)
    }

    /// Block until the transaction is accepted or reverted.
    ///
    /// Polls with exponential backoff starting at 2 s, capped at 30 s.
    /// Times out after `max_attempts` polls (default 30, roughly 5 minutes).
    pub async fn wait(&self) -> Result<TransactionReceiptWithBlockInfo> {
        self.wait_with_options(30, Duration::from_secs(2)).await
    }

    /// Like [`wait`] but with configurable retry count and initial poll interval.
    pub async fn wait_with_options(
        &self,
        max_attempts: u32,
        initial_interval: Duration,
    ) -> Result<TransactionReceiptWithBlockInfo> {
        let mut interval = initial_interval;
        let cap = Duration::from_secs(30);

        for attempt in 0..max_attempts {
            debug!(hash = %self.hash_hex(), attempt, "polling transaction receipt");

            match self.provider.get_transaction_receipt(self.hash).await {
                Ok(receipt) => {
                    return match execution_result(&receipt.receipt) {
                        ExecutionResult::Succeeded => Ok(receipt),
                        ExecutionResult::Reverted { reason } => {
                            Err(StarkzapError::TransactionReverted {
                                reason: reason.clone(),
                            })
                        }
                    };
                }
                Err(e) => {
                    // A ProviderError here typically means the tx is not yet
                    // visible (pending/unknown). Warn and retry.
                    warn!(hash = %self.hash_hex(), error = %e, "receipt fetch error, retrying");
                }
            }

            sleep(interval).await;
            interval = (interval * 2).min(cap);
        }

        Err(StarkzapError::WaitTimeout {
            attempts: max_attempts,
        })
    }

    /// Poll the current status without blocking.
    pub async fn status(&self) -> Result<TxStatus> {
        match self.provider.get_transaction_receipt(self.hash).await {
            Ok(receipt) => Ok(match execution_result(&receipt.receipt) {
                ExecutionResult::Succeeded => TxStatus::Accepted,
                ExecutionResult::Reverted { reason } => TxStatus::Reverted {
                    reason: reason.clone(),
                },
            }),
            // Receipt not yet available — treat as pending.
            Err(_) => Ok(TxStatus::Pending),
        }
    }

    /// Stream status updates at the given poll interval.
    ///
    /// The stream yields on every tick. Break when [`TxStatus::is_final`] returns `true`.
    pub fn watch(&self, interval: Duration) -> impl tokio_stream::Stream<Item = TxStatus> + '_ {
        IntervalStream::new(tokio::time::interval(interval))
            .then(move |_| async move { self.status().await.unwrap_or(TxStatus::Pending) })
    }
}

impl<P> std::fmt::Display for Tx<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tx({})", self.hash_hex())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract `ExecutionResult` from any `TransactionReceipt` variant.
fn execution_result(receipt: &TransactionReceipt) -> &ExecutionResult {
    match receipt {
        TransactionReceipt::Invoke(r)        => &r.execution_result,
        TransactionReceipt::L1Handler(r)     => &r.execution_result,
        TransactionReceipt::Declare(r)       => &r.execution_result,
        TransactionReceipt::Deploy(r)        => &r.execution_result,
        TransactionReceipt::DeployAccount(r) => &r.execution_result,
    }
}