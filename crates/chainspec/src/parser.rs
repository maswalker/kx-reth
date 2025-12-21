use std::sync::Arc;
use reth_cli::chainspec::{ChainSpecParser, parse_genesis};

use crate::{
    spec::KasplexChainSpec,
    KASPLEX_DEVNET, KASPLEX_INTERNAL_L2, KASPLEX_MAINNET, KASPLEX_TESTNET,
};

/// Chains supported by kasplex-reth. First value should be used as the default.
pub const SUPPORTED_CHAINS: &[&str] = &["kasplex-mainnet", "kasplex-internal-l2", "kasplex-testnet", "kasplex-devnet"];

/// Clap value parser for [`KasplexChainSpec`]s.
///
/// The value parser matches either a known chain, the path
/// to a json file, or a json formatted string in-memory. The json needs to be a Genesis struct.
pub fn chain_value_parser(s: &str) -> eyre::Result<Arc<KasplexChainSpec>> {
    Ok(match s {
        "kasplex-mainnet" | "kasplex_mainnet" => KASPLEX_MAINNET.clone(),
        "kasplex-internal-l2" | "kasplex_internal_l2" => KASPLEX_INTERNAL_L2.clone(),
        "kasplex-testnet" | "kasplex_testnet" => KASPLEX_TESTNET.clone(),
        "kasplex-devnet" | "kasplex_devnet" => KASPLEX_DEVNET.clone(),
        _ => Arc::new(parse_genesis(s)?.into()),
    })
}

/// Kasplex chain specification parser.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct KasplexChainSpecParser;

impl ChainSpecParser for KasplexChainSpecParser {
    /// The chain specification type.
    type ChainSpec = KasplexChainSpec;

    /// List of supported chains.
    const SUPPORTED_CHAINS: &'static [&'static str] = SUPPORTED_CHAINS;

    /// Parses the given string into a chain spec.
    ///
    /// # Arguments
    ///
    /// * `s` - A string slice that holds the chain spec to be parsed.
    ///
    /// # Errors
    ///
    /// This function will return an error if the input string cannot be parsed into a valid
    /// chain spec.
    fn parse(s: &str) -> eyre::Result<Arc<KasplexChainSpec>> {
        chain_value_parser(s)
    }
}


