pub use kasplex_reth_block as block;
pub use kasplex_reth_chainspec as chainspec;
pub use kasplex_reth_consensus as consensus;
pub use kasplex_reth_network as network;
pub use kasplex_reth_payload as payload;
pub use kasplex_reth_primitives as primitives;
pub use kasplex_reth_rpc as rpc;

use block::{config::KasplexEvmConfig, KasplexExecutorBuilder};
use chainspec::spec::KasplexChainSpec;
use consensus::builder::KasplexConsensusBuilder;
use network::KasplexNetworkBuilder;
use payload::KasplexPayloadBuilderBuilder;
use primitives::engine::KasplexEngineTypes;

mod pool;
pub use pool::KasplexPoolBuilder;
use reth::{
    api::{FullNodeComponents, FullNodeTypes, NodeTypes},
    builder::{
        DebugNode, Node,
        components::{BasicPayloadServiceBuilder, ComponentsBuilder},
    },
    providers::EthStorage,
};
// LocalPayloadAttributesBuilder is from reth_engine_local, but we'll implement it ourselves
use reth_engine_primitives::{EngineApiValidator, PayloadValidator};
use reth_ethereum::EthPrimitives;
use reth_node_api::{BlockTy, NodeAddOns, PayloadAttributesBuilder, PayloadTypes};
use reth_engine_local::LocalPayloadAttributesBuilder;
use std::sync::Arc;
use reth_node_builder::{
    NodeAdapter,
    rpc::{
        BasicEngineValidatorBuilder, EngineValidatorAddOn, PayloadValidatorBuilder, RethRpcAddOns,
        RpcAddOns, RpcHandle, RpcHooks,
    },
};
use reth_rpc::eth::core::EthRpcConverterFor;
use rpc::{
    engine::builder::KasplexEngineApiBuilder,
    eth::{builder::KasplexEthApiBuilder, types::KasplexEthApi},
};

/// The main node type for a Kasplex network node, implementing the `NodeTypes` trait.
#[derive(Debug, Clone, Default)]
pub struct KasplexNode;

impl NodeTypes for KasplexNode {
    /// The node's primitive types, defining basic operations and structures.
    type Primitives = EthPrimitives;
    /// The type used for configuration of the EVM.
    type ChainSpec = KasplexChainSpec;
    /// The type responsible for writing chain primitives to storage.
    type Storage = EthStorage;
    /// The node's engine types, defining the interaction with the consensus engine.
    type Payload = KasplexEngineTypes;
}

/// Kasplex custom addons which configuring RPC types.
pub struct KasplexAddOns<N: FullNodeComponents<Types = KasplexNode, Evm = KasplexEvmConfig>, PVB>(
    RpcAddOns<N, KasplexEthApiBuilder, PVB, KasplexEngineApiBuilder<PVB>>,
);

impl<N, PVB> Default for KasplexAddOns<N, PVB>
where
    N: FullNodeComponents<Types = KasplexNode, Evm = KasplexEvmConfig>,
    PVB: Default,
{
    /// Creates a new instance of `KasplexAddOns` with default configurations.
    fn default() -> Self {
        let add_ons = RpcAddOns::new(
            KasplexEthApiBuilder::default(),
            PVB::default(),
            KasplexEngineApiBuilder::new(PVB::default()),
            Default::default(),
            Default::default(),
        );

        KasplexAddOns(add_ons)
    }
}

impl<N, PVB> NodeAddOns<N> for KasplexAddOns<N, PVB>
where
    N: FullNodeComponents<Types = KasplexNode, Evm = KasplexEvmConfig>,
    PVB: PayloadValidatorBuilder<N> + Clone,
    PVB::Validator: PayloadValidator<<N::Types as NodeTypes>::Payload, Block = BlockTy<N::Types>>
        + EngineApiValidator<<N::Types as NodeTypes>::Payload>,
{
    /// Handle to add-ons.
    type Handle = RpcHandle<N, KasplexEthApi<N, EthRpcConverterFor<N>>>;

    /// Configures and launches the add-ons.
    async fn launch_add_ons(
        self,
        ctx: reth_node_api::AddOnsContext<'_, N>,
    ) -> eyre::Result<Self::Handle> {
        // Note: RPC registration is done in main.rs via extend_rpc_modules
        // Pool reorg handling can be set up separately if needed
        self.0.launch_add_ons(ctx).await
    }
}

impl<N, PVB> RethRpcAddOns<N> for KasplexAddOns<N, PVB>
where
    N: FullNodeComponents<Types = KasplexNode, Evm = KasplexEvmConfig>,
    PVB: PayloadValidatorBuilder<N> + Clone,
    PVB::Validator: PayloadValidator<<N::Types as NodeTypes>::Payload, Block = BlockTy<N::Types>>
        + EngineApiValidator<<N::Types as NodeTypes>::Payload>,
{
    /// eth API implementation.
    type EthApi = KasplexEthApi<N, EthRpcConverterFor<N>>;

    /// Returns a mutable reference to RPC hooks.
    fn hooks_mut(&mut self) -> &mut RpcHooks<N, Self::EthApi> {
        self.0.hooks_mut()
    }
}

impl<N, PVB> EngineValidatorAddOn<N> for KasplexAddOns<N, PVB>
where
    N: FullNodeComponents<Types = KasplexNode, Evm = KasplexEvmConfig>,
    PVB: PayloadValidatorBuilder<N> + Send,
    PVB::Validator: PayloadValidator<<N::Types as NodeTypes>::Payload, Block = BlockTy<N::Types>>
        + EngineApiValidator<<N::Types as NodeTypes>::Payload>,
{
    /// The ValidatorBuilder type to use for the engine API.
    type ValidatorBuilder = BasicEngineValidatorBuilder<PVB>;

    /// Returns the engine validator builder.
    fn engine_validator_builder(&self) -> Self::ValidatorBuilder {
        EngineValidatorAddOn::<N>::engine_validator_builder(&self.0)
    }
}

impl<N> Node<N> for KasplexNode
where
    N: FullNodeTypes<Types = Self>,
{
    /// The type that builds the node's components.
    type ComponentsBuilder = ComponentsBuilder<
        N,
        KasplexPoolBuilder,
        BasicPayloadServiceBuilder<KasplexPayloadBuilderBuilder>,
        KasplexNetworkBuilder,
        block::factory::KasplexExecutorBuilder,
        KasplexConsensusBuilder,
    >;

    /// Exposes the customizable node add-on types.
    type AddOns = KasplexAddOns<NodeAdapter<N>, rpc::engine::validator::KasplexEngineValidatorBuilder>;

    /// Returns a [`NodeComponentsBuilder`] for the node.
    fn components_builder(&self) -> Self::ComponentsBuilder {
        ComponentsBuilder::default()
            .node_types()
            .pool(KasplexPoolBuilder::new())
            .executor(KasplexExecutorBuilder::default())
            .payload(BasicPayloadServiceBuilder::new(KasplexPayloadBuilderBuilder))
            .network(KasplexNetworkBuilder)
            .consensus(KasplexConsensusBuilder::default())
    }

    /// Returns the node add-ons.
    fn add_ons(&self) -> Self::AddOns {
        KasplexAddOns::default()
    }
}

impl<N: FullNodeComponents<Types = Self>> DebugNode<N> for KasplexNode {
    /// RPC block type. Used by [`DebugConsensusClient`] to fetch blocks and submit them to the
    /// engine.
    type RpcBlock = alloy_rpc_types_eth::Block;

    /// Converts an RPC block to a primitive block.
    fn rpc_to_primitive_block(rpc_block: Self::RpcBlock) -> reth_ethereum_primitives::Block {
        rpc_block.into_consensus().convert_transactions()
    }

    /// Creates a payload attributes builder for local mining in dev mode.
    fn local_payload_attributes_builder(
        chain_spec: &Self::ChainSpec,
    ) -> impl PayloadAttributesBuilder<
        <<Self as reth_node_api::NodeTypes>::Payload as PayloadTypes>::PayloadAttributes,
    > {
        LocalPayloadAttributesBuilder::new(Arc::new(chain_spec.clone()))
    }
}

