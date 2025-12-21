use alloy_primitives::Bytes;
use alloy_rlp::{Decodable, Encodable};
use alloy_rpc_types_eth::Withdrawals;
use reth::{
    api::PayloadBuilderAttributes,
    payload::PayloadId,
    primitives::Recovered,
    revm::primitives::{Address, B256, keccak256, U256},
};
use reth_ethereum::TransactionSigned;
use reth_ethereum_engine_primitives::EthPayloadBuilderAttributes;
use reth_primitives_traits::SignerRecoverable;
use std::fmt::Debug;
use tracing::debug;

use crate::payload::attributes::KasplexPayloadAttributes;

/// Kasplex Payload Builder Attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KasplexPayloadBuilderAttributes {
    /// Inner ethereum payload builder attributes
    pub payload_attributes: EthPayloadBuilderAttributes,
    /// Kasplex related attributes.
    // The hash of the RLP-encoded transactions in the block.
    pub tx_list_hash: B256,
    // The coinbase for the block.
    pub beneficiary: Address,
    // The gas limit for the block.
    pub gas_limit: u64,
    // The timestamp for the block.
    pub timestamp: u64,
    // The mix hash for the block.
    pub mix_hash: B256,
    // The basefee for the block.
    pub base_fee_per_gas: U256,
    // The transactions inside the block.
    pub transactions: Vec<Recovered<TransactionSigned>>,
    // The extra data for the block.
    pub extra_data: Bytes,
}

impl PayloadBuilderAttributes for KasplexPayloadBuilderAttributes {
    /// The payload attributes that can be used to construct this type. Used as the argument in
    /// [`PayloadBuilderAttributes::try_new`].
    type RpcPayloadAttributes = KasplexPayloadAttributes;
    /// The error type used in [`PayloadBuilderAttributes::try_new`].
    type Error = alloy_rlp::Error;

    /// Creates a new payload builder for the given parent block and the attributes.
    ///
    /// Derives the unique [`PayloadId`] for the given parent and attributes.
    fn try_new(
        parent: B256,
        attributes: KasplexPayloadAttributes,
        version: u8,
    ) -> Result<Self, Self::Error> {
        let id = payload_id_kasplex(&parent, &attributes, version);

        let payload_attributes = EthPayloadBuilderAttributes {
            id,
            parent,
            timestamp: attributes.payload_attributes.timestamp,
            suggested_fee_recipient: attributes.payload_attributes.suggested_fee_recipient,
            prev_randao: attributes.payload_attributes.prev_randao,
            withdrawals: attributes.payload_attributes.withdrawals.unwrap_or_default().into(),
            parent_beacon_block_root: attributes.payload_attributes.parent_beacon_block_root,
        };

        let transactions = decode_transactions(&attributes.block_metadata.tx_list)
            .unwrap_or_else(|e| {
                // If we can't decode the given transactions bytes, we will mine an empty block instead.
                debug!(
                    target: "payload_builder", "Failed to decode transactions: {e}, bytes: {:?}, skipping all transactions",
                    &attributes.block_metadata.tx_list
                );
                Vec::new()
            })
            .into_iter()
            .filter_map(|tx| match tx.try_into_recovered() {
                Ok(recovered) => Some(recovered),
                Err(e) => {
                    debug!("Failed to recover transaction: {e}, skip this invalid transaction");
                    None
                }
            })
            .collect::<Vec<_>>();

        let res = Self {
            payload_attributes,
            tx_list_hash: keccak256(attributes.block_metadata.tx_list.clone()),
            beneficiary: attributes.block_metadata.beneficiary,
            gas_limit: attributes.block_metadata.gas_limit,
            timestamp: attributes.block_metadata.timestamp.to(),
            mix_hash: attributes.payload_attributes.prev_randao,
            base_fee_per_gas: attributes.base_fee_per_gas,
            extra_data: attributes.block_metadata.extra_data,
            transactions,
        };

        Ok(res)
    }

    /// Returns the id for the running payload job.
    fn payload_id(&self) -> PayloadId {
        self.payload_attributes.id
    }

    /// Returns the parent for the running payload job.
    fn parent(&self) -> B256 {
        self.payload_attributes.parent
    }

    /// Returns the timestamp for the running payload job.
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Returns the parent beacon block root for the running payload job.
    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.payload_attributes.parent_beacon_block_root
    }

    /// Returns the suggested fee recipient for the running payload job.
    fn suggested_fee_recipient(&self) -> Address {
        self.beneficiary
    }

    /// Returns the random beacon value for the running payload job.
    fn prev_randao(&self) -> B256 {
        self.mix_hash
    }

    /// Returns the withdrawals for the running payload job.
    fn withdrawals(&self) -> &Withdrawals {
        &self.payload_attributes.withdrawals
    }
}

/// Generates the payload id for the configured payload from the [`KasplexPayloadAttributes`].
///
/// Returns an 8-byte identifier by hashing the payload components with sha256 hash.
pub(crate) fn payload_id_kasplex(
    parent: &B256,
    attributes: &KasplexPayloadAttributes,
    payload_version: u8,
) -> PayloadId {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(parent.as_slice());
    hasher.update(&attributes.payload_attributes.timestamp.to_be_bytes()[..]);
    hasher.update(attributes.payload_attributes.prev_randao.as_slice());
    hasher.update(attributes.payload_attributes.suggested_fee_recipient.as_slice());
    if let Some(withdrawals) = &attributes.payload_attributes.withdrawals {
        let mut buf = Vec::new();
        withdrawals.encode(&mut buf);
        hasher.update(buf);
    }

    if let Some(parent_beacon_block) = attributes.payload_attributes.parent_beacon_block_root {
        hasher.update(parent_beacon_block);
    }
    let tx_hash = keccak256(attributes.block_metadata.tx_list.clone());
    hasher.update(tx_hash);

    let mut out = hasher.finalize();
    out[0] = payload_version;
    PayloadId::new(out.as_slice()[..8].try_into().expect("sufficient length"))
}

// Decodes the given RLP-encoded bytes into transactions.
fn decode_transactions(bytes: &[u8]) -> Result<Vec<TransactionSigned>, alloy_rlp::Error> {
    Vec::<TransactionSigned>::decode(&mut &bytes[..])
}

#[cfg(test)]
mod test {
    use reth::revm::primitives::hex;

    use super::*;

    #[test]
    fn test_decode_transactions() {
        let empty_decoded = decode_transactions(&Bytes::from_static(&hex!("0xc0")));

        assert_eq!(0, empty_decoded.unwrap().len());
    }
}
