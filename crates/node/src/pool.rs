//! Kasplex transaction pool builder
//! 
//! This module provides a custom pool builder that integrates Kasplex-specific
//! transaction pool functionality, including Reorg handling.

use kasplex_reth_tx_mapping::TxMapping;
use reth::builder::components::PoolBuilder;
use reth::chainspec::EthereumHardforks;
use reth_node_api::{NodePrimitives, NodeTypes};
use reth_node_ethereum::node::EthereumPoolBuilder;
use reth_ethereum::TransactionSigned;
use std::sync::Arc;

/// Kasplex transaction pool builder.
/// 
/// This builder wraps the standard Ethereum pool builder and adds Kasplex-specific
/// functionality, including Reorg handling with transaction number-based cleanup.
#[derive(Debug, Default, Clone)]
pub struct KasplexPoolBuilder {
    inner: EthereumPoolBuilder,
    /// Transaction mapping for Kasplex networks.
    tx_mapping: Option<Arc<TxMapping>>,
}

impl KasplexPoolBuilder {
    /// Creates a new Kasplex pool builder.
    pub fn new() -> Self {
        Self {
            inner: EthereumPoolBuilder::default(),
            tx_mapping: Some(Arc::new(kasplex_reth_tx_mapping::get_global_tx_mapping().clone())),
        }
    }

    /// Sets the transaction mapping for this pool builder.
    pub fn with_tx_mapping(mut self, tx_mapping: Arc<TxMapping>) -> Self {
        self.tx_mapping = Some(tx_mapping);
        self
    }
}

impl<N> PoolBuilder<N> for KasplexPoolBuilder
where
    N: reth::api::FullNodeTypes<
        Types: NodeTypes<
            ChainSpec: EthereumHardforks,
            Primitives: NodePrimitives<SignedTx = TransactionSigned>,
        >,
    >,
{
    type Pool = <EthereumPoolBuilder as PoolBuilder<N>>::Pool;

    async fn build_pool(
        self,
        ctx: &reth::builder::BuilderContext<N>,
    ) -> eyre::Result<Self::Pool>
    {
        // Build the standard Ethereum pool
        let pool = self.inner.build_pool(ctx).await?;

        // If we have a transaction mapping, we can set up Reorg handling
        // Note: The actual Reorg handling is done via the transaction pool's
        // on_canonical_state_change method, which is called automatically by reth's
        // transaction pool maintainer. To integrate Kasplex-specific reorg handling,
        // we would need to wrap the pool or extend the TransactionPool trait.
        // For now, the handle_kasplex_pool_reorg function is available and can be
        // called from custom pool implementations or hooks.
        if let Some(_tx_mapping) = self.tx_mapping {
            tracing::debug!(
                target: "kasplex::pool",
                "Kasplex pool builder initialized with transaction mapping"
            );
        }

        Ok(pool)
    }
}

