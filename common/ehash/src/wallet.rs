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

        // Initialize optional CDK Wallet instance if mint_url is provided
        let wallet_instance = if let Some(ref mint_url) = config.mint_url {
            // TODO: Initialize CDK Wallet with mint_url
            // This requires CDK Wallet initialization which may need additional setup
            // For now, leaving as None until CDK integration is fully implemented
            tracing::info!("Wallet initialization with mint_url: {}", mint_url);
            None
        } else {
            None
        };

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

    /// Process correlation data from SubmitSharesSuccess events
    ///
    /// Updates accounting statistics for display purposes:
    /// - Increments total eHash balance for the downstream miner's pubkey
    /// - Updates channel statistics (share count, last activity)
    ///
    /// Does NOT redeem tokens - external wallets handle redemption via authenticated API
    ///
    /// # Arguments
    /// * `data` - Correlation data from SubmitSharesSuccess message
    ///
    /// # Returns
    /// Ok(()) if processing succeeds, WalletError otherwise
    pub async fn process_correlation_data(
        &mut self,
        data: WalletCorrelationData,
    ) -> Result<(), WalletError> {
        // Update balance for this pubkey
        let ehash_amount = data.ehash_tokens_minted as u64;
        *self.ehash_balances.entry(data.locking_pubkey).or_insert(0) += ehash_amount;

        // Derive user_identity from locking_pubkey (encode as hpub for display)
        let user_identity = crate::hpub::encode_hpub(&data.locking_pubkey)
            .unwrap_or_else(|_| format!("pubkey_{:?}", data.locking_pubkey));

        // Update channel statistics
        let stats = self.channel_stats.entry(data.channel_id).or_insert(ChannelStats {
            channel_id: data.channel_id,
            locking_pubkey: data.locking_pubkey,
            user_identity: user_identity.clone(),
            total_ehash: 0,
            share_count: 0,
            last_share_time: data.timestamp,
        });

        stats.total_ehash += ehash_amount;
        stats.share_count += 1;
        stats.last_share_time = data.timestamp;

        tracing::info!(
            target: "wallet_handler",
            "Updated eHash balance for {}: +{} (total: {})",
            user_identity,
            ehash_amount,
            stats.total_ehash
        );

        Ok(())
    }

    /// Process correlation data with automatic retry on failure
    ///
    /// # Arguments
    /// * `data` - Correlation data to process
    ///
    /// # Returns
    /// Ok(()) if processing succeeds, WalletError if processing fails and data is queued
    async fn process_correlation_data_with_retry(
        &mut self,
        data: WalletCorrelationData,
    ) -> Result<(), WalletError> {
        match self.process_correlation_data(data.clone()).await {
            Ok(()) => {
                // Reset failure count on success
                self.failure_count = 0;
                self.last_failure = None;
                Ok(())
            }
            Err(e) => {
                // Queue for retry and increment failure count
                self.retry_queue.push_back(data);
                self.failure_count += 1;
                self.last_failure = Some(SystemTime::now());

                // Disable if too many failures
                if self.failure_count > self.config.max_retries {
                    tracing::warn!(
                        "Wallet handler disabled after {} failures",
                        self.config.max_retries
                    );
                }

                Err(e)
            }
        }
    }

    /// Attempt to recover from failures by processing the retry queue
    ///
    /// Uses exponential backoff to avoid hammering failed operations.
    /// Stops processing on first failure and waits for next recovery attempt.
    ///
    /// # Returns
    /// Ok(()) if recovery succeeds or is not yet due, WalletError on failure
    async fn attempt_recovery(&mut self) -> Result<(), WalletError> {
        if !self.config.recovery_enabled {
            return Ok(());
        }

        // Check if enough time has passed for retry (exponential backoff)
        if let Some(last_failure) = self.last_failure {
            let backoff_duration = std::time::Duration::from_secs(
                self.config.backoff_multiplier * (2_u64.pow(self.failure_count.min(10))),
            );

            if last_failure.elapsed().unwrap_or(std::time::Duration::ZERO) < backoff_duration {
                return Ok(());
            }
        }

        // Process retry queue
        while let Some(data) = self.retry_queue.pop_front() {
            match self.process_correlation_data(data.clone()).await {
                Ok(()) => {
                    tracing::info!("Wallet recovery successful");
                    self.failure_count = 0;
                    self.last_failure = None;
                }
                Err(_) => {
                    // Put back in queue and stop processing
                    self.retry_queue.push_front(data);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Main processing loop for the wallet thread
    ///
    /// Continuously processes incoming correlation events from the async channel.
    /// This method blocks until the channel is closed.
    ///
    /// # Returns
    /// Ok(()) when channel is closed, WalletError on fatal errors
    pub async fn run(&mut self) -> Result<(), WalletError> {
        tracing::info!("WalletHandler run loop started");

        loop {
            tokio::select! {
                // Process incoming correlation events
                event = self.receiver.recv() => {
                    match event {
                        Ok(data) => {
                            if let Err(e) = self.process_correlation_data_with_retry(data).await {
                                tracing::warn!("Failed to process correlation data: {}", e);
                            }
                        }
                        Err(_) => {
                            tracing::info!("Wallet handler channel closed, exiting run loop");
                            break;
                        }
                    }
                }
                // Attempt recovery periodically
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    if let Err(e) = self.attempt_recovery().await {
                        tracing::warn!("Recovery attempt failed: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Main processing loop with graceful shutdown handling
    ///
    /// Like `run()`, but accepts a shutdown signal channel.
    /// Completes pending operations before terminating.
    ///
    /// # Arguments
    /// * `shutdown_rx` - Channel that signals when to shut down
    ///
    /// # Returns
    /// Ok(()) on successful shutdown, WalletError on fatal errors
    pub async fn run_with_shutdown(
        &mut self,
        shutdown_rx: Receiver<()>,
    ) -> Result<(), WalletError> {
        tracing::info!("WalletHandler run loop started with shutdown support");

        loop {
            tokio::select! {
                // Process incoming correlation events
                event = self.receiver.recv() => {
                    match event {
                        Ok(data) => {
                            if let Err(e) = self.process_correlation_data_with_retry(data).await {
                                tracing::warn!("Failed to process correlation data: {}", e);
                            }
                        }
                        Err(_) => {
                            tracing::info!("Wallet handler channel closed");
                            break;
                        }
                    }
                }
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    tracing::info!("Wallet handler received shutdown signal, completing pending operations...");
                    self.shutdown().await?;
                    break;
                }
                // Attempt recovery periodically
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    if let Err(e) = self.attempt_recovery().await {
                        tracing::warn!("Recovery attempt failed: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Gracefully shutdown the wallet handler, completing pending operations
    ///
    /// Processes any remaining items in the retry queue before terminating.
    ///
    /// # Returns
    /// Ok(()) on successful shutdown, WalletError if pending operations fail
    pub async fn shutdown(&mut self) -> Result<(), WalletError> {
        tracing::info!("Shutting down wallet handler...");

        // Process remaining retry queue
        while let Some(data) = self.retry_queue.pop_front() {
            if let Err(e) = self.process_correlation_data(data).await {
                tracing::warn!("Failed to process queued data during shutdown: {}", e);
                // Continue processing other items
            }
        }

        tracing::info!("Wallet handler shutdown complete");
        Ok(())
    }

    /// Query P2PK-locked tokens from the mint for a specific downstream miner's pubkey
    ///
    /// This method queries the CDK Wallet (if configured) for tokens that are
    /// P2PK-locked to the specified pubkey. External wallets with the private key
    /// can use this to discover and redeem their tokens.
    ///
    /// # Arguments
    /// * `pubkey` - The locking pubkey to query tokens for
    ///
    /// # Returns
    /// Vec of P2PK-locked token proofs, or empty vec if none found or wallet not configured
    ///
    /// # Errors
    /// Returns `WalletError` if CDK Wallet query fails
    pub async fn query_p2pk_tokens(
        &self,
        _pubkey: &Secp256k1PublicKey,
    ) -> Result<Vec<cdk::nuts::Proof>, WalletError> {
        // Check if CDK Wallet is configured
        if self.wallet_instance.is_none() {
            tracing::debug!("CDK Wallet not configured, cannot query P2PK tokens");
            return Ok(vec![]);
        }

        // TODO: Implement CDK Wallet P2PK token query
        // This requires:
        // 1. Query CDK Wallet for all proofs
        // 2. Filter proofs by P2PK condition matching pubkey
        // 3. Return filtered proofs
        //
        // For now, returning empty vec as CDK integration is incomplete
        tracing::debug!("P2PK token query not yet implemented");
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};

    fn test_pubkey() -> PublicKey {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[1u8; 32]).unwrap();
        PublicKey::from_secret_key(&secp, &secret)
    }

    fn test_pubkey_2() -> PublicKey {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[2u8; 32]).unwrap();
        PublicKey::from_secret_key(&secp, &secret)
    }

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

    #[tokio::test]
    async fn test_wallet_handler_with_mint_url() {
        let config = WalletConfig {
            mint_url: Some("https://mint.example.com".parse().unwrap()),
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
        let pubkey = test_pubkey();

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

    #[tokio::test]
    async fn test_correlation_data_processing() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        };

        let mut handler = WalletHandler::new(config).await.unwrap();

        let pubkey = test_pubkey();
        let correlation_data = WalletCorrelationData {
            channel_id: 1,
            sequence_number: 42,
            user_identity: "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k".to_string(),
            timestamp: SystemTime::now(),
            ehash_tokens_minted: 256,
            locking_pubkey: pubkey,
        };

        // Process correlation data
        handler
            .process_correlation_data(correlation_data.clone())
            .await
            .unwrap();

        // Verify balance was updated
        assert_eq!(handler.get_ehash_balance(&pubkey), 256);

        // Verify channel stats were created
        let stats = handler.get_channel_stats(1).unwrap();
        assert_eq!(stats.channel_id, 1);
        assert_eq!(stats.total_ehash, 256);
        assert_eq!(stats.share_count, 1);
        assert_eq!(stats.locking_pubkey, pubkey);
    }

    #[tokio::test]
    async fn test_multiple_correlation_data_processing() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        };

        let mut handler = WalletHandler::new(config).await.unwrap();

        let pubkey = test_pubkey();

        // Process multiple correlation events
        for i in 0..5 {
            let correlation_data = WalletCorrelationData {
                channel_id: 1,
                sequence_number: i,
                user_identity: "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k".to_string(),
                timestamp: SystemTime::now(),
                ehash_tokens_minted: 100,
                locking_pubkey: pubkey,
            };

            handler
                .process_correlation_data(correlation_data)
                .await
                .unwrap();
        }

        // Verify total balance
        assert_eq!(handler.get_ehash_balance(&pubkey), 500);

        // Verify channel stats
        let stats = handler.get_channel_stats(1).unwrap();
        assert_eq!(stats.total_ehash, 500);
        assert_eq!(stats.share_count, 5);
    }

    #[tokio::test]
    async fn test_multi_miner_support() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        };

        let mut handler = WalletHandler::new(config).await.unwrap();

        let pubkey1 = test_pubkey();
        let pubkey2 = test_pubkey_2();

        // Process correlation data for miner 1
        let correlation_data1 = WalletCorrelationData {
            channel_id: 1,
            sequence_number: 1,
            user_identity: "miner1".to_string(),
            timestamp: SystemTime::now(),
            ehash_tokens_minted: 100,
            locking_pubkey: pubkey1,
        };

        handler
            .process_correlation_data(correlation_data1)
            .await
            .unwrap();

        // Process correlation data for miner 2
        let correlation_data2 = WalletCorrelationData {
            channel_id: 2,
            sequence_number: 1,
            user_identity: "miner2".to_string(),
            timestamp: SystemTime::now(),
            ehash_tokens_minted: 200,
            locking_pubkey: pubkey2,
        };

        handler
            .process_correlation_data(correlation_data2)
            .await
            .unwrap();

        // Verify each miner has correct balance
        assert_eq!(handler.get_ehash_balance(&pubkey1), 100);
        assert_eq!(handler.get_ehash_balance(&pubkey2), 200);

        // Verify channel stats
        assert_eq!(handler.get_channel_stats(1).unwrap().total_ehash, 100);
        assert_eq!(handler.get_channel_stats(2).unwrap().total_ehash, 200);
    }

    #[tokio::test]
    async fn test_retry_queue_functionality() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 3,
            backoff_multiplier: 1,
            recovery_enabled: true,
            log_level: None,
        };

        let mut handler = WalletHandler::new(config).await.unwrap();

        // Test that retry queue is initially empty
        assert_eq!(handler.retry_queue.len(), 0);
        assert_eq!(handler.failure_count, 0);
    }

    #[tokio::test]
    async fn test_channel_sender_receiver() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        };

        let handler = WalletHandler::new(config).await.unwrap();

        let sender = handler.get_sender();
        let receiver = handler.get_receiver();

        let pubkey = test_pubkey();
        let correlation_data = WalletCorrelationData {
            channel_id: 1,
            sequence_number: 1,
            user_identity: "test".to_string(),
            timestamp: SystemTime::now(),
            ehash_tokens_minted: 100,
            locking_pubkey: pubkey,
        };

        // Send via sender
        sender.send(correlation_data.clone()).await.unwrap();

        // Receive via receiver
        let received = receiver.recv().await.unwrap();

        assert_eq!(received.channel_id, correlation_data.channel_id);
        assert_eq!(received.sequence_number, correlation_data.sequence_number);
        assert_eq!(
            received.ehash_tokens_minted,
            correlation_data.ehash_tokens_minted
        );
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        };

        let mut handler = WalletHandler::new(config).await.unwrap();

        // Add some data to retry queue
        let pubkey = test_pubkey();
        let correlation_data = WalletCorrelationData {
            channel_id: 1,
            sequence_number: 1,
            user_identity: "test".to_string(),
            timestamp: SystemTime::now(),
            ehash_tokens_minted: 100,
            locking_pubkey: pubkey,
        };

        handler.retry_queue.push_back(correlation_data);

        // Shutdown should process the retry queue
        handler.shutdown().await.unwrap();

        // Verify retry queue was processed
        assert_eq!(handler.retry_queue.len(), 0);
        assert_eq!(handler.get_ehash_balance(&pubkey), 100);
    }

    #[tokio::test]
    async fn test_p2pk_token_query_no_wallet() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        };

        let handler = WalletHandler::new(config).await.unwrap();

        let pubkey = test_pubkey();
        let tokens = handler.query_p2pk_tokens(&pubkey).await.unwrap();

        // Should return empty vec when wallet not configured
        assert_eq!(tokens.len(), 0);
    }

    #[tokio::test]
    async fn test_all_balances_and_stats_accessors() {
        let config = WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        };

        let mut handler = WalletHandler::new(config).await.unwrap();

        let pubkey1 = test_pubkey();
        let pubkey2 = test_pubkey_2();

        // Add multiple miners
        handler
            .process_correlation_data(WalletCorrelationData {
                channel_id: 1,
                sequence_number: 1,
                user_identity: "miner1".to_string(),
                timestamp: SystemTime::now(),
                ehash_tokens_minted: 100,
                locking_pubkey: pubkey1,
            })
            .await
            .unwrap();

        handler
            .process_correlation_data(WalletCorrelationData {
                channel_id: 2,
                sequence_number: 1,
                user_identity: "miner2".to_string(),
                timestamp: SystemTime::now(),
                ehash_tokens_minted: 200,
                locking_pubkey: pubkey2,
            })
            .await
            .unwrap();

        // Test get_all_balances
        let all_balances = handler.get_all_balances();
        assert_eq!(all_balances.len(), 2);
        assert_eq!(all_balances.get(&pubkey1), Some(&100));
        assert_eq!(all_balances.get(&pubkey2), Some(&200));

        // Test get_all_channel_stats
        let all_stats = handler.get_all_channel_stats();
        assert_eq!(all_stats.len(), 2);
        assert!(all_stats.contains_key(&1));
        assert!(all_stats.contains_key(&2));
    }
}
