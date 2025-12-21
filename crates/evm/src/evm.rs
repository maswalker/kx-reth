use reth::revm::{
    context::{ContextTr, Evm as RevmEvm},
    handler::{EvmTr, instructions::EthInstructions},
    interpreter::interpreter::EthInterpreter,
};
use reth_revm::{
    context::{ContextError, FrameStack},
    handler::{EthFrame, FrameInitOrResult, FrameTr, ItemOrResult, PrecompileProvider},
    interpreter::InterpreterResult,
};
use revm_database_interface::Database;

/// Custom EVM for Kasplex, we extend the RevmEvm to use KasplexEvmHandler
/// for BaseFee distribution to Treasury address.
pub struct KasplexEvm<CTX, INSP, P> {
    pub inner:
        RevmEvm<CTX, INSP, EthInstructions<EthInterpreter, CTX>, P, EthFrame<EthInterpreter>>,
}

impl<CTX: ContextTr, INSP, P> KasplexEvm<CTX, INSP, P> {
    /// Creates a new instance of [`KasplexEvm`].
    pub fn new(
        inner: RevmEvm<
            CTX,
            INSP,
            EthInstructions<EthInterpreter, CTX>,
            P,
            EthFrame<EthInterpreter>,
        >,
    ) -> Self {
        Self { inner }
    }
}

/// A trait that integrates context, instruction set, and precompiles to create an EVM struct,
/// here we use the same implementation as `RevmEvm`.
impl<CTX: ContextTr, INSP, P> EvmTr for KasplexEvm<CTX, INSP, P>
where
    CTX: ContextTr,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    /// The context type that implements ContextTr to provide access to execution state
    type Context = CTX;
    /// The instruction set type that implements InstructionProvider to define available operations
    type Instructions = EthInstructions<EthInterpreter, CTX>;
    /// The type containing the available precompiled contracts
    type Precompiles = P;
    type Frame = EthFrame<EthInterpreter>;

    /// Returns shared references to context, instructions, precompiles, and frame stack.
    fn all(
        &self,
    ) -> (&Self::Context, &Self::Instructions, &Self::Precompiles, &FrameStack<Self::Frame>) {
        self.inner.all()
    }

    /// Returns mutable references to context, instructions, precompiles, and frame stack.
    fn all_mut(
        &mut self,
    ) -> (
        &mut Self::Context,
        &mut Self::Instructions,
        &mut Self::Precompiles,
        &mut FrameStack<Self::Frame>,
    ) {
        self.inner.all_mut()
    }

    /// Returns a mutable reference to the execution context
    fn ctx(&mut self) -> &mut Self::Context {
        &mut self.inner.ctx
    }

    /// Returns an immutable reference to the execution context
    fn ctx_ref(&self) -> &Self::Context {
        self.inner.ctx_ref()
    }

    /// Returns mutable references to both the context and instruction set.
    /// This enables atomic access to both components when needed.
    fn ctx_instructions(&mut self) -> (&mut Self::Context, &mut Self::Instructions) {
        self.inner.ctx_instructions()
    }

    /// Returns a mutable reference to the frame stack.
    fn frame_stack(&mut self) -> &mut FrameStack<Self::Frame> {
        self.inner.frame_stack()
    }

    /// Initializes the frame for the given frame input. Frame is pushed to the frame stack.
    fn frame_init(
        &mut self,
        frame_input: <Self::Frame as FrameTr>::FrameInit,
    ) -> Result<
        ItemOrResult<&mut Self::Frame, <Self::Frame as FrameTr>::FrameResult>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.inner.frame_init(frame_input)
    }

    /// Run the frame from the top of the stack. Returns the frame init or result.
    ///
    /// If frame has returned result it would mark it as finished.
    fn frame_run(
        &mut self,
    ) -> Result<
        FrameInitOrResult<Self::Frame>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.inner.frame_run()
    }

    /// Returns the result of the frame to the caller. Frame is popped from the frame stack.
    /// Consumes the frame result or returns it if there is more frames to run.
    fn frame_return_result(
        &mut self,
        frame_result: <Self::Frame as FrameTr>::FrameResult,
    ) -> Result<
        Option<<Self::Frame as FrameTr>::FrameResult>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.inner.frame_return_result(frame_result)
    }

    /// Returns mutable references to both the context and precompiles.
    /// This enables atomic access to both components when needed.
    fn ctx_precompiles(&mut self) -> (&mut Self::Context, &mut Self::Precompiles) {
        self.inner.ctx_precompiles()
    }
}



