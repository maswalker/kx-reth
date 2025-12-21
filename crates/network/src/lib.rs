use reth::{
    network::{EthNetworkPrimitives, NetworkHandle, PeersInfo},
    transaction_pool::{PoolTransaction, TransactionPool},
};
use reth_ethereum::{EthPrimitives, PooledTransactionVariant};
use reth_node_api::{FullNodeTypes, NodeTypes, TxTy};
use reth_node_builder::{BuilderContext, components::NetworkBuilder};
use tracing::info;

use kasplex_reth_chainspec::spec::KasplexChainSpec;

/// Kasplex network builder.
#[derive(Debug, Default, Clone, Copy)]
pub struct KasplexNetworkBuilder;

impl<Node, Pool> NetworkBuilder<Node, Pool> for KasplexNetworkBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec = KasplexChainSpec, Primitives = EthPrimitives>>,
    Pool: TransactionPool<
            Transaction: PoolTransaction<
                Consensus = TxTy<Node::Types>,
                Pooled = PooledTransactionVariant,
            >,
        > + Unpin
        + 'static,
{
    /// The network built.
    type Network = NetworkHandle<EthNetworkPrimitives>;

    /// Launches the network implementation and returns the handle to it.
    async fn build_network(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<Self::Network> {
        let network = ctx.network_builder().await?;
        let handle = ctx.start_network(network, pool);
        info!(target: "reth::kasplex::cli", enode=%handle.local_node_record(), "P2P networking initialized");
        Ok(handle)
    }
}

