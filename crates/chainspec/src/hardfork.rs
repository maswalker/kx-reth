use std::sync::LazyLock;
use alloy_hardforks::{EthereumHardfork, ForkCondition, Hardfork};
use reth::{chainspec::ChainHardforks, revm::primitives::U256};

/// Kasplex hardforks - all activated at block 0
/// 
/// Kasplex networks use all Ethereum hardforks activated at genesis (block 0).
/// Note: We exclude MuirGlacier, ArrowGlacier, and GrayGlacier to match Geth's fork ID calculation.
/// These hardforks are not recognized by Geth and would cause fork ID mismatch.
pub static KASPLEX_HARDFORKS: LazyLock<ChainHardforks> = LazyLock::new(|| {
    ChainHardforks::new(vec![
        (EthereumHardfork::Frontier.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Homestead.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Tangerine.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::SpuriousDragon.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Byzantium.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Constantinople.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Petersburg.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::Istanbul.boxed(), ForkCondition::Block(0)),
        // Note: MuirGlacier, ArrowGlacier, GrayGlacier are excluded to match Geth
        (EthereumHardfork::Berlin.boxed(), ForkCondition::Block(0)),
        (EthereumHardfork::London.boxed(), ForkCondition::Block(0)),
        (
            EthereumHardfork::Paris.boxed(),
            ForkCondition::TTD {
                activation_block_number: 0,
                fork_block: Some(0),
                total_difficulty: U256::ZERO,
            },
        ),
        (EthereumHardfork::Shanghai.boxed(), ForkCondition::Timestamp(0)),
    ])
});


