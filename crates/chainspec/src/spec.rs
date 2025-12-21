use std::fmt::Display;
use std::sync::Arc;

use alloy_chains::Chain;
use alloy_consensus::Header;
use alloy_eips::eip7840::BlobParams;
use alloy_genesis::Genesis;
use alloy_hardforks::{
    EthereumHardfork, EthereumHardforks, ForkCondition, ForkFilter, ForkId, Hardfork, Head,
};
use alloy_primitives::{Address, B256, U256};
use reth::chainspec::{BaseFeeParams, ChainSpec, DepositContract, EthChainSpec, Hardforks};
use reth_evm::eth::spec::EthExecutorSpec;
use reth_network_peers::NodeRecord;

/// A Kasplex chain specification.
///
/// A chain specification describes:
///
/// - Meta-information about the chain (the chain ID)
/// - The genesis block of the chain ([`Genesis`])
/// - What hardforks are activated, and under which conditions
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KasplexChainSpec {
    pub inner: ChainSpec,
}

impl From<Genesis> for KasplexChainSpec {
    /// Converts the given [`Genesis`] into a [`KasplexChainSpec`].
    fn from(genesis: Genesis) -> Self {
        let chain_spec = ChainSpec::from(genesis);
        Self { inner: chain_spec }
    }
}

impl Hardforks for KasplexChainSpec {
    /// Retrieves [`ForkCondition`] from `fork`. If `fork` is not present, returns
    /// [`ForkCondition::Never`].
    fn fork<H: Hardfork>(&self, fork: H) -> ForkCondition {
        self.inner.hardforks.fork(fork)
    }

    /// Get an iterator of all hardforks with their respective activation conditions.
    fn forks_iter(&self) -> impl Iterator<Item = (&dyn Hardfork, ForkCondition)> {
        self.inner.hardforks.forks_iter()
    }

    /// Compute the [`ForkId`] for the given [`Head`] following eip-6122 spec
    fn fork_id(&self, head: &Head) -> ForkId {
        self.inner.fork_id(head)
    }

    /// Returns the [`ForkId`] for the last fork.
    ///
    /// NOTE: This returns the latest implemented [`ForkId`]. In many cases this will be the future
    /// [`ForkId`] on given network.
    fn latest_fork_id(&self) -> ForkId {
        self.inner.latest_fork_id()
    }

    /// Creates a [`ForkFilter`] for the block described by [Head].
    fn fork_filter(&self, head: Head) -> ForkFilter {
        self.inner.fork_filter(head)
    }
}

impl EthereumHardforks for KasplexChainSpec {
    /// Retrieves [`ForkCondition`] by an [`EthereumHardfork`]. If `fork` is not present, returns
    /// [`ForkCondition::Never`].
    fn ethereum_fork_activation(&self, fork: EthereumHardfork) -> ForkCondition {
        self.inner.fork(fork)
    }
}

impl EthExecutorSpec for KasplexChainSpec {
    /// Address of deposit contract emitting deposit events.
    ///
    /// In Kasplex network, the deposit contract is not used, so this method returns `None`.
    fn deposit_contract_address(&self) -> Option<Address> {
        None
    }
}

/// Trait for Kasplex executor specification.
/// 
/// This trait extends `EthExecutorSpec` with Kasplex-specific requirements.
pub trait KasplexExecutorSpec: EthExecutorSpec + Hardforks + EthereumHardforks + EthChainSpec {
    // No additional methods required for now
}

impl KasplexExecutorSpec for KasplexChainSpec {}

impl KasplexExecutorSpec for Arc<KasplexChainSpec> {}

impl<'a> KasplexExecutorSpec for &'a KasplexChainSpec {}

impl EthChainSpec for KasplexChainSpec {
    /// The header type of the network.
    type Header = Header;

    /// Returns the [`Chain`] object this spec targets.
    fn chain(&self) -> Chain {
        self.inner.chain
    }

    /// Get the [`BaseFeeParams`] for the chain at the given timestamp.
    fn base_fee_params_at_timestamp(&self, timestamp: u64) -> BaseFeeParams {
        self.inner.base_fee_params_at_timestamp(timestamp)
    }

    /// Get the [`BlobParams`] for the given timestamp, in Kasplex network this is always `None`.
    fn blob_params_at_timestamp(&self, _timestamp: u64) -> Option<BlobParams> {
        None
    }

    /// Returns the [`DepositContract`] for the chain, in Kasplex network this is always `None`.
    fn deposit_contract(&self) -> Option<&DepositContract> {
        None
    }

    /// The genesis hash.
    fn genesis_hash(&self) -> B256 {
        self.inner.genesis_hash()
    }

    /// The delete limit for pruner, per run.
    fn prune_delete_limit(&self) -> usize {
        self.inner.prune_delete_limit
    }

    /// Returns a string representation of the hardforks.
    fn display_hardforks(&self) -> Box<dyn Display> {
        Box::new(self.inner.display_hardforks())
    }

    /// The genesis header.
    fn genesis_header(&self) -> &Self::Header {
        self.inner.genesis_header()
    }

    /// The genesis block specification.
    fn genesis(&self) -> &Genesis {
        self.inner.genesis()
    }

    /// The bootnodes for the chain, if any.
    fn bootnodes(&self) -> Option<Vec<NodeRecord>> {
        self.inner.bootnodes()
    }

    /// In Kasplex network, we always mark this value as `true` so that we
    /// can reorg the chain at will.
    fn is_optimism(&self) -> bool {
        true
    }

    /// Returns the block number at which the Paris hardfork is activated.
    /// In Kasplex network, this is always `0`.
    fn final_paris_total_difficulty(&self) -> Option<U256> {
        Some(U256::ZERO)
    }
}

/// Helper trait to check if a chain is a Kasplex chain
pub trait KasplexChainCheck {
    /// Returns `true` if this is a Kasplex chain.
    fn is_kasplex(&self) -> bool;
}

impl KasplexChainCheck for KasplexChainSpec {
    fn is_kasplex(&self) -> bool {
        matches!(
            self.inner.chain.id(),
            crate::KASPLEX_MAINNET_CHAIN_ID
                | crate::KASPLEX_INTERNAL_L2_CHAIN_ID
                | crate::KASPLEX_TESTNET_CHAIN_ID
                | crate::KASPLEX_DEVNET_CHAIN_ID
        )
    }
}


