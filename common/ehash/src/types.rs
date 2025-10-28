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
/// - Apply NUT-20 P2PK locks per-share for secure token redemption
#[derive(Debug, Clone)]
pub struct EHashMintData {
    /// The share hash returned from share validation
    pub share_hash: Hash,

    /// Whether this share found a block
    pub block_found: bool,

    /// The channel ID this share was submitted on
    pub channel_id: u32,

    /// The user identity associated with this channel (hpub format)
    /// Format: bech32-encoded public key with 'hpub' HRP (Human Readable Part)
    /// Example: "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k..."
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

    /// Locking pubkey for NUT-20 P2PK authentication (required per-share)
    ///
    /// This pubkey is extracted from:
    /// 1. Downstream miner sets user_identity to their hpub (bech32 format) when connecting
    /// 2. Proxy validates hpub format - INVALID = disconnect + no jobs
    /// 3. Proxy extracts secp256k1 public key from hpub
    /// 4. Proxy includes pubkey in SubmitSharesExtended TLV when submitting upstream
    /// 5. Pool extracts pubkey from TLV and includes here
    ///
    /// Minted eHash tokens are always P2PK-locked to this public key.
    /// The wallet must authenticate with the corresponding private key (NUT-20)
    /// to mint tokens from the PAID quote.
    ///
    /// Each share MUST have a locking pubkey, enabling:
    /// - Guaranteed security (all tokens are P2PK-locked)
    /// - Per-share granularity (different keys for different shares)
    /// - Key rotation (miners can change keys per share)
    /// - Multi-miner support (proxy handles multiple downstream miners, each with own key)
    pub locking_pubkey: bitcoin::secp256k1::PublicKey,
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
/// - Which locking pubkey was used (for multi-miner proxy support)
///
/// Note: user_identity can be derived from locking_pubkey (encoded as hpub) when needed
/// for display purposes, eliminating data redundancy.
#[derive(Debug, Clone)]
pub struct WalletCorrelationData {
    /// The channel ID this share was submitted on
    pub channel_id: u32,

    /// The sequence number of this share submission
    pub sequence_number: u32,

    /// When this correlation event was created
    pub timestamp: SystemTime,

    /// Number of eHash tokens minted for this share submission
    /// (extracted from SubmitSharesSuccess TLV field 0x0003|0x03)
    pub ehash_tokens_minted: u32,

    /// Locking pubkey for this downstream miner
    /// This enables TProxy to track eHash accounting per downstream miner pubkey
    /// Can be encoded as hpub for display purposes when needed
    pub locking_pubkey: bitcoin::secp256k1::PublicKey,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash as HashTrait;
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};

    fn test_pubkey() -> PublicKey {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[1u8; 32]).unwrap();
        PublicKey::from_secret_key(&secp, &secret)
    }

    #[test]
    fn test_ehash_mint_data_creation() {
        let share_hash = Hash::from_byte_array([0u8; 32]);
        let data = EHashMintData {
            share_hash,
            block_found: false,
            channel_id: 1,
            user_identity: "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 42,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        assert_eq!(data.channel_id, 1);
        assert_eq!(data.sequence_number, 42);
        assert!(!data.block_found);
        assert!(data.locking_pubkey.serialize().len() == 33);
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
            user_identity: "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
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
            user_identity: "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
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
            locking_pubkey: test_pubkey(),
        };

        assert_eq!(data.channel_id, 1);
        assert_eq!(data.sequence_number, 42);
        assert_eq!(data.ehash_tokens_minted, 256);
        assert!(data.locking_pubkey.serialize().len() == 33);
    }
}
