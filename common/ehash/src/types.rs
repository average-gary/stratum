//! Core data structures for eHash operations
//!
//! This module defines the primary data types used throughout the eHash system:
//! - `EHashMintData` - Data required for minting eHash tokens from share validation
//! - `WalletCorrelationData` - Data for correlating wallet operations with shares

use bitcoin::hashes::sha256d::Hash;
use bitcoin::hashes::Hash as HashTrait;
use bitcoin::Target;
use std::time::SystemTime;

/// Data required for minting eHash tokens from a validated share
///
/// This structure contains all the information needed by the MintHandler to:
/// - Calculate the eHash amount based on share difficulty
/// - Associate minted tokens with the correct channel and user
/// - Handle block found events for keyset lifecycle management
#[derive(Debug, Clone)]
pub struct EHashMintData {
    /// The share hash returned from share validation
    pub share_hash: Hash,

    /// Whether this share found a block
    pub block_found: bool,

    /// The channel ID this share was submitted on
    pub channel_id: u32,

    /// The user identity associated with this channel
    pub user_identity: String,

    /// The target difficulty for this share
    pub target: Target,

    /// The sequence number of this share submission
    pub sequence_number: u32,

    /// When this share was processed
    pub timestamp: SystemTime,

    /// Optional template ID (only present for block found case)
    pub template_id: Option<u64>,

    /// Optional coinbase data (only present for block found case)
    pub coinbase: Option<Vec<u8>>,
}

impl EHashMintData {
    /// Calculate eHash amount using hashpool's exponential valuation method
    ///
    /// Formula: `2^(leading_zero_bits - minimum_difficulty)`
    ///
    /// # Arguments
    /// * `minimum_difficulty` - Minimum leading zero bits required to earn 1 unit of eHash
    ///
    /// # Returns
    /// The eHash amount (0 if share doesn't meet minimum difficulty threshold)
    pub fn calculate_ehash_amount(&self, minimum_difficulty: u32) -> u64 {
        let hash_bytes: [u8; 32] = *self.share_hash.as_byte_array();
        crate::calculate_ehash_amount(hash_bytes, minimum_difficulty)
    }
}

/// Data for correlating wallet operations with successful share submissions
///
/// This structure is used by the WalletHandler to track:
/// - Which shares have been successfully submitted
/// - How many eHash tokens were minted for the channel
/// - Correlation between channel/sequence and minted tokens
#[derive(Debug, Clone)]
pub struct WalletCorrelationData {
    /// The channel ID this share was submitted on
    pub channel_id: u32,

    /// The sequence number of this share submission
    pub sequence_number: u32,

    /// The user identity associated with this channel
    pub user_identity: String,

    /// When this correlation event was created
    pub timestamp: SystemTime,

    /// Number of eHash tokens minted for this share submission
    /// (extracted from SubmitSharesSuccess TLV field 0x0003|0x03)
    pub ehash_tokens_minted: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash as HashTrait;

    #[test]
    fn test_ehash_mint_data_creation() {
        let share_hash = Hash::from_byte_array([0u8; 32]);
        let data = EHashMintData {
            share_hash,
            block_found: false,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 42,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
        };

        assert_eq!(data.channel_id, 1);
        assert_eq!(data.sequence_number, 42);
        assert!(!data.block_found);
    }

    #[test]
    fn test_ehash_mint_data_calculate_amount() {
        // Create a hash with 40 leading zeros (5 bytes of zeros)
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..5].fill(0x00);
        let share_hash = Hash::from_byte_array(hash_bytes);

        let data = EHashMintData {
            share_hash,
            block_found: false,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
        };

        // 40 leading zeros - 32 minimum = 8, so 2^8 = 256
        assert_eq!(data.calculate_ehash_amount(32), 256);
    }

    #[test]
    fn test_ehash_mint_data_below_minimum() {
        // Create a hash with only 24 leading zeros (3 bytes of zeros)
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..3].fill(0x00);
        let share_hash = Hash::from_byte_array(hash_bytes);

        let data = EHashMintData {
            share_hash,
            block_found: false,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
        };

        // Below minimum threshold, should return 0
        assert_eq!(data.calculate_ehash_amount(32), 0);
    }

    #[test]
    fn test_wallet_correlation_data_creation() {
        let data = WalletCorrelationData {
            channel_id: 1,
            sequence_number: 42,
            user_identity: "test_user".to_string(),
            timestamp: SystemTime::now(),
            ehash_tokens_minted: 256,
        };

        assert_eq!(data.channel_id, 1);
        assert_eq!(data.sequence_number, 42);
        assert_eq!(data.ehash_tokens_minted, 256);
    }
}
