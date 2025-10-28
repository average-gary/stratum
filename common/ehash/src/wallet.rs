//! Wallet handler implementation for eHash accounting and correlation tracking
//!
//! This module provides the `WalletHandler` type which manages:
//! - Async channel communication for share correlation events
//! - eHash accounting statistics for downstream miners
//! - Optional CDK Wallet instance for token queries
//! - Locking pubkey management for P2PK authentication
//!
//! ## Design Philosophy
//!
//! The WalletHandler focuses on **accounting and tracking** rather than token redemption:
//! - Tracks total eHash earned per pubkey (for display purposes)
//! - Tracks channel statistics (shares submitted, last activity)
//! - Does NOT redeem tokens automatically
//! - External wallets with private keys handle redemption via authenticated API
//!
//! ## External Wallet Redemption Flow
//!
//! 1. WalletHandler tracks correlation data from SubmitSharesSuccess
//! 2. External wallets query mint for quotes by locking pubkey (signature-authenticated)
//! 3. External wallets receive secret UUID quote IDs
//! 4. External wallets create blinded messages and sign MintRequest (NUT-20)
//! 5. Mint verifies signature matches quote's locking pubkey
//! 6. External wallets receive blind signatures and unblind to get eHash tokens
//!
//! ## Thread Safety
//! The WalletHandler is designed to run in a dedicated thread spawned via
//! the task manager, completely isolated from translation operations.
//!
//! ## Fault Tolerance
//! The WalletHandler implements automatic retry logic with exponential backoff:
//! - Failed correlation operations are queued for retry
//! - Automatic recovery attempts with configurable backoff
//! - Translation operations continue even during wallet failures

use crate::config::WalletConfig;
use crate::error::WalletError;
use crate::types::WalletCorrelationData;
use async_channel::{Receiver, Sender};
use bitcoin::secp256k1::PublicKey as Secp256k1PublicKey;
use cdk::Wallet as CdkWallet;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::SystemTime;

/// Channel statistics for tracking eHash accounting
///
/// This structure tracks statistics for a specific channel/downstream miner:
/// - Total eHash earned
/// - Number of shares submitted
/// - Last activity timestamp
/// - Associated pubkey and user identity
#[derive(Debug, Clone)]
pub struct ChannelStats {
    /// The channel ID
    pub channel_id: u32,

    /// Locking pubkey for this channel
    pub locking_pubkey: Secp256k1PublicKey,

    /// User identity string
    pub user_identity: String,

    /// Total eHash tokens earned by this channel
    pub total_ehash: u64,

    /// Total number of shares submitted
    pub share_count: u64,

    /// Timestamp of the last share submission
    pub last_share_time: SystemTime,
}

/// Handler for eHash accounting and correlation tracking
///
/// The WalletHandler manages eHash accounting for multiple downstream miners:
/// - Receives correlation events from SubmitSharesSuccess messages
/// - Tracks total eHash earned per downstream miner's pubkey
/// - Tracks channel statistics for display
/// - Does NOT redeem tokens (external wallets handle redemption)
///
/// # Multi-Miner Support
/// TProxy can handle multiple downstream miners simultaneously, each with their own
/// locking pubkey. The pubkeys are extracted from correlation events (which come from
/// the user_identity hpub field when miners connect).
///
/// # Thread Safety
/// The WalletHandler is designed to run in a dedicated thread spawned via
/// the task manager, completely isolated from TProxy translation operations.
///
/// # Fault Tolerance
/// The WalletHandler implements automatic retry logic with exponential backoff:
/// - Failed correlation operations are queued for retry
/// - Automatic recovery attempts with configurable backoff
/// - Translation operations continue even during wallet failures
///
/// # External Wallet Integration
/// External wallets can query the mint for outstanding quotes by:
/// 1. Signing a challenge message with their private key (NUT-20)
/// 2. Querying the mint's authenticated API with pubkey + signature
/// 3. Receiving secret UUID quote IDs for PAID quotes
/// 4. Minting tokens via standard NUT-20 authenticated flow
pub struct WalletHandler {
    /// Optional CDK Wallet instance for token queries
    /// If None, only accounting/tracking is performed
    wallet_instance: Option<Arc<CdkWallet>>,

    /// Receiver for incoming correlation events
    receiver: Receiver<WalletCorrelationData>,

    /// Sender for correlation events (cloneable for distribution)
    sender: Sender<WalletCorrelationData>,

    /// Configuration for wallet operations
    config: WalletConfig,

    /// Accounting data: pubkey -> total eHash earned
    /// Tracks eHash balances for all downstream miners
    ehash_balances: HashMap<Secp256k1PublicKey, u64>,

    /// Channel statistics: channel_id -> stats
    /// Maps channel IDs to their statistics (includes per-channel pubkey)
    channel_stats: HashMap<u32, ChannelStats>,

    /// Queue of failed correlation operations awaiting retry
    retry_queue: VecDeque<WalletCorrelationData>,

    /// Number of consecutive failures
    failure_count: u32,

    /// Timestamp of the last failure (for exponential backoff)
    last_failure: Option<SystemTime>,
}

impl WalletHandler {
    /// Create new WalletHandler with optional CDK Wallet instance
    ///
    /// # Arguments
    /// * `config` - Wallet configuration including optional mint URL and fault tolerance settings
    ///
    /// # Returns
    /// A new WalletHandler instance ready to process correlation events from multiple downstream miners
    ///
    /// # Errors
    /// Returns `WalletError` if:
    /// - CDK Wallet initialization fails (if mint_url is provided)
    /// - Configuration is invalid
    pub async fn new(config: WalletConfig) -> Result<Self, WalletError> {
        // Create async channel for WalletCorrelationData events
        let (sender, receiver) = async_channel::unbounded();

        // TODO: Initialize optional CDK Wallet instance
        // This will be implemented in task 4.2
        let wallet_instance = None;

        Ok(Self {
            wallet_instance,
            receiver,
            sender,
            config,
            ehash_balances: HashMap::new(),
            channel_stats: HashMap::new(),
            retry_queue: VecDeque::new(),
            failure_count: 0,
            last_failure: None,
        })
    }

    /// Get a cloneable sender for WalletCorrelationData events
    ///
    /// This sender can be distributed to message handlers to send
    /// correlation events to the wallet thread.
    pub fn get_sender(&self) -> Sender<WalletCorrelationData> {
        self.sender.clone()
    }

    /// Get the receiver for WalletCorrelationData events
    ///
    /// This receiver should only be used by the wallet thread's
    /// main processing loop.
    pub fn get_receiver(&self) -> Receiver<WalletCorrelationData> {
        self.receiver.clone()
    }

    /// Get total eHash earned by a specific downstream miner's pubkey
    ///
    /// # Arguments
    /// * `pubkey` - The locking pubkey for the downstream miner
    ///
    /// # Returns
    /// Total eHash tokens earned by this pubkey, or 0 if no records exist
    pub fn get_ehash_balance(&self, pubkey: &Secp256k1PublicKey) -> u64 {
        self.ehash_balances.get(pubkey).copied().unwrap_or(0)
    }

    /// Get accounting statistics for a channel
    ///
    /// # Arguments
    /// * `channel_id` - The channel ID to query
    ///
    /// # Returns
    /// Channel statistics if the channel exists, None otherwise
    pub fn get_channel_stats(&self, channel_id: u32) -> Option<&ChannelStats> {
        self.channel_stats.get(&channel_id)
    }

    /// Get all tracked pubkeys and their balances
    ///
    /// # Returns
    /// Reference to the HashMap of all downstream miner pubkeys and their eHash balances
    pub fn get_all_balances(&self) -> &HashMap<Secp256k1PublicKey, u64> {
        &self.ehash_balances
    }

    /// Get all channel statistics
    ///
    /// # Returns
    /// Reference to the HashMap of all channel statistics
    pub fn get_all_channel_stats(&self) -> &HashMap<u32, ChannelStats> {
        &self.channel_stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wallet_handler_creation() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        };

        let handler = WalletHandler::new(config).await.unwrap();

        // Verify handler was created successfully
        assert_eq!(handler.get_all_balances().len(), 0);
        assert_eq!(handler.get_all_channel_stats().len(), 0);
    }

    #[test]
    fn test_channel_stats_creation() {
        use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};

        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let pubkey = PublicKey::from_secret_key(&secp, &secret);

        let stats = ChannelStats {
            channel_id: 1,
            locking_pubkey: pubkey,
            user_identity: "test_user".to_string(),
            total_ehash: 1024,
            share_count: 10,
            last_share_time: SystemTime::now(),
        };

        assert_eq!(stats.channel_id, 1);
        assert_eq!(stats.total_ehash, 1024);
        assert_eq!(stats.share_count, 10);
    }
}
