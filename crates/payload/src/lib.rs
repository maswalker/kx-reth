use reth::{
    api::{FullNodeTypes, NodeTypes},
    builder::{BuilderContext, components::PayloadBuilderBuilder},
    transaction_pool::{PoolTransaction, TransactionPool},
};
use reth_ethereum::{EthPrimitives, TransactionSigned};

use kasplex_reth_block::config::KasplexEvmConfig;
use kasplex_reth_chainspec::spec::KasplexChainSpec;
use kasplex_reth_primitives::engine::KasplexEngineTypes;

pub mod builder;

pub use builder::KasplexPayloadBuilder;

/// The builder to spawn [`KasplexPayloadBuilder`] payload building tasks.
#[derive(Debug, Default, Clone)]
pub struct KasplexPayloadBuilderBuilder;

impl<Node, Pool> PayloadBuilderBuilder<Node, Pool, KasplexEvmConfig> for KasplexPayloadBuilderBuilder
where
    Node: FullNodeTypes<
        Types: NodeTypes<
            Primitives = EthPrimitives,
            ChainSpec = KasplexChainSpec,
            Payload = KasplexEngineTypes,
        >,
    >,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>
        + Unpin
        + 'static,
{
    /// Payload builder implementation.
    type PayloadBuilder = KasplexPayloadBuilder<Node::Provider, KasplexEvmConfig>;

    /// Spawns the payload service and returns the handle to it.
    async fn build_payload_builder(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
        evm_config: KasplexEvmConfig,
    ) -> eyre::Result<Self::PayloadBuilder> {
        let _ = pool;
        Ok(KasplexPayloadBuilder::new(ctx.provider().clone(), evm_config))
    }
}




