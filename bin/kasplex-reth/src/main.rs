//! Rust Kasplex node (kasplex-reth) binary executable.
use kasplex_reth_cli::{KasplexCli, KasplexCliExtArgs};
use kasplex_reth_node::{
    KasplexNode,
    chainspec::parser::KasplexChainSpecParser,
};
use kasplex_reth_rpc::eth::{
    auth::KasplexAuthExt,
    eth::KasplexExt,
};
use reth::{builder::NodeHandle, ress::install_ress_subprotocol};
use reth_rpc::eth::{EthApiTypes, RpcNodeCore};
use tracing::info;

#[global_allocator]
static ALLOC: reth_cli_util::allocator::Allocator = reth_cli_util::allocator::new_allocator();

fn main() {
    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    }

    if let Err(err) = KasplexCli::<KasplexChainSpecParser, KasplexCliExtArgs>::parse_args().run(
        async move |builder, ext_args| {
            info!(target: "reth::kasplex::cli", "Launching Kasplex node");
            let NodeHandle { node, node_exit_future } = builder
                .node(KasplexNode)
                .extend_rpc_modules(move |ctx| {
                    let provider = ctx.node().provider().clone();

                    // Extend the RPC modules with `kasplex` namespace RPCs extensions.
                    let kasplex_rpc_ext = KasplexExt::new(provider.clone());
                    ctx.modules.merge_configured(kasplex_rpc_ext.into_rpc())?;

                    // Extend the RPC modules with `kasplexAuth` namespace RPCs extensions.
                    let kasplex_auth_rpc_ext = KasplexAuthExt::new(
                        provider,
                        ctx.node().pool().clone(),
                        ctx.registry.eth_api().tx_resp_builder().clone(),
                        ctx.node().evm_config().clone(),
                    );
                    // Convert RpcModule<Arc<Self>> to Methods for merge_auth_methods
                    let auth_rpc_module = kasplex_auth_rpc_ext.into_rpc();
                    // Log registered methods for debugging
                    let method_names: Vec<String> = auth_rpc_module.method_names().map(|s: &str| s.to_string()).collect();
                    info!(target: "reth::kasplex::cli", "Registering kasplexAuth methods to auth_module: {:?}", method_names);
                    // merge_auth_methods accepts impl Into<Methods>
                    // RpcModule should implement Into<Methods> automatically
                    ctx.auth_module.merge_auth_methods(auth_rpc_module)?;

                    Ok(())
                })
                .launch_with_debug_capabilities()
                .await?;

            // Install ress subprotocol if enabled.
            if ext_args.ress.enabled {
                let provider = node.provider.clone();
                let evm_config = node.evm_config.clone();
                let network = node.network.clone();
                let task_executor = node.task_executor.clone();
                let engine_events = node.add_ons_handle.engine_events.new_listener();
                install_ress_subprotocol(
                    ext_args.ress,
                    provider,
                    evm_config,
                    network,
                    task_executor,
                    engine_events,
                )?;
            }

            // Keep the node handle alive to prevent RPC servers from being dropped
            // The RpcServerHandle has #[must_use = "Server stops if dropped"]
            let _node = node;

            node_exit_future.await
        },
    ) {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

