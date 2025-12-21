use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use jsonrpsee::types::error::ErrorObject;
use alloy_primitives::Bytes;
use reth::primitives::{Recovered, TransactionSigned};
use alloy_primitives::U256;
use reth_provider::{BlockReaderIdExt, ChainSpecProvider};
use alloy_rpc_types_txpool::TxpoolContent;
use reth_rpc_eth_api::RpcTransaction;
use reth_transaction_pool::{AllPoolTransactions, PoolTransaction, TransactionPool, TransactionOrigin};
use reth_rpc_eth_api::RpcConvert;
use reth_rpc_eth_types::{EthApiError, error::FromEvmError};
use reth_evm::ConfigureEvm;
use reth_ethereum::EthPrimitives;
use reth_rpc_eth_types::utils::recover_raw_transaction;
use std::collections::BTreeMap;
use tracing::trace;
use serde_json;

/// Kasplex Auth RPC API server trait
#[rpc(server, namespace = "kasplexAuth")]
pub trait KasplexAuthApiServer {
    /// Returns the content of the transaction pool.
    #[method(name = "txPoolContent")]
    async fn tx_pool_content(&self) -> RpcResult<serde_json::Value>;

    /// Returns the content of the transaction pool with minimum tip.
    #[method(name = "txPoolContentWithMinTip")]
    async fn tx_pool_content_with_min_tip(&self, min_tip: U256) -> RpcResult<serde_json::Value>;

    /// Sends a raw transaction.
    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, tx: Bytes) -> RpcResult<alloy_primitives::B256>;

    /// Sends multiple raw transactions.
    #[method(name = "sendRawTransactions")]
    async fn send_raw_transactions(&self, txs: Vec<Bytes>) -> RpcResult<Vec<alloy_primitives::B256>>;
}

/// The Kasplex Auth RPC extension implementation.
pub struct KasplexAuthExt<Provider, Pool, Eth, Evm>
where
    Provider: BlockReaderIdExt + ChainSpecProvider + Send + Sync + 'static,
    Pool: Send + Sync + 'static,
    Eth: Send + Sync + 'static,
    Evm: Send + Sync + 'static,
{
    provider: Provider,
    pool: Pool,
    tx_resp_builder: Eth,
    #[allow(dead_code)]
    evm_config: Evm,
}

impl<Provider, Pool, Eth, Evm> KasplexAuthExt<Provider, Pool, Eth, Evm>
where
    Provider: BlockReaderIdExt + ChainSpecProvider + Send + Sync + 'static,
    Pool: Send + Sync + 'static,
    Eth: Send + Sync + 'static,
    Evm: Send + Sync + 'static,
{
    /// Creates a new instance of `KasplexAuthExt` with the given components.
    pub fn new(provider: Provider, pool: Pool, tx_resp_builder: Eth, evm_config: Evm) -> Self {
        Self {
            provider,
            pool,
            tx_resp_builder,
            evm_config,
        }
    }
}

#[async_trait]
impl<Provider, Pool, Eth, Evm> KasplexAuthApiServerServer
    for KasplexAuthExt<Provider, Pool, Eth, Evm>
where
    Provider: BlockReaderIdExt + ChainSpecProvider + Send + Sync + 'static,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>> + Send + Sync + 'static,
    Eth: RpcConvert<Primitives = EthPrimitives, Error = EthApiError> + Clone + Send + Sync + 'static,
    Evm: ConfigureEvm<Primitives = EthPrimitives> + Clone + Send + Sync + 'static,
    EthApiError: FromEvmError<Evm>,
{
    /// Handler for: `kasplexAuth_txPoolContent`
    async fn tx_pool_content(&self) -> RpcResult<serde_json::Value> {
        trace!(target: "rpc::kasplex_auth", "Serving kasplexAuth_txPoolContent");
        
        // Get transaction pool content similar to txpool_content
        #[inline]
        fn insert<T: PoolTransaction, Rpc: RpcConvert>(
            tx: &T,
            tx_resp_builder: &Rpc,
            content: &mut BTreeMap<alloy_primitives::Address, BTreeMap<String, RpcTransaction<Rpc::Network>>>,
        ) -> Result<(), Rpc::Error>
        where 
            Rpc: RpcConvert<Primitives: reth_node_api::NodePrimitives<SignedTx = T::Consensus>>,
        {
            content.entry(tx.sender()).or_default().insert(
                tx.nonce().to_string(),
                tx_resp_builder.fill_pending(tx.clone_into_consensus())?,
            );
            Ok(())
        }

        let AllPoolTransactions { pending, queued } = self.pool.all_transactions();

        let mut content = TxpoolContent::<RpcTransaction<Eth::Network>>::default();
        for pending in pending {
            insert(&pending.transaction, &self.tx_resp_builder, &mut content.pending)?;
        }
        for queued in queued {
            insert(&queued.transaction, &self.tx_resp_builder, &mut content.queued)?;
        }

        Ok(serde_json::to_value(content).map_err(|e| ErrorObject::owned(1, format!("Failed to serialize: {}", e), None::<()>))?)
    }

    /// Handler for: `kasplexAuth_txPoolContentWithMinTip`
    async fn tx_pool_content_with_min_tip(&self, min_tip: U256) -> RpcResult<serde_json::Value> {
        trace!(target: "rpc::kasplex_auth", ?min_tip, "Serving kasplexAuth_txPoolContentWithMinTip");
        
        // Get transaction pool content filtered by minimum tip
        #[inline]
        fn insert_with_min_tip<T: PoolTransaction, Rpc: RpcConvert>(
            tx: &T,
            min_tip: U256,
            base_fee: u64,
            tx_resp_builder: &Rpc,
            content: &mut BTreeMap<alloy_primitives::Address, BTreeMap<String, RpcTransaction<Rpc::Network>>>,
        ) -> Result<bool, Rpc::Error>
        where 
            Rpc: RpcConvert<Primitives: reth_node_api::NodePrimitives<SignedTx = T::Consensus>>,
        {
            // Calculate effective tip: min(max_fee_per_gas - base_fee, max_priority_fee_per_gas)
            let max_fee_per_gas = U256::from(tx.max_fee_per_gas());
            let base_fee_u256 = U256::from(base_fee);
            let effective_tip = if max_fee_per_gas > base_fee_u256 {
                let fee_cap_tip = max_fee_per_gas - base_fee_u256;
                if let Some(priority_fee) = tx.max_priority_fee_per_gas() {
                    U256::from(priority_fee).min(fee_cap_tip)
                } else {
                    fee_cap_tip
                }
            } else {
                U256::ZERO
            };

            // Only include transactions with tip >= min_tip
            if effective_tip >= min_tip {
                content.entry(tx.sender()).or_default().insert(
                    tx.nonce().to_string(),
                    tx_resp_builder.fill_pending(tx.clone_into_consensus())?,
                );
                Ok(true)
            } else {
                Ok(false)
            }
        }

        // Get current base fee (fixed at 2000 GWei for Kasplex)
        let base_fee = 2_000_000_000_000u64; // Kasplex fixed base fee

        let AllPoolTransactions { pending, queued } = self.pool.all_transactions();

        let mut content = TxpoolContent::<RpcTransaction<Eth::Network>>::default();
        for pending in pending {
            insert_with_min_tip(&pending.transaction, min_tip, base_fee, &self.tx_resp_builder, &mut content.pending)?;
        }
        for queued in queued {
            insert_with_min_tip(&queued.transaction, min_tip, base_fee, &self.tx_resp_builder, &mut content.queued)?;
        }

        Ok(serde_json::to_value(content).map_err(|e| ErrorObject::owned(1, format!("Failed to serialize: {}", e), None::<()>))?)
    }

    /// Handler for: `kasplexAuth_sendRawTransaction`
    async fn send_raw_transaction(&self, tx: Bytes) -> RpcResult<alloy_primitives::B256> {
        trace!(target: "rpc::kasplex_auth", ?tx, "Serving kasplexAuth_sendRawTransaction");
        
        // Recover the transaction from raw bytes
        if tx.is_empty() {
            return Err(ErrorObject::owned(1, "Empty transaction data".to_string(), None::<()>));
        }

        // Decode and recover the transaction using the utility function
        let recovered: Recovered<TransactionSigned> = recover_raw_transaction(&tx)
            .map_err(|e| ErrorObject::owned(1, format!("Failed to decode transaction: {:?}", e), None::<()>))?;

        // For Kasplex: Validate base fee (gasFeeCap >= 2000 GWei)
        let min_l2_base_fee = U256::from(2_000_000_000_000u64); // 2000 GWei
        // Recovered derefs to TransactionSigned, access max_fee_per_gas through the Transaction trait
        use alloy_consensus::Transaction as TransactionTrait;
        let gas_fee_cap = U256::from(TransactionTrait::max_fee_per_gas(&*recovered));
        
        if gas_fee_cap < min_l2_base_fee {
            return Err(ErrorObject::owned(
                1,
                format!("max fee per gas ({}) is less than the minimum base fee (2000 GWei)", gas_fee_cap),
                None::<()>
            ));
        }
        
        // For Kasplex: Set transaction Number field if it's 0 or None
        // Get current block number and set transaction number to current_block + 1
        if let Ok(Some(header)) = self.provider.latest_header() {
            // SealedHeader<H> where H: alloy_consensus::BlockHeader + Sealable has number() method
            // But HeaderProvider::Header may not satisfy this constraint
            // Use header() to get &H, then access number through trait method if available
            // For now, we'll use a workaround: get the number from the header's number field
            // This requires H to be alloy_consensus::Header which has number field
            use alloy_consensus::BlockHeader;
            let tx_number = header.header().number() + 1;
            
            // Check if number needs to be set
            // Note: TransactionSigned doesn't have a number field in reth v1.9.3
            // The number is stored separately in TxMapping
            let tx_hash = recovered.hash();
            
            // Store transaction number in global TxMapping for later retrieval
            // This ensures the number can be extracted when building blocks
            kasplex_reth_tx_mapping::get_global_tx_mapping().insert(*tx_hash, tx_number);
            
            trace!(target: "rpc::kasplex_auth", tx_number, "Set transaction number");
        }

        // Convert to pool transaction and add to pool
        // Use try_from_consensus to convert Recovered<TransactionSigned> to pool transaction
        let pool_transaction = <Pool as TransactionPool>::Transaction::try_from_consensus(recovered)
            .map_err(|e| ErrorObject::owned(1, format!("Failed to convert transaction to pool format: {}", e), None::<()>))?;
        
        // Submit the transaction to the pool with a `Local` origin
        let outcome = self.pool
            .add_transaction(TransactionOrigin::Local, pool_transaction)
            .await
            .map_err(|e| ErrorObject::owned(1, format!("Failed to add transaction to pool: {:?}", e), None::<()>))?;

        Ok(outcome.hash)
    }

    /// Handler for: `kasplexAuth_sendRawTransactions`
    async fn send_raw_transactions(&self, txs: Vec<Bytes>) -> RpcResult<Vec<alloy_primitives::B256>> {
        trace!(target: "rpc::kasplex_auth", ?txs, "Serving kasplexAuth_sendRawTransactions");
        // Send multiple transactions and return their hashes
        let mut hashes = Vec::new();
        for tx in txs {
            match self.send_raw_transaction(tx).await {
                Ok(hash) => hashes.push(hash),
                Err(e) => {
                    tracing::warn!(target: "rpc::kasplex_auth", error = ?e, "Failed to send transaction in batch");
                    // Continue processing other transactions
                }
            }
        }
        Ok(hashes)
    }
}

impl<Provider, Pool, Eth, Evm> KasplexAuthExt<Provider, Pool, Eth, Evm>
where
    Provider: BlockReaderIdExt + ChainSpecProvider + Send + Sync + 'static,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>> + Send + Sync + 'static,
    Eth: RpcConvert<Primitives = EthPrimitives, Error = EthApiError> + Clone + Send + Sync + 'static,
    Evm: ConfigureEvm<Primitives = EthPrimitives> + Clone + Send + Sync + 'static,
    EthApiError: FromEvmError<Evm>,
{
    /// Converts this extension into an RPC module.
    pub fn into_rpc(self) -> jsonrpsee::RpcModule<Self> {
        jsonrpsee::RpcModule::new(self)
    }
}

