use alloy_consensus::Transaction;
use reth::{
    api::PayloadBuilderAttributes,
    providers::{ChainSpecProvider, StateProviderFactory},
    revm::{State, database::StateProviderDatabase, primitives::U256},
};
use reth_basic_payload_builder::{
    BuildArguments, BuildOutcome, MissingPayloadBehaviour, PayloadBuilder, PayloadConfig,
};
use reth_payload_primitives::PayloadBuilderError;
use reth_ethereum::EthPrimitives;
use reth_ethereum_engine_primitives::EthBuiltPayload;
use reth_evm::{
    ConfigureEvm,
    block::{BlockExecutionError, BlockValidationError},
    execute::{BlockBuilder, BlockBuilderOutcome},
};
use std::{convert::Infallible, sync::Arc};
use tracing::{debug, trace, warn};

use kasplex_reth_block::{KasplexEvmConfig, ExtraDataConfig, BaseFeeConfig};
use kasplex_reth_chainspec::spec::KasplexChainSpec;
use kasplex_reth_primitives::payload::builder::KasplexPayloadBuilderAttributes;

/// Kasplex payload builder
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KasplexPayloadBuilder<Client, EvmConfig = KasplexEvmConfig> {
    /// Client providing access to node state.
    client: Client,
    /// The type responsible for creating the evm.
    evm_config: EvmConfig,
}

impl<Client, EvmConfig> KasplexPayloadBuilder<Client, EvmConfig> {
    /// `KasplexPayloadBuilder` constructor.
    pub const fn new(client: Client, evm_config: EvmConfig) -> Self {
        Self { client, evm_config }
    }
}

impl<Client, EvmConfig> PayloadBuilder for KasplexPayloadBuilder<Client, EvmConfig>
where
    EvmConfig: ConfigureEvm<
            Primitives = EthPrimitives,
            Error = Infallible,
            NextBlockEnvCtx = reth_evm::NextBlockEnvAttributes,
        > + ExtraDataConfig + BaseFeeConfig,
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec = KasplexChainSpec> + Clone,
{
    /// The payload attributes type to accept for building.
    type Attributes = KasplexPayloadBuilderAttributes;
    /// The type of the built payload.
    type BuiltPayload = EthBuiltPayload;

    /// Tries to build a transaction payload using provided arguments.
    fn try_build(
        &self,
        args: BuildArguments<KasplexPayloadBuilderAttributes, EthBuiltPayload>,
    ) -> Result<BuildOutcome<EthBuiltPayload>, PayloadBuilderError> {
        kasplex_payload(self.evm_config.clone(), self.client.clone(), args)
    }

    /// Invoked when the payload job is being resolved and there is no payload yet.
    fn on_missing_payload(
        &self,
        _args: BuildArguments<Self::Attributes, Self::BuiltPayload>,
    ) -> MissingPayloadBehaviour<Self::BuiltPayload> {
        MissingPayloadBehaviour::AwaitInProgress
    }

    /// Builds an empty payload without any transaction.
    fn build_empty_payload(
        &self,
        _config: PayloadConfig<Self::Attributes>,
    ) -> Result<EthBuiltPayload, PayloadBuilderError> {
        Err(PayloadBuilderError::MissingPayload)
    }
}

// Build a Kasplex network payload using the given attributes.
#[inline]
fn kasplex_payload<EvmConfig, Client>(
    evm_config: EvmConfig,
    client: Client,
    args: BuildArguments<KasplexPayloadBuilderAttributes, EthBuiltPayload>,
) -> Result<BuildOutcome<EthBuiltPayload>, PayloadBuilderError>
where
    EvmConfig: ConfigureEvm<
            Primitives = EthPrimitives,
            Error = Infallible,
            NextBlockEnvCtx = reth_evm::NextBlockEnvAttributes,
        > + ExtraDataConfig + BaseFeeConfig,
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec = KasplexChainSpec>,
{
    let BuildArguments { mut cached_reads, config, cancel, best_payload: _ } = args;
    let PayloadConfig { parent_header, attributes } = config;

    let state_provider = client.state_by_block_hash(parent_header.hash())?;
    let state = StateProviderDatabase::new(&state_provider);
    let mut db =
        State::builder().with_database(cached_reads.as_db_mut(state)).with_bundle_update().build();

    debug!(target: "payload_builder", id=%attributes.payload_id(), parent_header = ?parent_header.hash(), parent_number = parent_header.number, attributes = ?attributes, "building Kasplex payload for block");

    // Get base fee and gas limit from attributes
    let base_fee = attributes.base_fee_per_gas.to::<u64>();
    let gas_limit = attributes.gas_limit;
    
    // Set extra_data and base_fee_per_gas for the next block before calling builder_for_next_block
    // This ensures context_for_next_block and next_evm_env can use the correct values from BlockMetadata
    evm_config.set_next_block_extra_data(attributes.extra_data.clone());
    evm_config.set_next_block_base_fee_per_gas(base_fee);
    
    let mut builder = evm_config
        .builder_for_next_block(
            &mut db,
            &parent_header,
            reth_evm::NextBlockEnvAttributes {
                timestamp: attributes.timestamp(),
                suggested_fee_recipient: attributes.suggested_fee_recipient(),
                prev_randao: attributes.prev_randao(),
                gas_limit,
                parent_beacon_block_root: attributes.parent_beacon_block_root(),
                withdrawals: attributes.withdrawals().clone().into(),
            },
        )
        .map_err(PayloadBuilderError::other)?;
    
    // Clear extra_data and base_fee_per_gas after builder_for_next_block is called
    evm_config.clear_next_block_extra_data();
    evm_config.clear_next_block_base_fee_per_gas();

    debug!(target: "payload_builder", id=%attributes.payload_id(), parent_header = ?parent_header.hash(), parent_number = parent_header.number, "building new Kasplex payload");
    let mut total_fees = U256::ZERO;
    
    // Collect transaction numbers for Numbers array
    let mut transaction_numbers = Vec::new();

    builder.apply_pre_execution_changes().map_err(|err| {
        warn!(target: "payload_builder", %err, "failed to apply pre-execution changes");
        PayloadBuilderError::Internal(err.into())
    })?;

    // Use transactions from attributes (decoded from tx_list in KasplexPayloadAttributes)
    for tx in &attributes.transactions {
        // check if the job was cancelled, if so we can exit early
        if cancel.is_cancelled() {
            return Ok(BuildOutcome::Cancelled);
        }

        // Extract transaction number for Numbers array
        // Note: Transaction number is stored in TransactionSigned.number field
        // We need to extract it from the transaction before execution
        let tx_number = extract_transaction_number(tx);
        transaction_numbers.push(tx_number);

        let gas_used = match builder.execute_transaction(tx.clone()) {
            Ok(gas_used) => gas_used,
            Err(BlockExecutionError::Validation(
                BlockValidationError::InvalidTx { .. } |
                BlockValidationError::TransactionGasLimitMoreThanAvailableBlockGas { .. },
            )) => {
                trace!(target: "payload_builder", ?tx, "skipping invalid transaction");
                // Remove the number we just added since transaction is invalid
                transaction_numbers.pop();
                continue;
            }
            // this is an error that we should treat as fatal for this attempt
            Err(err) => return Err(PayloadBuilderError::evm(err)),
        };

        // update add to total fees
        let miner_fee =
            tx.effective_tip_per_gas(base_fee).expect("fee is always valid; execution succeeded");
        total_fees += U256::from(miner_fee) * U256::from(gas_used);
    }

    let BlockBuilderOutcome { block, .. } = builder.finish(&state_provider)?;

    // For Kasplex, we collect transaction numbers for the Numbers array
    // Store them in TxMapping so they can be retrieved in block_to_payload
    // The block from BlockBuilderOutcome should already be in the correct format
    // Store transaction numbers before converting to sealed block
    if !transaction_numbers.is_empty() {
        trace!(target: "payload_builder", numbers_count = transaction_numbers.len(), "Collected transaction numbers for Kasplex block");
        
        // Store transaction numbers in TxMapping for later retrieval in block_to_payload
        // This ensures that when the block is converted to ExecutableData, the numbers
        // can be extracted from the mapping
        // Use the global shared instance to ensure consistency
        let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();
        // Get transactions from the block before sealing
        // RecoveredBlock has recovered_transactions() method that returns Recovered transactions
        // We need to get the transaction hash from the recovered transaction
        for (idx, number) in transaction_numbers.iter().enumerate() {
            if let Some(recovered_tx) = block.recovered_transaction(idx) {
                let tx_hash = *recovered_tx.hash();
                tx_mapping.insert(tx_hash, *number);
                trace!(
                    target: "payload_builder",
                    ?tx_hash,
                    number,
                    "Stored transaction number in mapping for Kasplex block"
                );
            }
        }
        
        // The numbers will be stored in the sidecar when the payload is converted to ExecutableData
        // See kasplex_reth_primitives::engine::types::KasplexExecutionDataSidecar
        // The block_to_payload function will extract them from TxMapping
    }

    // Convert RecoveredBlock to SealedBlock for EthBuiltPayload
    let sealed_block = Arc::new(block.sealed_block().clone());
    let payload = EthBuiltPayload::new(attributes.payload_id(), sealed_block, total_fees, None);

    Ok(BuildOutcome::Freeze(payload))
}

/// Extract transaction number from a transaction.
/// 
/// For Kasplex, transactions have a `number` field that indicates the block number
/// where the transaction was submitted. This function gets the number from the transaction mapping.
fn extract_transaction_number(tx: &reth::primitives::Recovered<reth::primitives::TransactionSigned>) -> u64 {
    // Get transaction hash first
    let tx_hash = tx.hash();
    
    // Use the global shared TxMapping instance
    let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();
    
    // Try to get from mapping (without removing it, as we might need it later)
    tx_mapping.get_submission_block(*tx_hash).unwrap_or(0)
}

