pub mod alloy;
#[allow(clippy::module_inception)]
pub mod evm;
pub mod execution;
pub mod factory;
pub mod handler;

pub use factory::KasplexEvmFactory;
pub use handler::KasplexEvmHandler;
pub use alloy::{KasplexEvmContext, KasplexEvmWrapper};
pub use evm::KasplexEvm;

use alloy_primitives::Address;
use std::str::FromStr;
use kasplex_reth_chainspec::spec::KasplexChainSpec;

/// Kasplex fixed base fee: 2000 GWei
pub const KASPLEX_BASE_FEE: u64 = 2_000_000_000_000u64;

/// Calculate the treasury address for a Kasplex chain based on chain ID.
/// 
/// Treasury address format: `0x{ChainID}{padding}{10001}`
/// 
/// Example for chain ID 202555:
/// - Chain ID: 202555 (hex: 0x3164b)
/// - Padding: zeros to fill to 40 hex characters
/// - Suffix: 10001
/// - Result: 0x000000000000000000000000000000000003164b10001
pub fn get_treasury_address(chain_id: u64) -> Address {
    let chain_id_str = format!("{:x}", chain_id);
    let suffix = "10001";
    let address_length: usize = 40; // 20 bytes = 40 hex characters
    
    // Calculate padding needed
    let padding_needed = address_length.saturating_sub(chain_id_str.len() + suffix.len());
    let padding = "0".repeat(padding_needed);
    
    // Construct address: 0x + chain_id + padding + suffix
    let address_hex = format!("{}{}{}", chain_id_str, padding, suffix);
    Address::from_str(&format!("0x{}", address_hex))
        .unwrap_or_else(|_| Address::ZERO)
}

/// Calculate the treasury address from a chain spec.
pub fn get_treasury_address_from_spec(chain_spec: &KasplexChainSpec) -> Address {
    get_treasury_address(chain_spec.inner.chain.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_treasury_address_calculation() {
        // Test for Kasplex Mainnet (chain ID 202555)
        let mainnet_treasury = get_treasury_address(202555);
        // The address should end with the chain ID and 10001
        assert!(mainnet_treasury != Address::ZERO);
        
        // Test for Kasplex Internal L2 (chain ID 168001)
        let internal_l2_treasury = get_treasury_address(168001);
        assert!(internal_l2_treasury != Address::ZERO);
        assert_ne!(mainnet_treasury, internal_l2_treasury);
    }
}
