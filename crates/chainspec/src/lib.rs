use std::sync::{Arc, LazyLock};

use alloy_primitives::B256;
use reth::{
    chainspec::{ChainSpec, make_genesis_header},
    primitives::SealedHeader,
    revm::primitives::b256,
};
use reth_ethereum_forks::ChainHardforks;

use crate::{
    hardfork::KASPLEX_HARDFORKS,
    spec::KasplexChainSpec,
};

pub mod hardfork;
pub mod parser;
pub mod spec;

/// Kasplex network chain IDs
pub const KASPLEX_MAINNET_CHAIN_ID: u64 = 202555;
pub const KASPLEX_INTERNAL_L2_CHAIN_ID: u64 = 168001;
pub const KASPLEX_TESTNET_CHAIN_ID: u64 = 168002;
pub const KASPLEX_DEVNET_CHAIN_ID: u64 = 167012;

/// Genesis hash for the Kasplex Mainnet network.
/// This will be computed from the genesis JSON at runtime.
/// For now, we use a placeholder that will be validated against the computed hash.
pub const KASPLEX_MAINNET_GENESIS_HASH: B256 =
    b256!("0x0000000000000000000000000000000000000000000000000000000000000000");

/// Genesis hash for the Kasplex Internal L2 network.
pub const KASPLEX_INTERNAL_L2_GENESIS_HASH: B256 =
    b256!("0x0000000000000000000000000000000000000000000000000000000000000000");

/// Genesis hash for the Kasplex Testnet network.
pub const KASPLEX_TESTNET_GENESIS_HASH: B256 =
    b256!("0x0000000000000000000000000000000000000000000000000000000000000000");

/// Genesis hash for the Kasplex Devnet network.
pub const KASPLEX_DEVNET_GENESIS_HASH: B256 =
    b256!("0x0000000000000000000000000000000000000000000000000000000000000000");

/// The Kasplex Mainnet spec
pub static KASPLEX_MAINNET: LazyLock<Arc<KasplexChainSpec>> =
    LazyLock::new(|| make_kasplex_mainnet_chain_spec().into());

/// The Kasplex Internal L2 spec
pub static KASPLEX_INTERNAL_L2: LazyLock<Arc<KasplexChainSpec>> =
    LazyLock::new(|| make_kasplex_internal_l2_chain_spec().into());

/// The Kasplex Testnet spec
pub static KASPLEX_TESTNET: LazyLock<Arc<KasplexChainSpec>> =
    LazyLock::new(|| make_kasplex_testnet_chain_spec().into());

/// The Kasplex Devnet spec
pub static KASPLEX_DEVNET: LazyLock<Arc<KasplexChainSpec>> =
    LazyLock::new(|| make_kasplex_devnet_chain_spec().into());

// Creates a new [`ChainSpec`] for the Kasplex Mainnet network.
fn make_kasplex_mainnet_chain_spec() -> KasplexChainSpec {
    make_kasplex_chain_spec(
        include_str!("../res/genesis/kasplex-mainnet.json"),
        KASPLEX_MAINNET_GENESIS_HASH,
        KASPLEX_HARDFORKS.clone(),
    )
}

// Creates a new [`ChainSpec`] for the Kasplex Internal L2 network.
fn make_kasplex_internal_l2_chain_spec() -> KasplexChainSpec {
    // TODO: Add genesis JSON file for Internal L2 when available
    // For now, create a minimal genesis
    make_kasplex_chain_spec_minimal(
        KASPLEX_INTERNAL_L2_CHAIN_ID,
        KASPLEX_INTERNAL_L2_GENESIS_HASH,
        KASPLEX_HARDFORKS.clone(),
    )
}

// Creates a new [`ChainSpec`] for the Kasplex Testnet network.
fn make_kasplex_testnet_chain_spec() -> KasplexChainSpec {
    // TODO: Add genesis JSON file for Testnet when available
    // For now, create a minimal genesis
    make_kasplex_chain_spec_minimal(
        KASPLEX_TESTNET_CHAIN_ID,
        KASPLEX_TESTNET_GENESIS_HASH,
        KASPLEX_HARDFORKS.clone(),
    )
}

// Creates a new [`ChainSpec`] for the Kasplex Devnet network.
fn make_kasplex_devnet_chain_spec() -> KasplexChainSpec {
    // TODO: Add genesis JSON file for Devnet when available
    // For now, create a minimal genesis
    make_kasplex_chain_spec_minimal(
        KASPLEX_DEVNET_CHAIN_ID,
        KASPLEX_DEVNET_GENESIS_HASH,
        KASPLEX_HARDFORKS.clone(),
    )
}

// Creates a minimal [`ChainSpec`] for a Kasplex network (used when genesis JSON is not available).
fn make_kasplex_chain_spec_minimal(
    chain_id: u64,
    genesis_hash: B256,
    hardforks: ChainHardforks,
) -> KasplexChainSpec {
    use alloy_genesis::Genesis;
    use alloy_primitives::U256;

    // Create a minimal genesis configuration for Kasplex
    let genesis = Genesis {
        config: alloy_genesis::ChainConfig {
            chain_id: chain_id.into(),
            ..Default::default()
        },
        extra_data: Default::default(),
        gas_limit: 15_000_000,
        base_fee_per_gas: Some(2_000_000_000_000u128), // 2000 GWei
        difficulty: U256::ZERO, // PoS
        timestamp: 0,
        nonce: 0,
        mix_hash: Default::default(),
        alloc: Default::default(),
        number: Some(0),
        parent_hash: Default::default(),
        coinbase: alloy_primitives::Address::ZERO,
        blob_gas_used: None,
        excess_blob_gas: None,
    };

    let genesis_header = SealedHeader::new(
        make_genesis_header(&genesis, &hardforks),
        genesis_hash,
    );

    let inner = ChainSpec {
        chain: chain_id.into(),
        genesis_header,
        genesis,
        paris_block_and_final_difficulty: Some((0, U256::from(0))),
        hardforks,
        prune_delete_limit: 10000,
        ..Default::default()
    };

    KasplexChainSpec { inner }
}

// Creates a new [`ChainSpec`] for a Kasplex network.
fn make_kasplex_chain_spec(
    genesis_json: &str,
    genesis_hash: B256,
    hardforks: ChainHardforks,
) -> KasplexChainSpec {
    use alloy_genesis::Genesis;
    use alloy_primitives::U256;

    // Deserialize genesis from JSON
    let genesis: Genesis = serde_json::from_str(genesis_json)
        .expect("Can't deserialize Kasplex genesis json");

    // Create genesis header and compute the actual hash
    let genesis_header_unsealed = make_genesis_header(&genesis, &hardforks);
    let computed_hash = genesis_header_unsealed.hash_slow();
    
    // Use the computed hash if the provided hash is zero (placeholder)
    let final_hash = if genesis_hash == B256::ZERO {
        computed_hash
    } else {
        // Validate that the provided hash matches the computed hash
        if computed_hash != genesis_hash {
            panic!("Genesis hash mismatch: expected {:?}, computed {:?}", genesis_hash, computed_hash);
        }
        genesis_hash
    };

    let genesis_header = SealedHeader::new(genesis_header_unsealed, final_hash);

    let inner = ChainSpec {
        chain: genesis.config.chain_id.into(),
        genesis_header,
        genesis,
        paris_block_and_final_difficulty: Some((0, U256::from(0))),
        hardforks,
        prune_delete_limit: 10000,
        ..Default::default()
    };

    KasplexChainSpec { inner }
}

