use kasplex_reth_primitives::{
    engine::types::KasplexExecutionData, payload::attributes::KasplexPayloadAttributes,
};
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
};
use reth_rpc::EngineApi;

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
}

impl<Provider, PayloadT: PayloadTypes, Pool, Validator, ChainSpec>
    KasplexEngineApi<Provider, PayloadT, Pool, Validator, ChainSpec>
where
    Provider:
        HeaderProvider + BlockReader + DatabaseProviderFactory + StateProviderFactory + 'static,
    PayloadT: PayloadTypes,
    Pool: TransactionPool + 'static,
    ChainSpec: EthereumHardforks + Send + Sync + 'static,
{
    /// Creates a new instance of `KasplexEngineApi` with the given parameters.
    pub fn new(
        engine_api: EngineApi<Provider, PayloadT, Pool, Validator, ChainSpec>,
    ) -> Self {
        Self { inner: engine_api }
    }
}

// This is the concrete kasplex engine API implementation.
#[async_trait]
impl<Provider, EngineT, Pool, Validator, ChainSpec> KasplexEngineApiServer<EngineT>
    for KasplexEngineApi<Provider, EngineT, Pool, Validator, ChainSpec>
where
    Provider:
        HeaderProvider + BlockReader + DatabaseProviderFactory + StateProviderFactory + 'static,
    EngineT: EngineTypes<
        ExecutionData = alloy_rpc_types_engine::ExecutionData,
        PayloadAttributes = KasplexPayloadAttributes,
        BuiltPayload = EthBuiltPayload,
    >,
    Pool: TransactionPool + 'static,
    Validator: EngineApiValidator<EngineT>,
    ChainSpec: EthereumHardforks + Send + Sync + 'static,
{
    /// Creates a new execution payload with the given execution data.
    async fn new_payload_v2(&self, payload: KasplexExecutionData) -> RpcResult<PayloadStatus> {
        // Convert KasplexExecutionData to ExecutionData
        let execution_data = alloy_rpc_types_engine::ExecutionData {
            payload: payload.execution_payload.into(),
            sidecar: alloy_rpc_types_engine::ExecutionPayloadSidecar::none(),
        };
        self.inner.new_payload_v2(execution_data).await.map_err(|e| e.into())
    }

    /// Updates the fork choice with the given state and payload attributes.
    async fn fork_choice_updated_v2(
        &self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<EngineT::PayloadAttributes>,
    ) -> RpcResult<ForkchoiceUpdated> {
        // For Kasplex, we don't need to persist L1 origin like Taiko does.
        // We simply delegate to the inner EngineApi.
        self.inner.fork_choice_updated_v2(fork_choice_state, payload_attributes).await.map_err(|e| e.into())
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

