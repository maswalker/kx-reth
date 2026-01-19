//! Kasplex engine payload validator
//! 
//! This module provides Kasplex-specific payload validation logic, including:
//! - Handling Numbers array from ExecutableData
//! - Restoring transaction Number fields from Numbers array
//! - Allowing block.timestamp == parent.timestamp for Kasplex
//! - Supporting Kasplex-specific ForkChoiceUpdate and NewPayload logic

use kasplex_reth_block::config::KasplexEvmConfig;
use kasplex_reth_chainspec::spec::{KasplexChainSpec, KasplexChainCheck};
use kasplex_reth_primitives::{
    engine::{KasplexEngineTypes, types::KasplexExecutionData},
    payload::attributes::KasplexPayloadAttributes,
};
use alloy_consensus::constants::EMPTY_WITHDRAWALS;
use alloy_rpc_types_engine::PayloadError;
use reth::primitives::RecoveredBlock;
use reth_engine_primitives::EngineApiValidator;
use reth_engine_tree::tree::{TreeConfig, payload_validator::BasicEngineValidator};
use reth_trie_db::ChangesetCache;
use reth_ethereum::{Block, EthPrimitives};
use reth_evm::ConfigureEngineEvm;
use reth_node_api::{
    AddOnsContext, FullNodeComponents, NewPayloadError, NodeTypes, PayloadTypes, PayloadValidator,
};
use reth_node_builder::{
    invalid_block_hook::InvalidBlockHookExt,
    rpc::{EngineValidatorBuilder, PayloadValidatorBuilder},
};
use reth_payload_primitives::{
    EngineApiMessageVersion, EngineObjectValidationError, InvalidPayloadAttributesError,
    PayloadAttributes, PayloadOrAttributes,
};
use reth_primitives_traits::Block as BlockTrait;
use std::sync::Arc;
use tracing::{debug, trace, warn};

/// Builder for [`KasplexEngineValidator`].
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct KasplexEngineValidatorBuilder;

impl<N> PayloadValidatorBuilder<N> for KasplexEngineValidatorBuilder
where
    N: FullNodeComponents<Evm = KasplexEvmConfig>,
    N::Types: NodeTypes<
            Primitives = EthPrimitives,
            ChainSpec = KasplexChainSpec,
            Payload = KasplexEngineTypes,
        >,
{
    /// The consensus implementation to build.
    type Validator = KasplexEngineValidator;

    /// Creates the engine validator.
    async fn build(self, ctx: &AddOnsContext<'_, N>) -> eyre::Result<Self::Validator> {
        Ok(KasplexEngineValidator::new(ctx.config.chain.clone()))
    }
}

impl<N> EngineValidatorBuilder<N> for KasplexEngineValidatorBuilder
where
    N: FullNodeComponents<Evm = KasplexEvmConfig>,
    N::Types: NodeTypes<
            Primitives = EthPrimitives,
            ChainSpec = KasplexChainSpec,
            Payload = KasplexEngineTypes,
        >,
    N::Evm: ConfigureEngineEvm<kasplex_reth_primitives::engine::types::KasplexExecutionData>,
{
    /// The tree validator type that will be used by the consensus engine.
    type EngineValidator = BasicEngineValidator<N::Provider, N::Evm, KasplexEngineValidator>;

    /// Builds the tree validator for the consensus engine.
    async fn build_tree_validator(
        self,
        ctx: &AddOnsContext<'_, N>,
        tree_config: TreeConfig,
        changeset_cache: ChangesetCache,
    ) -> eyre::Result<Self::EngineValidator> {
        let validator = <Self as PayloadValidatorBuilder<N>>::build(self, ctx).await?;
        let data_dir = ctx.config.datadir.clone().resolve_datadir(ctx.config.chain.inner.chain);
        let invalid_block_hook = ctx.create_invalid_block_hook(&data_dir).await?;
 
        // CRITICAL: Enable reorg support for Kasplex
        // This allows forkchoice updates to reorg the chain to an older block,
        // which is necessary when using --start-block to rebuild from a specific point
        let tree_config = tree_config
            .with_always_process_payload_attributes_on_canonical_head(true)
            .with_unwind_canonical_header(true);

        Ok(BasicEngineValidator::new(
            ctx.node.provider().clone(),
            Arc::new(ctx.node.consensus().clone()),
            ctx.node.evm_config().clone(),
            validator,
            tree_config,
            invalid_block_hook,
            changeset_cache,
        ))
    }
}

/// Validator for the Kasplex engine API.
#[derive(Debug, Clone)]
pub struct KasplexEngineValidator {
    pub chain_spec: Arc<KasplexChainSpec>,
}

impl KasplexEngineValidator {
    /// Instantiates a new validator.
    pub const fn new(chain_spec: Arc<KasplexChainSpec>) -> Self {
        Self { chain_spec }
    }
}

impl<Types> PayloadValidator<Types> for KasplexEngineValidator
where
    Types: PayloadTypes<ExecutionData = alloy_rpc_types_engine::ExecutionData>,
{
    /// The block type used by the engine.
    type Block = Block;

    /// Ensures that the given payload does not violate any consensus rules that concern the block's
    /// layout.
    ///
    /// This function must convert the payload into the executable block and pre-validate its
    /// fields.
    fn ensure_well_formed_payload(
        &self,
        payload: Types::ExecutionData,
    ) -> Result<RecoveredBlock<Self::Block>, NewPayloadError> {
        let alloy_rpc_types_engine::ExecutionData { payload, sidecar: _ } = payload;

        // Extract expected hash and debug info from payload first, before moving payload
        // Only V1 has block_hash field
        let (expected_hash, payload_for_debug) = match &payload {
            alloy_rpc_types_engine::ExecutionPayload::V1(p) => {
                // Extract debug info before moving
                let debug_info = (
                    p.block_number,
                    p.parent_hash,
                    p.fee_recipient,
                    p.state_root,
                    p.receipts_root,
                    p.logs_bloom,
                    p.prev_randao,
                    p.gas_limit,
                    p.gas_used,
                    p.timestamp,
                    p.extra_data.clone(),
                    p.base_fee_per_gas,
                    p.block_hash,
                    p.transactions.len(),
                );
                (p.block_hash, Some(debug_info))
            }
            _ => {
                // For V2/V3, we don't have block_hash in the payload
                // We'll compute it from the block after processing
                (alloy_primitives::B256::ZERO, None) // Placeholder, will be replaced
            }
        };

        // Debug: Log payload fields before conversion
        if let Some((block_number, parent_hash, fee_recipient, state_root, receipts_root, logs_bloom, prev_randao, gas_limit, gas_used, timestamp, ref extra_data, base_fee_per_gas, block_hash, tx_count)) = payload_for_debug {
            debug!(
                target: "rpc::engine::kasplex",
                block_number,
                ?parent_hash,
                ?fee_recipient,
                ?state_root,
                ?receipts_root,
                ?logs_bloom,
                ?prev_randao,
                gas_limit,
                gas_used,
                timestamp,
                ?extra_data,
                ?base_fee_per_gas,
                ?block_hash,
                transactions_count = tx_count,
                "Payload fields before conversion"
            );
        }

        // Process Kasplex execution data - transaction numbers are stored in TxMapping
        // when transactions are submitted via RPC
        let block = process_kasplex_execution_data(payload, None)
            .map_err(|e| NewPayloadError::Other(e.into()))?;

        // Handle Kasplex-specific sidecar data if available
        // Note: In the current implementation, we use ExecutionData which doesn't include
        // KasplexExecutionData directly. The sidecar handling is done separately.
        // If we need to handle tx_hash or withdrawals_hash from sidecar, we would do it here.
        
        let sealed_block = block.seal();
        let block_hash = sealed_block.hash();

        // Debug: Log block fields after conversion
        debug!(
            target: "rpc::engine::kasplex",
            block_number = sealed_block.number,
            parent_hash = ?sealed_block.parent_hash,
            beneficiary = ?sealed_block.beneficiary,
            state_root = ?sealed_block.state_root,
            receipts_root = ?sealed_block.receipts_root,
            logs_bloom = ?sealed_block.logs_bloom,
            prev_randao = ?sealed_block.mix_hash,
            gas_limit = sealed_block.gas_limit,
            gas_used = sealed_block.gas_used,
            timestamp = sealed_block.timestamp,
            extra_data = ?sealed_block.extra_data,
            base_fee_per_gas = ?sealed_block.base_fee_per_gas,
            transactions_root = ?sealed_block.transactions_root,
            withdrawals_root = ?sealed_block.withdrawals_root,
            ommers_hash = ?sealed_block.ommers_hash,
            computed_hash = ?block_hash,
            "Block fields after conversion"
        );

        // For V2/V3, use the computed block hash
        let expected_hash = if expected_hash.is_zero() {
            block_hash
        } else {
            expected_hash
        };

        // Ensure the hash included in the payload matches the block hash
        if expected_hash != block_hash {
            warn!(
                target: "rpc::engine::kasplex",
                expected_hash = ?expected_hash,
                computed_hash = ?block_hash,
                block_number = sealed_block.number,
                "Hash mismatch - comparing field by field"
            );

            // Compare individual fields to find the mismatch
            if let Some((_, parent_hash, fee_recipient, state_root, receipts_root, logs_bloom, prev_randao, gas_limit, gas_used, timestamp, ref extra_data, base_fee_per_gas, _, _)) = payload_for_debug {
                if parent_hash != sealed_block.parent_hash {
                    warn!(target: "rpc::engine::kasplex", "parent_hash mismatch: payload={:?}, block={:?}", parent_hash, sealed_block.parent_hash);
                }
                if fee_recipient != sealed_block.beneficiary {
                    warn!(target: "rpc::engine::kasplex", "fee_recipient/beneficiary mismatch: payload={:?}, block={:?}", fee_recipient, sealed_block.beneficiary);
                }
                if state_root != sealed_block.state_root {
                    warn!(target: "rpc::engine::kasplex", "state_root mismatch: payload={:?}, block={:?}", state_root, sealed_block.state_root);
                }
                if receipts_root != sealed_block.receipts_root {
                    warn!(target: "rpc::engine::kasplex", "receipts_root mismatch: payload={:?}, block={:?}", receipts_root, sealed_block.receipts_root);
                }
                if logs_bloom != sealed_block.logs_bloom {
                    warn!(target: "rpc::engine::kasplex", "logs_bloom mismatch");
                }
                if prev_randao != sealed_block.mix_hash {
                    warn!(target: "rpc::engine::kasplex", "prev_randao/mix_hash mismatch: payload={:?}, block={:?}", prev_randao, sealed_block.mix_hash);
                }
                if gas_limit != sealed_block.gas_limit {
                    warn!(target: "rpc::engine::kasplex", "gas_limit mismatch: payload={}, block={}", gas_limit, sealed_block.gas_limit);
                }
                if gas_used != sealed_block.gas_used {
                    warn!(target: "rpc::engine::kasplex", "gas_used mismatch: payload={}, block={}", gas_used, sealed_block.gas_used);
                }
                if timestamp != sealed_block.timestamp {
                    warn!(target: "rpc::engine::kasplex", "timestamp mismatch: payload={}, block={}", timestamp, sealed_block.timestamp);
                }
                if *extra_data != sealed_block.extra_data {
                    warn!(target: "rpc::engine::kasplex", "extra_data mismatch: payload={:?}, block={:?}", extra_data, sealed_block.extra_data);
                }
                if base_fee_per_gas.to::<u64>() != sealed_block.base_fee_per_gas.unwrap_or(0) {
                    warn!(target: "rpc::engine::kasplex", "base_fee_per_gas mismatch: payload={:?}, block={:?}", base_fee_per_gas, sealed_block.base_fee_per_gas);
                }
            }

            return Err(PayloadError::BlockHash {
                execution: sealed_block.hash(),
                consensus: expected_hash,
            })
            .map_err(|e| NewPayloadError::Other(e.into()));
        }

        sealed_block.try_recover().map_err(|e| NewPayloadError::Other(e.into()))
    }

    /// Validates the payload attributes with respect to the header.
    fn validate_payload_attributes_against_header(
        &self,
        attr: &Types::PayloadAttributes,
        header: &<Self::Block as BlockTrait>::Header,
    ) -> Result<(), InvalidPayloadAttributesError> {
        // Kasplex allows block.timestamp == parent.timestamp
        // This is different from standard Ethereum which requires timestamp > parent.timestamp
        if attr.timestamp() < header.timestamp {
            return Err(InvalidPayloadAttributesError::InvalidTimestamp);
        }
        Ok(())
    }

    /// Converts the execution data into a sealed block.
    fn convert_payload_to_block(
        &self,
        execution_data: Types::ExecutionData,
    ) -> Result<reth_primitives_traits::SealedBlock<Self::Block>, NewPayloadError> {
        let recovered = <Self as PayloadValidator<Types>>::ensure_well_formed_payload(self, execution_data)?;
        // RecoveredBlock has a sealed_block method that returns SealedBlock
        Ok(recovered.sealed_block().clone())
    }
}

// EngineApiValidator implementation for KasplexEngineValidator
impl<Types> EngineApiValidator<Types> for KasplexEngineValidator
where
    Types: PayloadTypes<PayloadAttributes = KasplexPayloadAttributes, ExecutionData = alloy_rpc_types_engine::ExecutionData>,
{
    /// Validates the presence or exclusion of fork-specific fields based on the payload attributes
    /// and the message version.
    fn validate_version_specific_fields(
        &self,
        _version: EngineApiMessageVersion,
        _payload_or_attrs: PayloadOrAttributes<'_, Types::ExecutionData, Types::PayloadAttributes>,
    ) -> Result<(), EngineObjectValidationError> {
        // For Kasplex, we don't have version-specific validation beyond standard checks
        Ok(())
    }

    /// Ensures that the payload attributes are valid for the given [`EngineApiMessageVersion`].
    fn ensure_well_formed_attributes(
        &self,
        _version: EngineApiMessageVersion,
        _attributes: &Types::PayloadAttributes,
    ) -> Result<(), EngineObjectValidationError> {
        // Attributes are well-formed if they pass the basic validation
        Ok(())
    }
}

/// Process Kasplex execution data and restore transaction numbers.
/// 
/// This function:
/// 1. Extracts the Numbers array from KasplexExecutionData sidecar (if available)
/// 2. Restores transaction Number fields from the Numbers array
/// 3. If Numbers array is not available, tries to get from transaction mapping
pub fn process_kasplex_execution_data(
    payload: alloy_rpc_types_engine::ExecutionPayload,
    kasplex_data: Option<&KasplexExecutionData>,
) -> Result<Block, PayloadError> {
    // Convert payload to block
    // Extract ExecutionPayloadV1 from the enum and use its try_into_block method
    let payload_v1 = match payload {
        alloy_rpc_types_engine::ExecutionPayload::V1(p) => p,
        _ => return Err(PayloadError::BaseFee(alloy_primitives::U256::ZERO)),
    };
    let mut block: Block = payload_v1.try_into_block()?;
    
    // Fix withdrawals_root for Kasplex blocks
    // Kasplex blocks should have withdrawals_root set to EMPTY_WITHDRAWALS
    // This is required for correct hash calculation to match geth
    if block.header.withdrawals_root.is_none() {
        block.header.withdrawals_root = Some(EMPTY_WITHDRAWALS);
    }
    
    // If we have Kasplex execution data with numbers, restore them
    if let Some(kasplex_data) = kasplex_data {
        let numbers = &kasplex_data.kasplex_sidecar.numbers;
        
        let transactions: Vec<_> = block.body.transactions().collect();
        if !numbers.is_empty() && numbers.len() == transactions.len() {
            trace!(
                target: "rpc::engine::kasplex",
                numbers_count = numbers.len(),
                "Restoring transaction numbers from KasplexExecutionData sidecar"
            );
            
            // Restore transaction numbers from Numbers array
            // Store them in transaction mapping for future reference
            // Use the global shared instance
            let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();
            for (tx, number) in transactions.iter().zip(numbers.iter()) {
                let tx_hash = tx.tx_hash();
                // Store in transaction mapping
                tx_mapping.insert(*tx_hash, *number);
                trace!(
                    target: "rpc::engine::kasplex",
                    ?tx_hash,
                    number,
                    "Stored transaction number from Numbers array"
                );
            }
        } else if !numbers.is_empty() {
            let tx_count = block.body.transactions().count();
            debug!(
                target: "rpc::engine::kasplex",
                numbers_count = numbers.len(),
                tx_count,
                "Numbers array length mismatch, trying transaction mapping"
            );
        }
    }
    
    // If numbers are not available from sidecar, try to get from transaction mapping
    // This handles the case where transactions were submitted via RPC and numbers
    // were stored in the mapping
    // Use the global shared instance
    let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();
    for tx in block.body.transactions() {
        let tx_hash = tx.tx_hash();
        let number = tx_mapping.get_tx_number(*tx_hash);
        if number != 0 {
            trace!(
                target: "rpc::engine::kasplex",
                ?tx_hash,
                number,
                "Found transaction number in mapping"
            );
        }
    }
    
    Ok(block)
}

/// Validate Kasplex-specific block rules.
/// 
/// For Kasplex:
/// - Allow block.timestamp == parent.timestamp
/// - Other standard validations still apply
pub fn validate_kasplex_block(
    block: &reth::primitives::SealedBlock,
    parent_timestamp: u64,
) -> Result<(), String> {
    // Kasplex allows block.timestamp == parent.timestamp
    // This is different from standard Ethereum which requires timestamp > parent.timestamp
    if block.timestamp < parent_timestamp {
        return Err(format!(
            "Block timestamp {} is less than parent timestamp {}",
            block.timestamp, parent_timestamp
        ));
    }
    
    Ok(())
}

/// Check if a ForkChoiceUpdate should allow reorg for Kasplex.
/// 
/// For Kasplex networks, reorgs are allowed.
pub fn kasplex_allow_reorg(chain_spec: &KasplexChainSpec) -> bool {
    chain_spec.is_kasplex()
}

/// Check if NewPayload should accept txHash instead of full transactions for Kasplex.
/// 
/// For Kasplex, NewPayload can accept txHash in the sidecar instead of full transactions.
pub fn kasplex_accept_tx_hash(
    chain_spec: &KasplexChainSpec,
    kasplex_data: Option<&KasplexExecutionData>,
) -> bool {
    if !chain_spec.is_kasplex() {
        return false;
    }
    
    // If we have Kasplex execution data with tx_hash, we can use it
    if let Some(data) = kasplex_data {
        !data.kasplex_sidecar.tx_hash.is_zero()
    } else {
        false
    }
}

/// Check if a block is a Kasplex block based on the execution data.
pub fn is_kasplex_block(
    kasplex_data: Option<&KasplexExecutionData>,
) -> bool {
    kasplex_data
        .map(|data| data.kasplex_sidecar.kasplex_block)
        .unwrap_or(false)
}
