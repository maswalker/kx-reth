use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use reth_provider::{BlockReaderIdExt, ChainSpecProvider};
use tracing::trace;

/// Kasplex RPC API server trait
#[rpc(server, namespace = "kasplex")]
pub trait KasplexApiServer {
    /// Returns the sync mode of the node.
    #[method(name = "getSyncMode")]
    async fn get_sync_mode(&self) -> RpcResult<String>;
}

/// The Kasplex RPC extension implementation.
pub struct KasplexExt<Provider>
where
    Provider: BlockReaderIdExt + ChainSpecProvider,
{
    #[allow(dead_code)]
    provider: Provider,
}

impl<Provider> KasplexExt<Provider>
where
    Provider: BlockReaderIdExt + ChainSpecProvider,
{
    /// Creates a new instance of `KasplexExt` with the given provider.
    pub fn new(provider: Provider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl<Provider> KasplexApiServerServer for KasplexExt<Provider>
where
    Provider: BlockReaderIdExt + ChainSpecProvider + 'static,
{
    /// Handler for: `kasplex_getSyncMode`
    async fn get_sync_mode(&self) -> RpcResult<String> {
        trace!(target: "rpc::kasplex", "Serving kasplex_getSyncMode");
        // Check if the node is syncing by checking if latest block is recent
        // For now, we return "full" as the default sync mode
        // In the future, this could be determined by checking the actual sync mode
        Ok("full".to_string())
    }
}

impl<Provider> KasplexExt<Provider>
where
    Provider: BlockReaderIdExt + ChainSpecProvider + 'static,
{
    /// Converts this extension into an RPC module.
    pub fn into_rpc(self) -> jsonrpsee::RpcModule<Self> {
        jsonrpsee::RpcModule::new(self)
    }
}
