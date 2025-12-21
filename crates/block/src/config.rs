use std::sync::Arc;
use kasplex_reth_chainspec::spec::KasplexChainSpec;
use kasplex_reth_evm::KasplexEvmFactory;
use reth_evm::{ConfigureEvm, ConfigureEngineEvm};
use reth_evm_ethereum::{EthEvmConfig, RethReceiptBuilder};
use reth::primitives::{BlockTy, Header, SealedBlock, SealedHeader};
use std::borrow::Cow;
use alloy_rpc_types_eth::Withdrawals;

use crate::{assembler::KasplexBlockAssembler, factory::KasplexBlockExecutorFactory};

/// Kasplex EVM configuration.
#[derive(Debug, Clone)]
pub struct KasplexEvmConfig {
    /// Block executor factory for Kasplex.
    pub executor_factory: KasplexBlockExecutorFactory<RethReceiptBuilder, Arc<KasplexChainSpec>, KasplexEvmFactory>,
    /// Block assembler for Kasplex.
    pub block_assembler: KasplexBlockAssembler,
}

impl KasplexEvmConfig {
    /// Creates a new Kasplex EVM configuration with the given chain spec.
    pub fn new(chain_spec: Arc<KasplexChainSpec>) -> Self {
        Self::new_with_evm_factory(chain_spec, KasplexEvmFactory)
    }

    /// Creates a new Kasplex EVM configuration with the given chain spec and EVM factory.
    pub fn new_with_evm_factory(
        chain_spec: Arc<KasplexChainSpec>,
        evm_factory: KasplexEvmFactory,
    ) -> Self {
        Self {
            block_assembler: KasplexBlockAssembler::new(chain_spec.clone()),
            executor_factory: KasplexBlockExecutorFactory::new(
                RethReceiptBuilder::default(),
                chain_spec,
                evm_factory,
            ),
        }
    }

    /// Returns the chain spec associated with this configuration.
    pub const fn chain_spec(&self) -> &Arc<KasplexChainSpec> {
        self.executor_factory.spec()
    }
}

impl ConfigureEvm for KasplexEvmConfig {
    type Primitives = reth_ethereum::EthPrimitives;
    type Error = std::convert::Infallible;
    type NextBlockEnvCtx = reth_evm::NextBlockEnvAttributes;
    type BlockExecutorFactory = KasplexBlockExecutorFactory<RethReceiptBuilder, Arc<KasplexChainSpec>, KasplexEvmFactory>;
    type BlockAssembler = KasplexBlockAssembler;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        &self.executor_factory
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        &self.block_assembler
    }


    fn evm_env(&self, header: &reth::primitives::Header) -> Result<reth_evm::EvmEnvFor<Self>, Self::Error> {
        // Use the inner EthEvmConfig to get the EVM environment
        let inner = EthEvmConfig::new(self.chain_spec().clone());
        inner.evm_env(header)
    }

    fn next_evm_env(
        &self,
        parent: &Header,
        attributes: &Self::NextBlockEnvCtx,
    ) -> Result<reth_evm::EvmEnvFor<Self>, Self::Error> {
        // Use the inner EthEvmConfig to get the next EVM environment
        let inner = EthEvmConfig::new(self.chain_spec().clone());
        inner.next_evm_env(parent, attributes)
    }

    fn context_for_block<'a>(
        &self,
        block: &'a SealedBlock<BlockTy<Self::Primitives>>,
    ) -> Result<reth_evm::ExecutionCtxFor<'a, Self>, Self::Error> {
        use crate::factory::KasplexBlockExecutionCtx;
        Ok(KasplexBlockExecutionCtx {
            parent_hash: block.header().parent_hash,
            parent_beacon_block_root: block.header().parent_beacon_block_root,
            ommers: &[],
            withdrawals: Some(Cow::Owned(Withdrawals::new(vec![]))),
            basefee_per_gas: block.header().base_fee_per_gas.unwrap_or_default(),
            extra_data: block.header().extra_data.clone(),
        })
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader,
        _ctx: Self::NextBlockEnvCtx,
    ) -> Result<reth_evm::ExecutionCtxFor<'_, Self>, Self::Error> {
        use crate::factory::KasplexBlockExecutionCtx;
        Ok(KasplexBlockExecutionCtx {
            parent_hash: parent.hash(),
            parent_beacon_block_root: None,
            ommers: &[],
            withdrawals: Some(Cow::Owned(Withdrawals::new(vec![]))),
            basefee_per_gas: parent.base_fee_per_gas.unwrap_or_default(),
            extra_data: parent.extra_data.clone(),
        })
    }
}

impl ConfigureEngineEvm<alloy_rpc_types_engine::ExecutionData> for KasplexEvmConfig {
    fn evm_env_for_payload(
        &self,
        payload: &alloy_rpc_types_engine::ExecutionData,
    ) -> Result<reth_evm::EvmEnvFor<Self>, Self::Error> {
        // Use the inner EthEvmConfig to get the EVM environment
        let inner = EthEvmConfig::new(self.chain_spec().clone());
        inner.evm_env_for_payload(payload)
    }

    fn context_for_payload<'a>(
        &self,
        payload: &'a alloy_rpc_types_engine::ExecutionData,
    ) -> Result<reth_evm::ExecutionCtxFor<'a, Self>, Self::Error> {
        use crate::factory::KasplexBlockExecutionCtx;
        use alloy_rpc_types_engine::ExecutionPayload;
        // Extract context from payload - use trait methods where available
        let parent_hash = payload.payload.parent_hash();
        // For Kasplex, we primarily use V1 payloads
        // Extract fields based on payload variant
        let (parent_beacon_block_root, basefee_per_gas, extra_data) = match &payload.payload {
            ExecutionPayload::V1(p) => (
                None, // V1 doesn't have parent_beacon_block_root in payload
                p.base_fee_per_gas.to::<u64>(),
                p.extra_data.clone(),
            ),
            ExecutionPayload::V2(_) | ExecutionPayload::V3(_) => {
                // For V2/V3, use trait methods or default values
                // These variants have nested structures that are more complex
                // For now, use defaults - full implementation would need to handle nested payload_inner
                (None, 0, Default::default())
            }
        };
        Ok(KasplexBlockExecutionCtx {
            parent_hash,
            parent_beacon_block_root,
            ommers: &[],
            withdrawals: None, // Withdrawals are in sidecar, but we'll handle them separately if needed
            basefee_per_gas,
            extra_data,
        })
    }

    fn tx_iterator_for_payload(
        &self,
        payload: &alloy_rpc_types_engine::ExecutionData,
    ) -> Result<impl reth_evm::ExecutableTxIterator<Self>, Self::Error> {
        use reth::primitives::TransactionSigned;
        use reth_primitives_traits::SignedTransaction;
        use reth_provider::errors::any::AnyError;
        use alloy_eips::eip2718::Decodable2718;
        
        Ok(payload.payload.transactions().clone().into_iter().map(
            |tx| {
                let tx = TransactionSigned::decode_2718_exact(tx.as_ref())
                    .map_err(AnyError::new)?;
                let signer = SignedTransaction::try_recover(&tx).map_err(AnyError::new)?;
                Ok::<_, AnyError>(tx.with_signer(signer))
            },
        ))
    }
}

