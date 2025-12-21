use kasplex_reth_block::config::KasplexEvmConfig;
use kasplex_reth_chainspec::spec::KasplexChainSpec;
use kasplex_reth_primitives::engine::KasplexEngineTypes;

use crate::eth::types::KasplexEthApi;
use reth_ethereum::EthPrimitives;
use reth_node_api::{FullNodeComponents, NodeTypes};
use reth_node_builder::rpc::{EthApiBuilder, EthApiCtx};
use reth_node_ethereum::EthereumEthApiBuilder;
use reth_rpc::eth::core::EthRpcConverterFor;

/// Builds [`KasplexEthApi`] for the Kasplex node.
#[derive(Debug, Default)]
pub struct KasplexEthApiBuilder(EthereumEthApiBuilder);

impl KasplexEthApiBuilder {
    /// Creates a new instance of `KasplexEthApiBuilder`.
    pub fn new() -> Self {
        Self(EthereumEthApiBuilder::default())
    }
}

impl<N> EthApiBuilder<N> for KasplexEthApiBuilder
where
    N: FullNodeComponents<Evm = KasplexEvmConfig>,
    N::Types: NodeTypes<
            Primitives = EthPrimitives,
            ChainSpec = KasplexChainSpec,
            Payload = KasplexEngineTypes,
        >,
{
    /// The Ethapi implementation this builder will build.
    type EthApi = KasplexEthApi<N, EthRpcConverterFor<N>>;

    /// Builds the [`KasplexEthApi`] from the given context.
    async fn build_eth_api(self, ctx: EthApiCtx<'_, N>) -> eyre::Result<Self::EthApi> {
        let api = ctx.eth_api_builder().build();

        Ok(KasplexEthApi(api))
    }
}
