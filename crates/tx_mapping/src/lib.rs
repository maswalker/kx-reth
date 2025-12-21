//! Kasplex transaction mapping module
//! 
//! This module manages the mapping between transaction hashes and their submission block numbers.
//! It is used to track which block number a transaction was submitted in, which is important
//! for Kasplex's anchor transaction mechanism.

use alloy_primitives::B256;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};
use tracing::{debug, trace};

/// Number of blocks to retain unexecuted transactions.
/// Transactions older than this will be considered expired.
pub const UNEXECUTED_TX_RETENTION_BLOCKS: u64 = 100;

/// Transaction mapping state
#[derive(Debug, Clone)]
struct TxMappingState {
    /// Mapping from transaction hash to submission block number
    mapping: HashMap<B256, u64>,
    /// Set of deleted transaction hashes
    deleted: std::collections::HashSet<B256>,
    /// Reorg state
    reorg_in_progress: bool,
}

impl Default for TxMappingState {
    fn default() -> Self {
        Self {
            mapping: HashMap::new(),
            deleted: std::collections::HashSet::new(),
            reorg_in_progress: false,
        }
    }
}

/// Transaction mapping manager
/// 
/// This manages the mapping between transaction hashes and their submission block numbers.
/// It supports:
/// - Setting mappings in bulk
/// - Getting and removing transaction numbers
/// - Tracking deleted transactions
/// - Managing reorg state
#[derive(Debug, Clone)]
pub struct TxMapping {
    state: Arc<RwLock<TxMappingState>>,
}

impl Default for TxMapping {
    fn default() -> Self {
        // Use the global shared instance
        GLOBAL_TX_MAPPING.clone()
    }
}

/// Global shared transaction mapping instance
/// 
/// This ensures that all components use the same TxMapping instance,
/// allowing proper sharing of transaction number data across:
/// - RPC handlers (when transactions are submitted)
/// - Payload builder (when building blocks)
/// - Block executor (when executing blocks)
static GLOBAL_TX_MAPPING: LazyLock<TxMapping> = LazyLock::new(|| {
    TxMapping {
        state: Arc::new(RwLock::new(TxMappingState::default())),
    }
});

/// Get the global shared transaction mapping instance
/// 
/// This is the preferred way to access the transaction mapping,
/// as it ensures all components use the same instance.
pub fn get_global_tx_mapping() -> &'static TxMapping {
    &GLOBAL_TX_MAPPING
}

impl TxMapping {
    /// Create a new transaction mapping instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Set transaction mappings in bulk
    /// 
    /// This replaces the entire mapping with the provided dictionary.
    pub fn set_mapping(&self, mapping: HashMap<B256, u64>) {
        let mut state = self.state.write().expect("tx mapping lock poisoned");
        state.mapping = mapping;
        trace!(target: "tx_mapping", count = state.mapping.len(), "Set transaction mapping");
    }

    /// Get the transaction number for a given hash and remove it from the mapping
    /// 
    /// Returns the block number if found, or 0 if not found.
    pub fn get_tx_number(&self, tx_hash: B256) -> u64 {
        let mut state = self.state.write().expect("tx mapping lock poisoned");
        state.mapping.remove(&tx_hash).unwrap_or(0)
    }

    /// Add a transaction to the deleted set
    pub fn add_deleted_transaction(&self, tx_hash: B256) {
        let mut state = self.state.write().expect("tx mapping lock poisoned");
        state.deleted.insert(tx_hash);
        trace!(target: "tx_mapping", ?tx_hash, "Marked transaction as deleted");
    }

    /// Check if a transaction is marked as deleted
    pub fn is_deleted_transaction(&self, tx_hash: B256) -> bool {
        let state = self.state.read().expect("tx mapping lock poisoned");
        state.deleted.contains(&tx_hash)
    }

    /// Mark that a pool reorg has started
    pub fn mark_start_pool_reorg(&self) {
        let mut state = self.state.write().expect("tx mapping lock poisoned");
        state.reorg_in_progress = true;
        debug!(target: "tx_mapping", "Marked pool reorg as started");
    }

    /// Mark that a pool reorg has completed
    pub fn mark_pool_reorged(&self) {
        let mut state = self.state.write().expect("tx mapping lock poisoned");
        state.reorg_in_progress = false;
        debug!(target: "tx_mapping", "Marked pool reorg as completed");
    }

    /// Check if a reorg is in progress
    pub fn is_reorg_complete(&self) -> bool {
        let state = self.state.read().expect("tx mapping lock poisoned");
        !state.reorg_in_progress
    }

    /// Insert a single transaction mapping
    pub fn insert(&self, tx_hash: B256, block_number: u64) {
        let mut state = self.state.write().expect("tx mapping lock poisoned");
        state.mapping.insert(tx_hash, block_number);
        trace!(target: "tx_mapping", ?tx_hash, block_number, "Inserted transaction mapping");
    }

    /// Check if a transaction is expired based on the current block number
    /// 
    /// A transaction is expired if:
    /// - It was submitted more than `UNEXECUTED_TX_RETENTION_BLOCKS` blocks ago
    /// - Or if it was submitted in a future block
    pub fn is_transaction_expired(&self, tx_hash: B256, current_block_number: u64) -> bool {
        let state = self.state.read().expect("tx mapping lock poisoned");
        
        if let Some(&submission_block) = state.mapping.get(&tx_hash) {
            // Transaction is expired if it was submitted more than retention blocks ago
            // or if it's in a future block
            submission_block + UNEXECUTED_TX_RETENTION_BLOCKS < current_block_number
                || submission_block > current_block_number
        } else {
            // If not in mapping, it's not expired (might be a new transaction)
            false
        }
    }

    /// Get the submission block number for a transaction without removing it
    pub fn get_submission_block(&self, tx_hash: B256) -> Option<u64> {
        let state = self.state.read().expect("tx mapping lock poisoned");
        state.mapping.get(&tx_hash).copied()
    }

    /// Clear all mappings (useful for testing or reset)
    pub fn clear(&self) {
        let mut state = self.state.write().expect("tx mapping lock poisoned");
        state.mapping.clear();
        state.deleted.clear();
        state.reorg_in_progress = false;
        trace!(target: "tx_mapping", "Cleared all transaction mappings");
    }

    /// Get the number of mappings
    pub fn len(&self) -> usize {
        let state = self.state.read().expect("tx mapping lock poisoned");
        state.mapping.len()
    }

    /// Check if the mapping is empty
    pub fn is_empty(&self) -> bool {
        let state = self.state.read().expect("tx mapping lock poisoned");
        state.mapping.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx_mapping_basic() {
        let mapping = TxMapping::new();
        let tx_hash = B256::from([1u8; 32]);
        let block_number = 100;

        // Insert and retrieve
        mapping.insert(tx_hash, block_number);
        assert_eq!(mapping.get_submission_block(tx_hash), Some(block_number));
        
        // Get and remove
        let retrieved = mapping.get_tx_number(tx_hash);
        assert_eq!(retrieved, block_number);
        assert_eq!(mapping.get_submission_block(tx_hash), None);
    }

    #[test]
    fn test_deleted_transactions() {
        let mapping = TxMapping::new();
        let tx_hash = B256::from([2u8; 32]);

        assert!(!mapping.is_deleted_transaction(tx_hash));
        mapping.add_deleted_transaction(tx_hash);
        assert!(mapping.is_deleted_transaction(tx_hash));
    }

    #[test]
    fn test_reorg_state() {
        let mapping = TxMapping::new();

        assert!(mapping.is_reorg_complete());
        mapping.mark_start_pool_reorg();
        assert!(!mapping.is_reorg_complete());
        mapping.mark_pool_reorged();
        assert!(mapping.is_reorg_complete());
    }

    #[test]
    fn test_transaction_expiration() {
        let mapping = TxMapping::new();
        let tx_hash = B256::from([3u8; 32]);
        let submission_block = 100;
        let current_block = 100 + UNEXECUTED_TX_RETENTION_BLOCKS;

        mapping.insert(tx_hash, submission_block);
        
        // Not expired yet
        assert!(!mapping.is_transaction_expired(tx_hash, submission_block + UNEXECUTED_TX_RETENTION_BLOCKS - 1));
        
        // Expired
        assert!(mapping.is_transaction_expired(tx_hash, current_block));
    }

    #[test]
    fn test_bulk_set_mapping() {
        let mapping = TxMapping::new();
        let mut bulk_mapping = HashMap::new();
        let tx1 = B256::from([1u8; 32]);
        let tx2 = B256::from([2u8; 32]);
        
        bulk_mapping.insert(tx1, 100);
        bulk_mapping.insert(tx2, 200);
        
        mapping.set_mapping(bulk_mapping);
        
        assert_eq!(mapping.get_submission_block(tx1), Some(100));
        assert_eq!(mapping.get_submission_block(tx2), Some(200));
    }
}

