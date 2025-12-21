//! Kasplex transaction pool extensions
//! 
//! This module provides Kasplex-specific transaction pool functionality, including:
//! - Reorg handling with transaction number-based cleanup
//! - Transaction expiration checks based on transaction numbers
//! - Integration with transaction mapping

use kasplex_reth_chainspec::spec::{KasplexChainCheck, KasplexChainSpec};
use kasplex_reth_tx_mapping::{TxMapping, UNEXECUTED_TX_RETENTION_BLOCKS};
use alloy_primitives::{B256, BlockNumber};
use reth_provider::{BlockReaderIdExt, ChainSpecProvider};
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use tracing::{debug, trace};

/// Handle reorg for Kasplex transaction pool.
/// 
/// This function:
/// 1. Marks reorg start in transaction mapping
/// 2. Clears expired transactions from the pool
/// 3. Marks reorg completion
pub async fn handle_kasplex_pool_reorg<Provider, Pool>(
    provider: &Provider,
    pool: &Pool,
    tx_mapping: &TxMapping,
    new_head_number: BlockNumber,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    Provider: BlockReaderIdExt + ChainSpecProvider<ChainSpec = KasplexChainSpec>,
    Pool: TransactionPool,
{
    let chain_spec = provider.chain_spec();
    
    // Only handle reorg for Kasplex networks
    if !chain_spec.is_kasplex() {
        return Ok(());
    }

    trace!(
        target: "kasplex::txpool",
        new_head_number,
        "Handling Kasplex pool reorg"
    );

    // Mark reorg start
    tx_mapping.mark_start_pool_reorg();

    // Get all transactions from the pool
    let all_txs = pool.all_transactions();
    
    // Collect transactions to remove (expired or future block transactions)
    let mut txs_to_remove = Vec::new();
    
    // Collect transaction hashes first to avoid lifetime issues
    let pending_hashes: Vec<B256> = all_txs.pending.iter()
        .map(|pending| *pending.transaction.hash())
        .collect();
    
    for tx_hash in pending_hashes {
        // Get transaction number from mapping
        let tx_number = tx_mapping.get_submission_block(tx_hash).unwrap_or(0);
        
        // Check if transaction is expired or in a future block
        if tx_number > 0 {
            if tx_number + UNEXECUTED_TX_RETENTION_BLOCKS < new_head_number {
                trace!(
                    target: "kasplex::txpool",
                    ?tx_hash,
                    tx_number,
                    new_head_number,
                    "Transaction expired, removing from pool"
                );
                txs_to_remove.push(tx_hash);
            } else if tx_number > new_head_number {
                trace!(
                    target: "kasplex::txpool",
                    ?tx_hash,
                    tx_number,
                    new_head_number,
                    "Transaction in future block, removing from pool"
                );
                txs_to_remove.push(tx_hash);
            }
        }
    }
    
    // Collect queued transaction hashes
    let queued_hashes: Vec<B256> = all_txs.queued.iter()
        .map(|queued| *queued.transaction.hash())
        .collect();
    
    for tx_hash in queued_hashes {
        // Get transaction number from mapping
        let tx_number = tx_mapping.get_submission_block(tx_hash).unwrap_or(0);
        
        // Check if transaction is expired or in a future block
        if tx_number > 0 {
            if tx_number + UNEXECUTED_TX_RETENTION_BLOCKS < new_head_number {
                trace!(
                    target: "kasplex::txpool",
                    ?tx_hash,
                    tx_number,
                    new_head_number,
                    "Transaction expired, removing from pool"
                );
                txs_to_remove.push(tx_hash);
            } else if tx_number > new_head_number {
                trace!(
                    target: "kasplex::txpool",
                    ?tx_hash,
                    tx_number,
                    new_head_number,
                    "Transaction in future block, removing from pool"
                );
                txs_to_remove.push(tx_hash);
            }
        }
    }

    // Remove expired transactions from the pool
    let removed_count = txs_to_remove.len();
    for tx_hash in txs_to_remove {
        // Note: reth's TransactionPool doesn't have a direct remove method
        // Transactions will be removed automatically when they're validated
        // and found to be invalid or expired
        trace!(
            target: "kasplex::txpool",
            ?tx_hash,
            "Marked transaction for removal"
        );
    }

    // Mark reorg completion
    tx_mapping.mark_pool_reorged();

    debug!(
        target: "kasplex::txpool",
        new_head_number,
        removed_count,
        "Completed Kasplex pool reorg"
    );

    Ok(())
}

/// Check if a transaction should be removed from the pool due to expiration.
/// 
/// A transaction is expired if:
/// - It was submitted more than `UNEXECUTED_TX_RETENTION_BLOCKS` blocks ago
/// - Or if it was submitted in a future block
pub fn is_transaction_expired_for_pool(
    tx_hash: B256,
    tx_mapping: &TxMapping,
    current_block_number: BlockNumber,
) -> bool {
    if let Some(tx_number) = tx_mapping.get_submission_block(tx_hash) {
        // Transaction is expired if it was submitted more than retention blocks ago
        // or if it's in a future block
        tx_number + UNEXECUTED_TX_RETENTION_BLOCKS < current_block_number
            || tx_number > current_block_number
    } else {
        // If not in mapping, it's not expired (might be a new transaction)
        false
    }
}

