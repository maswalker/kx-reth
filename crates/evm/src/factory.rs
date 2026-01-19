use alloy_evm::{Database, EvmEnv, EvmFactory};
use reth::revm::{
    Context, Inspector, MainBuilder, MainContext,
    context::{
        BlockEnv, TxEnv,
        result::{EVMError, HaltReason},
    },
    inspector::NoOpInspector,
    interpreter::interpreter::EthInterpreter,
};
use reth_evm::precompiles::PrecompilesMap;
use reth_revm::precompile::{PrecompileSpecId, Precompiles};
use reth::revm::primitives::hardfork::SpecId;

use crate::{
    alloy::{KasplexEvmContext, KasplexEvmWrapper},
    evm::KasplexEvm,
};

/// A factory type for creating instances of the Kasplex EVM given a certain input.
#[derive(Default, Debug, Clone, Copy)]
pub struct KasplexEvmFactory;

impl EvmFactory for KasplexEvmFactory {
    /// The EVM type that this factory creates.
    type Evm<DB: Database + reth::revm::Database, I: Inspector<KasplexEvmContext<DB>, EthInterpreter>> =
        KasplexEvmWrapper<DB, I, Self::Precompiles>;
    /// Transaction environment.
    type Tx = TxEnv;
    /// EVM error.
    type Error<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    /// Halt reason.
    type HaltReason = HaltReason;
    /// The EVM context for inspectors.
    type Context<DB: Database + reth::revm::Database> = KasplexEvmContext<DB>;
    /// The EVM specification identifier
    type Spec = SpecId;
    /// Block environment used by the EVM.
    type BlockEnv = BlockEnv;
    /// Precompiles used by the EVM.
    type Precompiles = PrecompilesMap;

    /// Creates a new instance of an EVM.
    fn create_evm<DB: Database + reth::revm::Database>(
        &self,
        db: DB,
        input: EvmEnv<Self::Spec, Self::BlockEnv>,
    ) -> Self::Evm<DB, NoOpInspector> {
        let spec_id = input.cfg_env.spec;
        let evm = Context::mainnet()
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .with_db(db)
            .build_mainnet_with_inspector(NoOpInspector {})
            .with_precompiles(PrecompilesMap::from_static(Precompiles::new(
                PrecompileSpecId::from_spec_id(spec_id.into()),
            )));

        KasplexEvmWrapper::new(KasplexEvm::new(evm), false)
    }

    /// Creates a new instance of an EVM with an inspector.
    fn create_evm_with_inspector<DB: Database + reth::revm::Database, I: Inspector<Self::Context<DB>>>(
        &self,
        db: DB,
        input: EvmEnv<Self::Spec, Self::BlockEnv>,
        inspector: I,
    ) -> Self::Evm<DB, I> {
        let spec_id = input.cfg_env.spec;
        let evm = Context::mainnet()
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .with_db(db)
            .build_mainnet_with_inspector(NoOpInspector {})
            .with_precompiles(PrecompilesMap::from_static(Precompiles::new(
                PrecompileSpecId::from_spec_id(spec_id.into()),
            )))
            .with_inspector(inspector);

        KasplexEvmWrapper::new(KasplexEvm::new(evm), true)
    }
}

