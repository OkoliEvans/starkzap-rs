//! Transaction handle returned by all mutating wallet operations.
//!
//! [`Tx`] wraps a transaction hash and a shared provider reference, providing
//! ergonomic methods to wait for confirmation or stream status updates.

use std::{sync::Arc, time::Duration};

use starknet::{
    core::types::{
        Felt, ExecutionResult, MaybePendingTransactionReceipt, PendingTransactionReceipt,
        TransactionReceipt,
    },
    providers::Provider,
};
use tokio::time::sleep;
use tokio_stream::{wrappers::IntervalStream, StreamExt};
use tracing::{debug, warn};

use crate::error::{Result, StarkzapError};

/// The confirmed status of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxStatus {
    /// Transaction is in the mempool / not yet in a block.
    Pending,
    /// Included in a block and execution succeeded.
    Accepted,
    /// Included in a block but execution was reverted.
    Reverted { reason: String },
    /// Transaction was rejected (never made it on-chain).
    Rejected { reason: String },
}

impl TxStatus {
    /// Returns `true` if the transaction is in a terminal state.
    pub fn is_final(&self) -> bool {
        !matches!(self, TxStatus::Pending)
    }
}

/// A submitted transaction handle.
///
/// Returned by [`crate::wallet::Wallet::transfer`],
/// [`crate::wallet::Wallet::execute`], and all staking operations.
#[derive(Debug, Clone)]
pub struct Tx {
    /// The transaction hash.
    pub hash: Felt,
    provider: Arc<dyn Provider + Send + Sync>,
}

impl Tx {
    pub(crate) fn new(hash: Felt, provider: Arc<dyn Provider + Send + Sync>) -> Self {
        Self { hash, provider }
    }

    /// The transaction hash as a `0x`-prefixed hex string.
    pub fn hash_hex(&self) -> String {
        format!("{:#x}", self.hash)
    }

    /// Block until the transaction is accepted or reverted.
    ///
    /// Polls with exponential backoff (2s → 4s → 8s … capped at 30s).
    /// Times out after `max_attempts` (default 30, ~5 minutes).
    ///
    /// # Errors
    ///
    /// - [`StarkzapError::TransactionReverted`] if the tx was reverted
    /// - [`StarkzapError::TransactionRejected`] if the tx was rejected
    /// - [`StarkzapError::WaitTimeout`] if the limit is reached
    pub async fn wait(&self) -> Result<TransactionReceipt> {
        self.wait_with_options(30, Duration::from_secs(2)).await
    }

    /// Like [`wait`] but with configurable `max_attempts` and initial `poll_interval`.
    pub async fn wait_with_options(
        &self,
        max_attempts: u32,
        initial_interval: Duration,
    ) -> Result<TransactionReceipt> {
        let mut interval = initial_interval;
        let cap = Duration::from_secs(30);

        for attempt in 0..max_attempts {
            debug!(
                hash = %self.hash_hex(),
                attempt,
                "polling transaction"
            );

            match self.provider.get_transaction_receipt(self.hash).await {
                Ok(MaybePendingTransactionReceipt::Receipt(receipt)) => {
                    return match receipt.execution_result() {
                        ExecutionResult::Succeeded => Ok(receipt),
                        ExecutionResult::Reverted { reason } => {
                            Err(StarkzapError::TransactionReverted { reason: reason.clone() })
                        }
                    };
                }
                Ok(MaybePendingTransactionReceipt::PendingReceipt(_)) => {
                    // Still pending — keep polling
                    debug!(hash = %self.hash_hex(), "transaction pending");
                }
                Err(e) => {
                    // Provider errors during polling (e.g. tx not found yet) are retried
                    warn!(hash = %self.hash_hex(), error = %e, "receipt fetch error, retrying");
                }
            }

            sleep(interval).await;
            interval = (interval * 2).min(cap);
        }

        Err(StarkzapError::WaitTimeout { attempts: max_attempts })
    }

    /// Poll the current status without blocking.
    pub async fn status(&self) -> Result<TxStatus> {
        match self.provider.get_transaction_receipt(self.hash).await {
            Ok(MaybePendingTransactionReceipt::PendingReceipt(_)) => Ok(TxStatus::Pending),
            Ok(MaybePendingTransactionReceipt::Receipt(receipt)) => {
                match receipt.execution_result() {
                    ExecutionResult::Succeeded => Ok(TxStatus::Accepted),
                    ExecutionResult::Reverted { reason } => Ok(TxStatus::Reverted {
                        reason: reason.clone(),
                    }),
                }
            }
            Err(_) => Ok(TxStatus::Pending), // Not yet visible on-chain
        }
    }

    /// Stream status updates at the given poll interval.
    ///
    /// The stream ends when the transaction reaches a terminal state.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use tokio_stream::StreamExt;
    /// use starkzap_rs::tx::TxStatus;
    ///
    /// let mut stream = tx.watch(std::time::Duration::from_secs(3));
    /// while let Some(status) = stream.next().await {
    ///     println!("Status: {:?}", status);
    ///     if status.is_final() { break; }
    /// }
    /// ```
    pub fn watch(
        &self,
        interval: Duration,
    ) -> impl tokio_stream::Stream<Item = TxStatus> + '_ {
        let interval_stream = IntervalStream::new(tokio::time::interval(interval));
        interval_stream.then(move |_| async move {
            self.status().await.unwrap_or(TxStatus::Pending)
        })
    }
}

impl std::fmt::Display for Tx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tx({})", self.hash_hex())
    }
}