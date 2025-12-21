use reth_rpc_eth_api::{
    RpcConvert,
    helpers::{
        EthApiSpec, EthBlocks, EthState, EthTransactions, LoadTransaction,
        Call, EthCall, EthFees, LoadFee, LoadBlock, LoadState, LoadReceipt,
        Trace, LoadPendingBlock, SpawnBlocking, estimate::EstimateCall,
        pending_block::PendingEnvBuilder, spec::SignersForRpc,
    },
    RpcNodeCore, RpcNodeCoreExt, EthApiTypes,
};
use reth_rpc_eth_types::{
    EthApiError, EthStateCache, FeeHistoryCache, GasPriceOracle, PendingBlock,
    builder::config::PendingBlockKind, error::FromEvmError, utils::recover_raw_transaction,
};
use reth_rpc::EthApi;
use reth_provider::ProviderHeader;
use reth::tasks::{
    TaskSpawner,
    pool::{BlockingTaskGuard, BlockingTaskPool},
};
use reth_transaction_pool::{PoolTransaction, TransactionPool, TransactionOrigin};
use reth::primitives::TransactionSigned;
use jsonrpsee::tokio;
use std::time::Duration;

/// Kasplex ETH API wrapper.
/// 
/// This wraps the standard EthApi and can be extended with Kasplex-specific functionality.
pub struct KasplexEthApi<N: RpcNodeCore, Rpc: RpcConvert>(pub EthApi<N, Rpc>);

impl<N: RpcNodeCore, Rpc: RpcConvert> Clone for KasplexEthApi<N, Rpc> {
    fn clone(&self) -> Self {
        KasplexEthApi(self.0.clone())
    }
}

impl<N: RpcNodeCore, Rpc: RpcConvert> std::fmt::Debug for KasplexEthApi<N, Rpc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KasplexEthApi").finish_non_exhaustive()
    }
}

// Implement EthApiTypes for KasplexEthApi by delegating to inner EthApi
impl<N, Rpc> EthApiTypes for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    Rpc: RpcConvert,
{
    type Error = EthApiError;
    type NetworkTypes = Rpc::Network;
    type RpcConvert = Rpc;

    fn tx_resp_builder(&self) -> &Self::RpcConvert {
        self.0.tx_resp_builder()
    }
}

// Implement RpcNodeCore for KasplexEthApi by delegating to inner EthApi
impl<N: RpcNodeCore, Rpc: RpcConvert> RpcNodeCore for KasplexEthApi<N, Rpc> {
    type Primitives = N::Primitives;
    type Provider = N::Provider;
    type Pool = N::Pool;
    type Evm = N::Evm;
    type Network = N::Network;

    fn pool(&self) -> &Self::Pool {
        self.0.pool()
    }

    fn evm_config(&self) -> &Self::Evm {
        self.0.evm_config()
    }

    fn network(&self) -> &Self::Network {
        self.0.network()
    }

    fn provider(&self) -> &Self::Provider {
        self.0.provider()
    }
}

impl<N, Rpc> RpcNodeCoreExt for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    Rpc: RpcConvert,
{
    fn cache(&self) -> &EthStateCache<N::Primitives> {
        self.0.cache()
    }
}

impl<N, Rpc> SpawnBlocking for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    Rpc: RpcConvert,
{
    fn io_task_spawner(&self) -> impl TaskSpawner {
        self.0.task_spawner()
    }

    fn tracing_task_pool(&self) -> &BlockingTaskPool {
        self.0.blocking_task_pool()
    }

    fn tracing_task_guard(&self) -> &BlockingTaskGuard {
        self.0.blocking_task_guard()
    }
}

impl<N, Rpc> LoadReceipt for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
}

impl<N, Rpc> Trace for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
}

impl<N, Rpc> EthFees for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
}

impl<N, Rpc> LoadFee for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
    fn gas_oracle(&self) -> &GasPriceOracle<Self::Provider> {
        self.0.gas_oracle()
    }

    fn fee_history_cache(&self) -> &FeeHistoryCache<ProviderHeader<N::Provider>> {
        self.0.fee_history_cache()
    }
}

impl<N, Rpc> LoadBlock for KasplexEthApi<N, Rpc>
where
    Self: LoadPendingBlock,
    N: RpcNodeCore,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
}

impl<N, Rpc> EthCall for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError, Evm = N::Evm>,
{
}

impl<N, Rpc> EstimateCall for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError, Evm = N::Evm>,
{
}

impl<N, Rpc> Call for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError, Evm = N::Evm>,
{
    fn call_gas_limit(&self) -> u64 {
        self.0.gas_cap()
    }

    fn max_simulate_blocks(&self) -> u64 {
        self.0.max_simulate_blocks()
    }

    fn evm_memory_limit(&self) -> u64 {
        self.0.evm_memory_limit()
    }
}

impl<N, Rpc> EthState for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
    fn max_proof_window(&self) -> u64 {
        self.0.eth_proof_window()
    }
}

impl<N, Rpc> LoadState for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
}

impl<N, Rpc> EthBlocks for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
}

impl<N, Rpc> LoadPendingBlock for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives>,
{
    fn pending_block(&self) -> &tokio::sync::Mutex<Option<PendingBlock<Self::Primitives>>> {
        self.0.pending_block()
    }

    fn pending_block_kind(&self) -> PendingBlockKind {
        self.0.pending_block_kind()
    }

    fn pending_env_builder(&self) -> &dyn PendingEnvBuilder<Self::Evm> {
        self.0.pending_env_builder()
    }
}

impl<N, Rpc> EthTransactions for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
    <N::Pool as TransactionPool>::Transaction: PoolTransaction<Consensus = TransactionSigned>,
{
    fn signers(&self) -> &SignersForRpc<Self::Provider, Self::NetworkTypes> {
        self.0.signers()
    }

    fn send_raw_transaction_sync_timeout(&self) -> Duration {
        self.0.send_raw_transaction_sync_timeout()
    }

    /// Decodes and recovers the transaction and submits it to the pool.
    ///
    /// Returns the hash of the transaction.
    async fn send_raw_transaction(&self, tx: alloy_primitives::Bytes) -> Result<reth::revm::primitives::B256, Self::Error> {
        let recovered = recover_raw_transaction(&tx)?;

        // broadcast raw transaction to subscribers if there is any.
        self.0.broadcast_raw_transaction(tx);

        let pool_transaction = <Self::Pool as TransactionPool>::Transaction::from_pooled(recovered);

        // submit the transaction to the pool with a `Local` origin
        let outcome =
            self.pool().add_transaction(TransactionOrigin::Local, pool_transaction).await?;

        Ok(outcome.hash)
    }
}

impl<N, Rpc> LoadTransaction for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    EthApiError: FromEvmError<N::Evm>,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
}

impl<N, Rpc> EthApiSpec for KasplexEthApi<N, Rpc>
where
    N: RpcNodeCore,
    Rpc: RpcConvert<Primitives = N::Primitives, Error = EthApiError>,
{
    fn starting_block(&self) -> reth::revm::primitives::U256 {
        self.0.starting_block()
    }
}
