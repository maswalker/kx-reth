use std::sync::Arc;

use reth_ethereum::EthPrimitives;
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_builder::{BuilderContext, components::ConsensusBuilder};

use crate::validation::KasplexBeaconConsensus;
use kasplex_reth_chainspec::spec::KasplexChainSpec;

/// A basic Kasplex consensus builder.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct KasplexConsensusBuilder;

impl<Node> ConsensusBuilder<Node> for KasplexConsensusBuilder
where
    Node: FullNodeTypes<
        Types: NodeTypes<
            Primitives = EthPrimitives,
            ChainSpec = KasplexChainSpec,
        >,
    >,
{
    /// The consensus implementation to build.
    type Consensus = Arc<KasplexBeaconConsensus>;

    /// Creates the KasplexBeaconConsensus implementation.
    async fn build_consensus(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Consensus> {
        Ok(Arc::new(KasplexBeaconConsensus::new(ctx.chain_spec())))
    }
}


