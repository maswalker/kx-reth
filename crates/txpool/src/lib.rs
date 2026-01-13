//! Kasplex transaction pool extensions
//! 
//! This module provides Kasplex-specific transaction pool functionality, including:
//! - Reorg handling with transaction number-based cleanup
//! - Transaction expiration checks based on transaction numbers
//! - Integration with transaction mapping

use kasplex_reth_chainspec::spec::KasplexChainCheck;
use kasplex_reth_tx_mapping::{TxMapping, UNEXECUTED_TX_RETENTION_BLOCKS};
use alloy_primitives::{B256, BlockNumber};
use alloy_consensus::BlockHeader;
use reth_provider::{BlockReaderIdExt, ChainSpecProvider};
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use reth_primitives::SealedBlock;
use reth_primitives_traits::{BlockBody, transaction::TxHashRef};
use tracing::{debug, trace};

/// Handle reorg for Kasplex transaction pool.
/// 
/// This function:
/// 1. Marks reorg start in transaction mapping
/// 2. Clears expired transactions from the pool
/// 3. Marks reorg completion
pub async fn handle_kasplex_pool_reorg<Provider, Pool, ChainSpec>(
    provider: &Provider,
    pool: &Pool,
    tx_mapping: &TxMapping,
    new_head_number: BlockNumber,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    Provider: BlockReaderIdExt + ChainSpecProvider<ChainSpec = ChainSpec>,
    ChainSpec: KasplexChainCheck,
    Pool: TransactionPool,
{
    let chain_spec = provider.chain_spec();
    
    // Only handle reorg for Kasplex networks
    if !chain_spec.is_kasplex() {
        return Ok(());
    }

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

    // Actually remove the expired transactions from the pool
    let removed_count = txs_to_remove.len();
    if removed_count > 0 {
        let removed_txs = pool.remove_transactions(txs_to_remove);
        let actually_removed = removed_txs.len();
        if actually_removed > 0 {
            debug!(
                target: "kasplex::txpool",
                removed_count,
                actually_removed,
                "Removed expired transactions from pool"
            );
        } else {
            trace!(
                target: "kasplex::txpool",
                removed_count,
                "No expired transactions found in pool (may have been already removed)"
            );
        }
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

/// Remove transactions that were included in a block from the pool.
/// 
/// This function should be called after a block is finalized to remove
/// all transactions that were included in that block from the mempool.
pub async fn remove_included_transactions<Pool, Block>(
    pool: &Pool,
    block: &SealedBlock<Block>,
    _tx_mapping: &TxMapping,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>>
where
    Pool: TransactionPool,
    Block: reth_primitives_traits::Block,
    <Block as reth_primitives_traits::Block>::Body: BlockBody,
    <<Block as reth_primitives_traits::Block>::Body as BlockBody>::Transaction: TxHashRef,
{
    // Get all transaction hashes from the block
    // SealedBlock has body() method that returns a reference to BlockBody
    // BlockBody has transactions() method that returns &[Transaction]
    // Transaction implements TxHashRef trait which has tx_hash() method
    let block_tx_hashes: Vec<B256> = block.body().transactions()
        .iter()
        .map(|tx| *tx.tx_hash())
        .collect();

    // Get all transactions from the pool
    let all_txs = pool.all_transactions();

    // Check if any pool transactions match the block transactions
    let mut txs_to_remove = Vec::new();

    // Check pending transactions
    for pending in all_txs.pending.iter() {
        let tx_hash = *pending.transaction.hash();
        if block_tx_hashes.contains(&tx_hash) {
            txs_to_remove.push(tx_hash);
        }
    }

    // Check queued transactions
    for queued in all_txs.queued.iter() {
        let tx_hash = *queued.transaction.hash();
        if block_tx_hashes.contains(&tx_hash) {
            txs_to_remove.push(tx_hash);
        }
    }

    // Actually remove the included transactions from the pool
    // Note: reth's TransactionPool also automatically removes transactions that are
    // included in blocks when on_canonical_state_change is called, but we explicitly
    // remove them here to ensure they are removed immediately after block finalization.
    let removed_count = txs_to_remove.len();

    if removed_count > 0 {
        let removed_txs = pool.remove_transactions(txs_to_remove.clone());
        let actually_removed = removed_txs.len();
        if actually_removed > 0 {
            debug!(
                target: "kasplex::txpool",
                block_number = block.header().number(),
                removed_count,
                actually_removed,
                "Removed transactions included in block from pool"
            );
            for tx_hash in &txs_to_remove[..actually_removed.min(txs_to_remove.len())] {
                trace!(
                    target: "kasplex::txpool",
                    ?tx_hash,
                    "Transaction removed from pool (was included in block)"
                );
            }
        } else {
            trace!(
                target: "kasplex::txpool",
                block_number = block.header().number(),
                removed_count,
                "No included transactions found in pool (may have been already removed by reth)"
            );
        }
    }

    Ok(removed_count)
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

