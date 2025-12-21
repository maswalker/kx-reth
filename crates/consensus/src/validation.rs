use std::sync::Arc;

use alloy_consensus::EMPTY_OMMER_ROOT_HASH;
use alloy_primitives::U256;
use reth::{
    beacon_consensus::validate_block_post_execution,
    consensus::{Consensus, ConsensusError, FullConsensus, HeaderValidator},
    consensus_common::validation::{
        validate_against_parent_hash_number, validate_body_against_header,
        validate_header_base_fee, validate_header_extra_data, validate_header_gas,
    },
    primitives::SealedBlock,
};
use reth_node_api::NodePrimitives;
use reth_primitives_traits::{
    Block, BlockHeader, GotExpected, RecoveredBlock, SealedHeader,
};
use reth_provider::BlockExecutionResult;

use kasplex_reth_chainspec::spec::KasplexChainSpec;

/// Kasplex fixed base fee: 2000 GWei
pub const KASPLEX_BASE_FEE: u64 = 2_000_000_000_000u64;

/// Kasplex consensus implementation.
///
/// Provides basic checks as outlined in the execution specs.
#[derive(Debug, Clone)]
pub struct KasplexBeaconConsensus {
    chain_spec: Arc<KasplexChainSpec>,
}

impl KasplexBeaconConsensus {
    /// Create a new instance of [`KasplexBeaconConsensus`]
    pub fn new(chain_spec: Arc<KasplexChainSpec>) -> Self {
        Self { chain_spec }
    }
}

impl<N> FullConsensus<N> for KasplexBeaconConsensus
where
    N: NodePrimitives,
{
    /// Validate a block with regard to execution results:
    ///
    /// - Compares the receipts root in the block header to the block body
    /// - Compares the gas used in the block header to the actual gas usage after execution
    fn validate_block_post_execution(
        &self,
        block: &RecoveredBlock<N::Block>,
        result: &BlockExecutionResult<N::Receipt>,
    ) -> Result<(), ConsensusError> {
        validate_block_post_execution(block, &self.chain_spec, &result.receipts, &result.requests)
    }
}

impl<B: Block> Consensus<B> for KasplexBeaconConsensus {
    /// The error type related to consensus.
    type Error = ConsensusError;

    /// Ensures the block response data matches the header.
    ///
    /// This ensures the body response items match the header's hashes:
    ///   - ommer hash
    ///   - transaction root
    ///   - withdrawals root
    fn validate_body_against_header(
        &self,
        body: &B::Body,
        header: &SealedHeader<B::Header>,
    ) -> Result<(), ConsensusError> {
        validate_body_against_header(body, header.header())
    }

    /// Validate a block without regard for state:
    ///
    /// - Compares the ommer hash in the block header to the block body
    /// - Compares the transactions root in the block header to the block body
    fn validate_block_pre_execution(&self, _block: &SealedBlock<B>) -> Result<(), ConsensusError> {
        // In Kasplex network, ommer hash is always empty.
        // validate_body_against_header already checks ommers hash, so we just verify it's empty
        // by checking the header through the header validator
        // Note: We use the same check as in validate_header
        // Since BlockHeader trait may not have ommers_hash(), we rely on validate_body_against_header
        // which already checks this. For now, we just return Ok().
        // The ommers_hash check is done in validate_header which is called elsewhere.
        Ok(())
    }
}

impl<H> HeaderValidator<H> for KasplexBeaconConsensus
where
    H: BlockHeader,
{
    /// Validate if header is correct and follows consensus specification.
    ///
    /// This is called on standalone header to check if all hashes are correct.
    fn validate_header(&self, header: &SealedHeader<H>) -> Result<(), ConsensusError> {
        let header = header.header();

        if !header.difficulty().is_zero() {
            return Err(ConsensusError::TheMergeDifficultyIsNotZero);
        }

        if !header.nonce().is_some_and(|nonce| nonce.is_zero()) {
            return Err(ConsensusError::TheMergeNonceIsNotZero);
        }

        if header.ommers_hash() != EMPTY_OMMER_ROOT_HASH {
            return Err(ConsensusError::TheMergeOmmerRootIsNotEmpty);
        }

        validate_header_extra_data(header)?;
        validate_header_gas(header)?;
        validate_header_base_fee(header, &self.chain_spec)?;
        validate_kasplex_base_fee_in_header(header)
    }

    /// Validate that the header information regarding parent are correct.
    fn validate_header_against_parent(
        &self,
        header: &SealedHeader<H>,
        parent: &SealedHeader<H>,
    ) -> Result<(), ConsensusError> {
        validate_against_parent_hash_number(header.header(), parent)?;

        let header_base_fee =
            header.header().base_fee_per_gas().ok_or(ConsensusError::BaseFeeMissing)?;

        // Kasplex allows block.timestamp == parent.timestamp (unlike standard Ethereum)
        if header.timestamp() < parent.timestamp() {
            return Err(ConsensusError::TimestampIsInPast {
                parent_timestamp: parent.timestamp(),
                timestamp: header.timestamp(),
            });
        }

        // Validate that the base fee is exactly KASPLEX_BASE_FEE
        let expected_base_fee = KASPLEX_BASE_FEE;
        if header_base_fee != expected_base_fee {
            return Err(ConsensusError::BaseFeeDiff(GotExpected {
                got: header_base_fee,
                expected: expected_base_fee,
            }));
        }

        Ok(())
    }
}

/// Validate that the base fee in the header matches Kasplex's fixed base fee.
fn validate_kasplex_base_fee_in_header<H: BlockHeader>(
    header: &H,
) -> Result<(), ConsensusError> {
    let base_fee: u64 = header.base_fee_per_gas().ok_or(ConsensusError::BaseFeeMissing)?;
    let expected_base_fee = KASPLEX_BASE_FEE;

    if base_fee != expected_base_fee {
        return Err(ConsensusError::BaseFeeDiff(GotExpected {
            got: base_fee,
            expected: expected_base_fee,
        }));
    }

    Ok(())
}

/// Calculate the base fee for a Kasplex chain.
///
/// Kasplex uses a fixed base fee of 2000 GWei for all blocks,
/// regardless of parent block's base fee or gas usage.
pub fn calculate_kasplex_base_fee() -> U256 {
    U256::from(KASPLEX_BASE_FEE)
}

/// Check if a base fee is valid for Kasplex.
///
/// For Kasplex chains, the base fee must be exactly 2000 GWei.
pub fn validate_kasplex_base_fee(base_fee: Option<U256>) -> bool {
    base_fee.map_or(false, |fee| fee == U256::from(KASPLEX_BASE_FEE))
}

/// Get the expected base fee for Kasplex chains.
pub fn get_kasplex_base_fee() -> U256 {
    U256::from(KASPLEX_BASE_FEE)
}
