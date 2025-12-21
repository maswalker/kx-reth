use alloy_rpc_types_engine::{
    ExecutionData, ExecutionPayloadEnvelopeV2, ExecutionPayloadEnvelopeV3, ExecutionPayloadEnvelopeV4,
    ExecutionPayloadEnvelopeV5, ExecutionPayloadV1, ExecutionPayloadSidecar,
};
use reth::primitives::SealedBlock;
use reth_ethereum_engine_primitives::EthBuiltPayload;
use reth_node_api::{BuiltPayload, EngineTypes, NodePrimitives, PayloadTypes};

// KasplexExecutionData and KasplexExecutionDataSidecar are used in types module
use crate::payload::{attributes::KasplexPayloadAttributes, builder::KasplexPayloadBuilderAttributes};

pub mod types;

/// The types used in the Kasplex consensus engine.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
#[non_exhaustive]
pub struct KasplexEngineTypes;

impl PayloadTypes for KasplexEngineTypes {
    /// The execution payload type provided as input.
    /// Note: We use ExecutionData here to satisfy EngineApiServer constraints.
    /// Kasplex-specific sidecar data is handled separately in the validator.
    type ExecutionData = ExecutionData;
    /// The built payload type.
    type BuiltPayload = EthBuiltPayload;
    /// The RPC payload attributes type the CL node emits via the engine API.
    type PayloadAttributes = KasplexPayloadAttributes;
    /// The payload attributes type that contains information about a running payload job.
    type PayloadBuilderAttributes = KasplexPayloadBuilderAttributes;

    /// Converts a block into an execution payload.
    fn block_to_payload(
        block: SealedBlock<
            <<Self::BuiltPayload as BuiltPayload>::Primitives as NodePrimitives>::Block,
        >,
    ) -> Self::ExecutionData {
        let _tx_hash = block.transactions_root;
        let _withdrawals_hash = block.withdrawals_root;

        // Extract transaction numbers from transaction mapping
        // In Kasplex, transaction numbers are stored in the transaction mapping
        // when transactions are submitted via RPC. We collect them here for the sidecar.
        // 
        // Note: Since reth's TransactionSigned doesn't have a number field in the official version,
        // we rely on the transaction mapping to store and retrieve transaction numbers.
        // Use the global shared instance to ensure consistency
        let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();
        let block_hash = block.hash();
        
        // Convert block to unsealed block first to access transactions
        let unsealed = block.into_block();
        
        // Extract transaction numbers from unsealed block body
        let numbers: Vec<u64> = unsealed.body.transactions.iter()
            .map(|tx| {
                let tx_hash = tx.hash();
                tx_mapping.get_submission_block(*tx_hash).unwrap_or(0)
            })
            .collect();

        // Convert block to execution payload using the same method as alethia-reth
        let payload = ExecutionPayloadV1::from_block_unchecked(block_hash, &unsealed);

        // Store Kasplex-specific sidecar data in TxMapping for later retrieval
        // The sidecar data will be handled by the validator
        let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();
        for (tx, number) in unsealed.body.transactions.iter().zip(numbers.iter()) {
            let tx_hash = tx.hash();
            tx_mapping.insert(*tx_hash, *number);
        }

        // Return standard ExecutionData to satisfy EngineApiServer constraints
        ExecutionData {
            payload: payload.into(),
            sidecar: ExecutionPayloadSidecar::none(),
        }
    }
}

impl EngineTypes for KasplexEngineTypes {
    /// Execution Payload V1 envelope type.
    type ExecutionPayloadEnvelopeV1 = ExecutionPayloadV1;
    /// Execution Payload V2 envelope type.
    type ExecutionPayloadEnvelopeV2 = ExecutionPayloadEnvelopeV2;
    /// Execution Payload V3 envelope type.
    type ExecutionPayloadEnvelopeV3 = ExecutionPayloadEnvelopeV3;
    /// Execution Payload V4 envelope type.
    type ExecutionPayloadEnvelopeV4 = ExecutionPayloadEnvelopeV4;
    /// Execution Payload V5 envelope type.
    type ExecutionPayloadEnvelopeV5 = ExecutionPayloadEnvelopeV5;
}


