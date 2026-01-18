use alloy_primitives::B256;
use alloy_rpc_types_engine::{ExecutionPayload, ExecutionPayloadV1};
use alloy_rpc_types_eth::Withdrawal;
use reth_payload_primitives::ExecutionPayload as ExecutionPayloadTr;
use serde::{Deserialize, Serialize};

/// Kasplex execution data containing the execution payload and Kasplex-specific sidecar data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KasplexExecutionData {
    /// The standard execution payload envelope.
    pub execution_payload: ExecutionPayloadV1,
    /// Kasplex-specific sidecar data.
    pub kasplex_sidecar: KasplexExecutionDataSidecar,
}

impl KasplexExecutionData {
    /// Creates a new instance of `ExecutionPayload`.
    pub fn into_payload(self) -> ExecutionPayload {
        ExecutionPayload::V1(self.execution_payload)
    }
}

impl From<KasplexExecutionData> for ExecutionPayload {
    fn from(input: KasplexExecutionData) -> Self {
        input.into_payload()
    }
}

/// Kasplex-specific sidecar data attached to execution payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KasplexExecutionDataSidecar {
    /// Transaction hash (txHash) instead of full transaction list.
    pub tx_hash: B256,
    /// Withdrawals hash.
    pub withdrawals_hash: B256,
    /// Transaction submission block numbers array.
    /// Maps each transaction to its submission block number.
    pub numbers: Vec<u64>,
    /// Flag indicating this is a Kasplex block.
    pub kasplex_block: bool,
}

impl ExecutionPayloadTr for KasplexExecutionData {
    /// Returns the parent hash of the block.
    fn parent_hash(&self) -> B256 {
        self.execution_payload.parent_hash
    }

    /// Returns the hash of the block.
    fn block_hash(&self) -> B256 {
        self.execution_payload.block_hash
    }

    /// Returns the block number.
    fn block_number(&self) -> u64 {
        self.execution_payload.block_number
    }

    /// Returns the withdrawals associated with the block, if any.
    fn withdrawals(&self) -> Option<&Vec<Withdrawal>> {
        None
    }

    /// Returns the access list included in this payload.
    ///
    /// Returns `None` for pre-Amsterdam blocks.
    fn block_access_list(&self) -> Option<&alloy_primitives::Bytes> {
        None
    }

    /// Returns the parent beacon block root, if applicable.
    fn parent_beacon_block_root(&self) -> Option<B256> {
        None
    }

    /// Returns the timestamp of the block.
    fn timestamp(&self) -> u64 {
        self.execution_payload.timestamp
    }

    /// Returns the gas used in the block.
    fn gas_used(&self) -> u64 {
        self.execution_payload.gas_used
    }

    /// Returns the number of transactions in the block.
    fn transaction_count(&self) -> usize {
        self.execution_payload.transactions.len()
    }
}


