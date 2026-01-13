use kasplex_reth_primitives::{
    engine::types::KasplexExecutionData, payload::attributes::KasplexPayloadAttributes,
};
use kasplex_reth_txpool::{handle_kasplex_pool_reorg, remove_included_transactions};
use kasplex_reth_tx_mapping;
use alloy_hardforks::EthereumHardforks;
use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated, PayloadId, PayloadStatus};
use async_trait::async_trait;
use jsonrpsee::{RpcModule, proc_macros::rpc, core::RpcResult};
use reth::{
    rpc::api::IntoEngineApiRpcModule, transaction_pool::TransactionPool,
};
use reth_engine_primitives::EngineApiValidator;
use reth_ethereum_engine_primitives::EthBuiltPayload;
use reth_node_api::{EngineTypes, PayloadTypes};
use reth_provider::{
    BlockReader, DatabaseProviderFactory, HeaderProvider, StateProviderFactory,
    BlockReaderIdExt, ChainSpecProvider,
};
use reth::rpc::types::BlockHashOrNumber;
use reth_primitives_traits::Block;
use alloy_consensus::BlockHeader;
use reth_rpc::EngineApi;
use std::sync::Arc;
use tracing::warn;

/// The list of all supported Engine capabilities available over the engine endpoint.
pub const KASPLEX_ENGINE_CAPABILITIES: &[&str] =
    &["engine_forkchoiceUpdatedV2", "engine_getPayloadV2", "engine_newPayloadV2"];

/// Extension trait that gives access to Kasplex engine API RPC methods.
///
/// Note:
/// > The provider should use a JWT authentication layer.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "engine", server_bounds(Engine::PayloadAttributes: jsonrpsee::core::DeserializeOwned)))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "engine", client_bounds(Engine::PayloadAttributes: jsonrpsee::core::Serialize + Clone), server_bounds(Engine::PayloadAttributes: jsonrpsee::core::DeserializeOwned)))]
pub trait KasplexEngineApi<Engine: EngineTypes> {
    #[method(name = "newPayloadV2")]
    async fn new_payload_v2(&self, payload: KasplexExecutionData) -> RpcResult<PayloadStatus>;

    #[method(name = "forkchoiceUpdatedV2")]
    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<Engine::PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated>;

    #[method(name = "getPayloadV2")]
    async fn get_payload_v2(
        &self,
        payload_id: PayloadId,
    ) -> RpcResult<Engine::ExecutionPayloadEnvelopeV2>;
}

/// A concrete implementation of the `KasplexEngineApi` trait.
pub struct KasplexEngineApi<Provider, PayloadT: PayloadTypes, Pool, Validator, ChainSpec> {
    inner: EngineApi<Provider, PayloadT, Pool, Validator, ChainSpec>,
    /// Provider for accessing blockchain state
    provider: Arc<Provider>,
    /// Transaction pool for managing transactions
    pool: Arc<Pool>,
}

impl<Provider, PayloadT: PayloadTypes, Pool, Validator, ChainSpec>
    KasplexEngineApi<Provider, PayloadT, Pool, Validator, ChainSpec>
where
    Provider:
        HeaderProvider + BlockReader + DatabaseProviderFactory + StateProviderFactory
        + BlockReaderIdExt + ChainSpecProvider<ChainSpec = ChainSpec> + 'static,
    PayloadT: PayloadTypes,
    Pool: TransactionPool + 'static,
    ChainSpec: EthereumHardforks + Send + Sync + 'static,
{
    /// Creates a new instance of `KasplexEngineApi` with the given parameters.
    pub fn new(
        engine_api: EngineApi<Provider, PayloadT, Pool, Validator, ChainSpec>,
        provider: Arc<Provider>,
        pool: Arc<Pool>,
    ) -> Self {
        Self {
            inner: engine_api,
            provider,
            pool,
        }
    }
}

// This is the concrete kasplex engine API implementation.
#[async_trait]
impl<Provider, EngineT, Pool, Validator, ChainSpec> KasplexEngineApiServer<EngineT>
    for KasplexEngineApi<Provider, EngineT, Pool, Validator, ChainSpec>
where
    Provider:
        HeaderProvider + BlockReader + DatabaseProviderFactory + StateProviderFactory
        + BlockReaderIdExt + ChainSpecProvider<ChainSpec = ChainSpec> + 'static,
    EngineT: EngineTypes<
        ExecutionData = alloy_rpc_types_engine::ExecutionData,
        PayloadAttributes = KasplexPayloadAttributes,
        BuiltPayload = EthBuiltPayload,
    >,
    Pool: TransactionPool + 'static,
    Validator: EngineApiValidator<EngineT>,
    ChainSpec: EthereumHardforks + Send + Sync + 'static + kasplex_reth_chainspec::spec::KasplexChainCheck,
{
    /// Creates a new execution payload with the given execution data.
    async fn new_payload_v2(&self, payload: KasplexExecutionData) -> RpcResult<PayloadStatus> {
        // Extract block hash and number from payload before processing
        let block_hash = payload.execution_payload.block_hash;
        let block_number = payload.execution_payload.block_number;

        // Convert KasplexExecutionData to ExecutionData
        let execution_data = alloy_rpc_types_engine::ExecutionData {
            payload: payload.execution_payload.into(),
            sidecar: alloy_rpc_types_engine::ExecutionPayloadSidecar::none(),
        };

        let result = self.inner.new_payload_v2(execution_data).await?;

        // After a block is finalized, remove included transactions from the pool
        // This is similar to Geth's behavior where transactions are removed after being included
        // Always try to remove transactions if we have a valid block hash
        // (reth's TransactionPool will handle the actual removal via on_canonical_state_change)
        if let Ok(Some(block)) = self.provider.block_by_hash(block_hash) {
            let sealed_block = block.seal_slow();

            // Log executed block's state_root and hash for debugging
            // This helps compare with geth's execution results
            let header = sealed_block.header();
            tracing::info!(
                target: "kasplex::engine",
                block_number,
                ?block_hash,
                state_root = ?header.state_root(),
                receipts_root = ?header.receipts_root(),
                transactions_root = ?header.transactions_root(),
                computed_block_hash = ?sealed_block.hash(),
                payload_status = ?result.status,
                "Block executed - state_root and roots after execution"
            );

            let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();

            if let Err(e) = remove_included_transactions(
                self.pool.as_ref(),
                &sealed_block,
                tx_mapping,
            ).await {
                warn!(
                    target: "kasplex::engine",
                    block_number,
                    ?block_hash,
                    "Failed to remove included transactions from pool: {}",
                    e
                );
            } else {
                tracing::debug!(
                    target: "kasplex::engine",
                    block_number,
                    ?block_hash,
                    "Removed included transactions from pool"
                );
            }
        }

        Ok(result)
    }

    /// Updates the fork choice with the given state and payload attributes.
    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<EngineT::PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated> {
        // Delegate to the inner EngineApi first
        let result = self.inner.fork_choice_updated_v2(fork_choice_state, payload_attributes).await?;

        // After a reorg (forkchoice update), handle Kasplex-specific pool cleanup
        // This is similar to Geth's ClearForReorg() which removes transactions
        // that were submitted after the reorg point
        // Get the current head block number after the forkchoice update
        if let Ok(Some(current_header)) = self.provider.header_by_hash_or_number(
            BlockHashOrNumber::Hash(fork_choice_state.head_block_hash)
        ) {
            let new_head_number = current_header.number();
            let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();

            if let Err(e) = handle_kasplex_pool_reorg(
                self.provider.as_ref(),
                self.pool.as_ref(),
                tx_mapping,
                new_head_number,
            ).await {
                warn!(
                    target: "kasplex::engine",
                    new_head_number,
                    ?fork_choice_state.head_block_hash,
                    "Failed to handle pool reorg: {}",
                    e
                );
            } else {
                tracing::debug!(
                    target: "kasplex::engine",
                    new_head_number,
                    ?fork_choice_state.head_block_hash,
                    "Handled pool reorg successfully"
                );
            }
        } else {
            // Fallback: try to get best block number
            if let Ok(best_number) = self.provider.best_block_number() {
                let tx_mapping = kasplex_reth_tx_mapping::get_global_tx_mapping();
                if let Err(e) = handle_kasplex_pool_reorg::<_, _, ChainSpec>(
                    self.provider.as_ref(),
                    self.pool.as_ref(),
                    &tx_mapping,
                    best_number,
                ).await {
                    warn!(
                        target: "kasplex::engine",
                        best_number,
                        "Failed to handle pool reorg (using best block): {}",
                        e
                    );
                }
            }
        }

        Ok(result)
    }

    /// Retrieves the execution payload by its ID.
    async fn get_payload_v2(
        &self,
        payload_id: PayloadId,
    ) -> RpcResult<EngineT::ExecutionPayloadEnvelopeV2> {
        self.inner.get_payload_v2(payload_id).await.map_err(|e| e.into())
    }
}

impl<Provider, EngineT, Pool, Validator, ChainSpec> IntoEngineApiRpcModule
    for KasplexEngineApi<Provider, EngineT, Pool, Validator, ChainSpec>
where
    EngineT: EngineTypes,
    Self: KasplexEngineApiServer<EngineT>,
{
    /// Consumes the type and returns all the methods and subscriptions defined in the trait and
    /// returns them as a single [`RpcModule`]
    fn into_rpc_module(self) -> RpcModule<()> {
        self.into_rpc().remove_context()
    }
}

