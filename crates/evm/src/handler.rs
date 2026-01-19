use reth::revm::{
    Database, Inspector,
    context::{
        Block, Cfg, ContextTr, JournalTr, Transaction,
        journaled_state::account::JournaledAccountTr,
        result::HaltReason,
    },
    handler::{
        EvmTr, EvmTrError, FrameResult, Handler, PrecompileProvider,
        instructions::InstructionProvider,
    },
    inspector::{InspectorEvmTr, InspectorHandler},
    interpreter::{Gas, InterpreterResult, interpreter::EthInterpreter},
    primitives::U256,
};
use reth_revm::{
    handler::{EthFrame, FrameTr, pre_execution::validate_account_nonce_and_code_with_components},
    interpreter::interpreter_action::FrameInit,
    state::EvmState,
};
use tracing::debug;

/// Handler for Kasplex EVM, it implements the `Handler` trait
/// and provides methods to handle the execution of transactions and the
/// reward for the beneficiary, including BaseFee distribution to Treasury.
#[derive(Default, Debug, Clone)]
pub struct KasplexEvmHandler<CTX, ERROR, FRAME> {
    pub _phantom: core::marker::PhantomData<(CTX, ERROR, FRAME)>,
}

impl<CTX, ERROR, FRAME> KasplexEvmHandler<CTX, ERROR, FRAME> {
    /// Creates a new instance of [`KasplexEvmHandler`].
    pub fn new() -> Self {
        Self { _phantom: core::marker::PhantomData }
    }
}

/// The implementation of Kasplex network transaction execution.
impl<EVM, ERROR, FRAME> Handler for KasplexEvmHandler<EVM, ERROR, FRAME>
where
    EVM: EvmTr<
            Context: ContextTr<Journal: JournalTr<State = EvmState>>,
            Precompiles: PrecompileProvider<EVM::Context, Output = InterpreterResult>,
            Instructions: InstructionProvider<
                Context = EVM::Context,
                InterpreterTypes = EthInterpreter,
            >,
            Frame = FRAME,
        >,
    ERROR: EvmTrError<EVM>,
    FRAME: FrameTr<FrameResult = FrameResult, FrameInit = FrameInit>,
{
    /// The EVM type containing Context, Instruction, and Precompiles implementations.
    type Evm = EVM;
    /// The error type returned by this handler.
    type Error = ERROR;
    /// The halt reason type included in the output
    type HaltReason = HaltReason;

    /// Transfers transaction fees to the block beneficiary's account, and distributes
    /// the base fee income to the network treasury.
    fn reward_beneficiary(
        &self,
        evm: &mut Self::Evm,
        exec_result: &mut FrameResult,
    ) -> Result<(), Self::Error> {
        reward_beneficiary(evm.ctx(), exec_result.gas_mut())
            .map_err(From::from)
    }

    #[inline]
    fn reimburse_caller(
        &self,
        evm: &mut Self::Evm,
        exec_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
    ) -> Result<(), Self::Error> {
        reimburse_caller(evm.ctx(), exec_result.gas(), U256::ZERO)
            .map_err(From::from)
    }

    /// Validates the transaction against the state and deducts the caller's balance.
    ///
    /// Loads the beneficiary account (EIP-3651: Warm COINBASE) and all accounts/storage from the
    /// access list (EIP-2929).
    ///
    /// Deducts the maximum possible fee from the caller's balance.
    fn validate_against_state_and_deduct_caller(
        &self,
        evm: &mut Self::Evm,
    ) -> Result<(), Self::Error> {
        let ctx = evm.ctx_mut();
        let (block, tx, cfg, journal, _, _) = ctx.all_mut();
        let caller = tx.caller();
        let beneficiary = block.beneficiary();
        let basefee = block.basefee() as u128;

        // Warm COINBASE (EIP-3651)
        journal.load_account(beneficiary)?;

        // Note: Access list loading is handled by the EVM framework automatically
        // We don't need to manually load accounts and storage from the access list here

        // Load caller's account info for validation first
        let caller_account_info = journal.load_account(caller)?.info.clone();

        // Load caller's account to validate and deduct balance
        let account_load_result = journal.load_account_with_code_mut(caller)?;
        let mut caller_account = account_load_result.data;

        // Validate account nonce and code
        validate_account_nonce_and_code_with_components(
            &caller_account_info,
            tx,
            cfg,
        )?;

        // Bump the nonce for calls. Nonce for CREATE will be bumped in `make_create_frame`.
        if tx.kind().is_call() {
            caller_account.bump_nonce();
        }

        // Deduct gas fee from caller's balance
        // Note: We use effective_balance_spending which accounts for actual gas price,
        // and subtract tx.value() because value will be deducted separately in the call.
        let effective_balance_spending = tx.effective_balance_spending(
            basefee,
            0u128, // blob_gasprice (not applicable for Kasplex)
        ).map_err(|e| ERROR::from(reth_revm::context::result::InvalidTransaction::from(e)))?;

        // Subtract value from effective_balance_spending because value is deducted separately in the call
        let gas_balance_spending = effective_balance_spending.saturating_sub(tx.value());

        let new_balance = caller_account.balance().saturating_sub(gas_balance_spending);
        caller_account.set_balance(new_balance);

        Ok(())
    }
}

/// Trait that extends [`Handler`] with inspection functionality, here we just use the default
/// implementation.
impl<EVM, ERROR> InspectorHandler for KasplexEvmHandler<EVM, ERROR, EthFrame<EthInterpreter>>
where
    EVM: InspectorEvmTr<
            Inspector: Inspector<<<Self as Handler>::Evm as EvmTr>::Context, EthInterpreter>,
            Context: ContextTr<Journal: JournalTr<State = EvmState>>,
            Precompiles: PrecompileProvider<EVM::Context, Output = InterpreterResult>,
            Instructions: InstructionProvider<
                Context = EVM::Context,
                InterpreterTypes = EthInterpreter,
            >,
            Frame = EthFrame<EthInterpreter>,
        >,
    ERROR: EvmTrError<EVM>,
{
    type IT = EthInterpreter;
}

/// Rewards the beneficiary and distributes base fee to treasury.
/// 
/// For Kasplex, all base fee is distributed to the Treasury address.
/// The beneficiary (coinbase) only receives the tip (effective_gas_price - base_fee).
#[inline]
fn reward_beneficiary<CTX: ContextTr>(
    context: &mut CTX,
    gas: &mut Gas,
) -> Result<(), <CTX::Db as Database>::Error> {
    let block_number = context.block().number();
    let beneficiary = context.block().beneficiary();
    let basefee = context.block().basefee() as u128;
    let effective_gas_price = context.tx().effective_gas_price(basefee);
    let coinbase_gas_price = effective_gas_price.saturating_sub(basefee);

    // Reward beneficiary with tip (effective_gas_price - base_fee)
    let spent_minus_refund = gas.spent().saturating_sub(gas.refunded() as u64);
    let tip_amount = coinbase_gas_price * spent_minus_refund as u128;
    let total_base_fee = U256::from(basefee.saturating_mul(spent_minus_refund as u128));

    context.journal_mut().balance_incr(
        beneficiary,
        U256::from(tip_amount),
    )?;

    // Distribute all base fee to Treasury address
    let chain_id = context.cfg().chain_id();
    let treasury_address = crate::get_treasury_address(chain_id);

    context.journal_mut().balance_incr(treasury_address, total_base_fee)?;

    debug!(
        target: "kasplex_evm",
        "Rewarded beneficiary: {} (tip: {}), distributed base fee to treasury: {} (amount: {}) at block: {}",
        beneficiary,
        tip_amount,
        treasury_address,
        total_base_fee,
        block_number
    );

    Ok(())
}

/// Reimburses the caller for unused gas.
#[inline]
pub fn reimburse_caller<CTX: ContextTr>(
    context: &mut CTX,
    gas: &Gas,
    additional_refund: U256,
) -> Result<(), <CTX::Db as Database>::Error> {
    let basefee = context.block().basefee() as u128;
    let caller = context.tx().caller();
    let effective_gas_price = context.tx().effective_gas_price(basefee);

    debug!(
        target: "kasplex_evm",
        "Reimbursing caller: {}, gas remaining: {}, gas refunded: {}, additional refund: {}",
        caller,
        gas.remaining(),
        gas.refunded(),
        additional_refund
    );

    // Return balance of not spent gas
    // Note: In geth, refund is added to gasRemaining before returnGas(), so returnGas() only
    // returns gasRemaining * gasPrice. In reth, gas.remaining() and gas.refunded() are separate,
    // so we need to return (remaining + refunded) * effective_gas_price to match geth's behavior.
    let remaining_gas = gas.remaining();
    let refunded_gas = gas.refunded() as u64;
    let total_to_refund = remaining_gas + refunded_gas;
    let refund_amount = U256::from(effective_gas_price.saturating_mul(total_to_refund as u128)) + additional_refund;

    context.journal_mut().balance_incr(caller, refund_amount)?;

    debug!(
        target: "kasplex_evm",
        "Reimbursed caller: {} (remaining: {}, refunded: {}, total: {}, amount: {})",
        caller,
        remaining_gas,
        refunded_gas,
        total_to_refund,
        refund_amount
    );

    Ok(())
}

