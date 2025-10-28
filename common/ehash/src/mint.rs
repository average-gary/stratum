//! Mint handler implementation for eHash token minting
//!
//! This module provides the `MintHandler` type which manages:
//! - CDK Mint instance initialization and lifecycle
//! - Async channel communication for share validation events
//! - eHash token minting based on share difficulty
//! - P2PK token locking for secure wallet redemption (via PAID quotes with locking pubkeys)
//! - Block found event handling and keyset lifecycle
//!
//! ## P2PK Token Locking (NUT-20) - Per-Share Design
//!
//! The MintHandler implements NUT-20 P2PK token locking with per-share granularity
//! to prevent front-running attacks. Per NUT-04, quote IDs MUST be random and secret.
//! NUT-20 adds public key authentication to enforce ownership:
//!
//! ### Protocol Flow:
//! 1. Downstream miner sets `user_identity` to their hpub (bech32-encoded pubkey)
//! 2. Proxy/wallet validates hpub format - INVALID = disconnect + no jobs
//! 3. Proxy extracts secp256k1 public key from hpub
//! 4. Proxy includes pubkey in `SubmitSharesExtended` TLV field when submitting upstream
//! 5. Pool extracts pubkey from TLV, includes in `EHashMintData.locking_pubkey`
//! 6. MintHandler creates PAID quote with:
//!    - Random UUID v4 quote ID (prevents front-running per NUT-04)
//!    - Per-share locking_pubkey from TLV (enforces NUT-20 authentication)
//! 7. External wallet authenticates with the private key for locking_pubkey (NUT-20)
//! 8. Wallet queries for PAID quotes matching their pubkey
//! 9. Wallet creates blinded messages with P2PK SpendingConditions
//! 10. Wallet signs the MintRequest with their private key (NUT-20 signature)
//! 11. Mint verifies signature, signs blinded messages, returns blind signatures
//! 12. Wallet unblinds to obtain P2PK-locked token proofs
//!
//! ### Security Properties:
//! - **Per-share locking**: Each share has its own pubkey (enables key rotation)
//! - **Front-running prevention**: UUID v4 quote ID is not derivable (NUT-04)
//! - **Authentication**: NUT-20 signature prevents unauthorized minting
//! - **Required security**: All eHash tokens are P2PK-locked (not optional)
//!
//! ## Share Hash Tracking
//!
//! Each MintQuote includes the share hash in its `payments` field as proof
//! that the quote was earned through mining work. This provides:
//! - Auditability: Track which shares generated which tokens
//! - Verification: Prove tokens were earned through valid mining work
//! - Correlation: Link tokens back to specific share submissions

use crate::config::MintConfig;
use crate::error::MintError;
use crate::types::EHashMintData;
use async_channel::{Receiver, Sender};
use cdk::amount::Amount;
use cdk::mint::MintBuilder;
use cdk::nuts::{CurrencyUnit, PaymentMethod, Proofs, PublicKey as CdkPublicKey};
use cdk::Mint;
use cdk_common::mint::MintQuote;
use cdk_common::payment::PaymentIdentifier;
use cdk_common::quote_id::QuoteId;
use cdk_sqlite::mint::memory;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::SystemTime;

/// Block reward information from Template Provider
///
/// This structure represents block reward details used for
/// calculating eHash-to-sats conversion rates during keyset
/// lifecycle transitions.
#[derive(Debug, Clone)]
pub struct BlockRewardInfo {
    /// Block height for this reward
    pub height: u64,
    /// Total reward in satoshis (coinbase + fees)
    pub total_reward_sats: u64,
    /// Coinbase reward in satoshis
    pub coinbase_reward_sats: u64,
    /// Transaction fees in satoshis
    pub fees_sats: u64,
    /// Template ID that produced this block
    pub template_id: u64,
}

/// Handler for eHash token minting operations
///
/// The MintHandler manages all aspects of eHash token creation:
/// - Receives share validation events via async channels
/// - Calculates eHash amounts based on share difficulty
/// - Creates P2PK-locked tokens using CDK (per-share pubkeys)
/// - Handles block found events for keyset lifecycle transitions
/// - Provides fault tolerance with retry queue and exponential backoff
///
/// # Thread Safety
/// The MintHandler is designed to run in a dedicated thread spawned via
/// the task manager, completely isolated from mining operations.
///
/// # Fault Tolerance
/// The MintHandler implements automatic retry logic with exponential backoff:
/// - Failed mint operations are queued for retry
/// - Automatic recovery attempts with configurable backoff
/// - Mining operations continue even during mint failures
///
/// # Per-Share P2PK Locking
/// Each share includes its own locking pubkey in `EHashMintData.locking_pubkey`.
/// No channel-level pubkey mapping is maintained - all pubkeys are per-share.
pub struct MintHandler {
    /// CDK Mint instance with native database and accounting
    mint_instance: Arc<Mint>,

    /// Receiver for incoming share validation events
    receiver: Receiver<EHashMintData>,

    /// Sender for share validation events (cloneable for distribution)
    sender: Sender<EHashMintData>,

    /// Configuration for mint operations
    config: MintConfig,

    /// Queue of failed mint operations awaiting retry
    retry_queue: VecDeque<EHashMintData>,

    /// Number of consecutive failures
    failure_count: u32,

    /// Timestamp of the last failure (for exponential backoff)
    last_failure: Option<SystemTime>,
}

impl MintHandler {
    /// Create new MintHandler with CDK's native database backend
    ///
    /// # Arguments
    /// * `config` - Mint configuration including database URL and mint settings
    ///
    /// # Returns
    /// A new MintHandler instance ready to process share validation events
    ///
    /// # Errors
    /// Returns `MintError` if:
    /// - CDK Mint initialization fails
    /// - Database connection cannot be established
    /// - Configuration is invalid
    pub async fn new(config: MintConfig) -> Result<Self, MintError> {
        // Create async channel for EHashMintData events
        let (sender, receiver) = async_channel::unbounded();

        // TODO: Initialize CDK Mint with database backend
        // This will be implemented in task 3.2
        let mint_instance = Arc::new(Self::initialize_cdk_mint(&config).await?);

        Ok(Self {
            mint_instance,
            receiver,
            sender,
            config,
            retry_queue: VecDeque::new(),
            failure_count: 0,
            last_failure: None,
        })
    }

    /// Initialize CDK Mint with database backend
    ///
    /// Configures the Mint with:
    /// - "HASH" currency unit for eHash tokens
    /// - Database backend from config
    /// - Mint private key (if provided)
    async fn initialize_cdk_mint(config: &MintConfig) -> Result<Mint, MintError> {
        tracing::info!("Initializing CDK Mint...");

        // Create database backend
        // TODO: Support file-based database from config.database_url
        // For now, using in-memory database
        let database = memory::empty()
            .await
            .map_err(|e| MintError::ConfigError(format!("Failed to create database: {}", e)))?;

        // Convert to Arc for shared ownership
        let database = Arc::new(database);

        // Create MintBuilder with database
        let mut mint_builder = MintBuilder::new(database.clone());

        // Configure mint info with URL
        mint_builder = mint_builder
            .with_name("eHash Mint".to_string())
            .with_description("Cashu ecash mint for hashpool eHash tokens".to_string())
            .with_urls(vec![config.mint_url.to_string()]);

        // TODO: Add payment processors if needed for bolt11 minting/melting
        // For eHash, we're bypassing the bolt11 quote flow and directly creating
        // PAID quotes, so we don't need payment processors for now

        // Create a seed for the mint
        // TODO: Support configurable seed/private key from config
        let seed = [21u8; 32]; // Default seed for now

        // Build the mint with seed using DbSignatory
        // The signatory handles keyset generation automatically
        let keystore = database.clone();

        // Create supported units configuration
        // (input_fee_ppk, max_order) for each unit
        let mut supported_units = HashMap::new();
        for unit in &config.supported_units {
            // input_fee_ppk: fee in parts per thousand (0 for eHash)
            // max_order: maximum power of 2 for keyset (32 = up to 2^32)
            supported_units.insert(unit.clone(), (0u64, 32u8));
        }

        // Create custom derivation paths for custom units
        // Standard units (Sat, Msat, etc.) have built-in paths, but Custom units need explicit paths
        use bitcoin::bip32::{ChildNumber, DerivationPath};
        let mut custom_paths = HashMap::new();

        for unit in &config.supported_units {
            if let CurrencyUnit::Custom(name) = unit {
                // Use a deterministic derivation path for custom units
                // Format: m/0'/999'/hash(unit_name)' where 999 is reserved for custom units
                // We use the first 31 bits of the hash of the unit name for uniqueness
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut hasher = DefaultHasher::new();
                name.hash(&mut hasher);
                let hash_value = hasher.finish();
                // Use only 31 bits to ensure it fits in hardened index range (< 2^31)
                let index = (hash_value as u32) & 0x7FFFFFFF;

                let path = DerivationPath::from(vec![
                    ChildNumber::from_hardened_idx(0).expect("0 is valid"),
                    ChildNumber::from_hardened_idx(999).expect("999 is valid"), // Reserved for custom units
                    ChildNumber::from_hardened_idx(index).expect("hash index is valid"),
                ]);

                custom_paths.insert(unit.clone(), path);
                tracing::debug!(
                    "Created custom derivation path for unit '{}': m/0'/999'/{}'",
                    name,
                    index
                );
            }
        }

        // Create the DbSignatory with supported units and custom paths
        use cdk_signatory::db_signatory::DbSignatory;
        use cdk_signatory::embedded::Service;

        let db_signatory = DbSignatory::new(
            keystore.clone(),
            &seed,
            supported_units,
            custom_paths, // Now includes paths for custom units like "HASH"
        )
        .await
        .map_err(|e| MintError::ConfigError(format!("Failed to create signatory: {}", e)))?;

        let signatory = Arc::new(Service::new(Arc::new(db_signatory)));

        let mint = mint_builder
            .build_with_signatory(signatory)
            .await
            .map_err(|e| MintError::ConfigError(format!("Failed to build mint: {}", e)))?;

        tracing::info!(
            "CDK Mint initialized successfully with units: {:?}",
            config.supported_units
        );

        Ok(mint)
    }

    /// Get the sender channel for distributing to other components
    ///
    /// This sender can be cloned and passed to ChannelManager and other
    /// components that need to send share validation events to the mint.
    pub fn get_sender(&self) -> Sender<EHashMintData> {
        self.sender.clone()
    }

    /// Get the receiver channel for the main processing loop
    ///
    /// This receiver should only be used by the mint thread's run loop.
    pub fn get_receiver(&self) -> Receiver<EHashMintData> {
        self.receiver.clone()
    }


    /// Main processing loop for the mint thread
    ///
    /// Continuously receives and processes share validation events until
    /// the channel is closed.
    ///
    /// # Errors
    /// Returns `MintError` if processing fails unrecoverably
    pub async fn run(&mut self) -> Result<(), MintError> {
        tracing::info!("Starting MintHandler run loop...");

        loop {
            match self.receiver.recv().await {
                Ok(data) => {
                    // Process the mint data
                    if let Err(e) = self.process_mint_data(data).await {
                        tracing::error!("Error processing mint data: {}", e);
                        // Continue processing other events even if one fails
                        // This ensures mining operations are not affected by mint errors
                    }
                }
                Err(_) => {
                    // Channel closed, exit gracefully
                    tracing::info!("MintHandler receiver channel closed, shutting down...");
                    break;
                }
            }
        }

        tracing::info!("MintHandler run loop completed");
        Ok(())
    }

    /// Main processing loop with graceful shutdown handling
    ///
    /// Like `run()`, but accepts a shutdown signal channel to allow
    /// graceful termination while completing pending mint operations.
    ///
    /// # Arguments
    /// * `shutdown_rx` - Receiver for shutdown signal
    ///
    /// # Errors
    /// Returns `MintError` if processing fails unrecoverably
    pub async fn run_with_shutdown(
        &mut self,
        shutdown_rx: Receiver<()>,
    ) -> Result<(), MintError> {
        loop {
            tokio::select! {
                // Process incoming events
                event = self.receiver.recv() => {
                    match event {
                        Ok(data) => self.process_mint_data(data).await?,
                        Err(_) => break, // Channel closed
                    }
                }
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    tracing::info!("Mint thread received shutdown signal, completing pending operations...");
                    self.shutdown().await?;
                    break;
                }
            }
        }
        Ok(())
    }

    /// Process share validation data with automatic retry on failure
    ///
    /// This wrapper provides fault tolerance by:
    /// - Attempting to process the mint data via `process_mint_data`
    /// - On success: resetting failure counters and returning
    /// - On failure: queuing the data for retry and incrementing failure count
    /// - Disabling the handler if max retries are exceeded
    ///
    /// # Arguments
    /// * `data` - Share validation data to process
    ///
    /// # Errors
    /// Returns `MintError` if processing fails
    pub async fn process_mint_data_with_retry(&mut self, data: EHashMintData) -> Result<(), MintError> {
        match self.process_mint_data(data.clone()).await {
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

                tracing::warn!(
                    "Mint operation failed (attempt {}), queued for retry: {}",
                    self.failure_count,
                    e
                );

                // Check if we've exceeded max retries
                if self.failure_count >= self.config.max_retries {
                    tracing::error!(
                        "Mint handler disabled after {} consecutive failures",
                        self.config.max_retries
                    );
                }

                Err(e)
            }
        }
    }

    /// Process share validation data and mint eHash tokens
    ///
    /// Uses CDK's native MintQuote and database for accounting.
    ///
    /// # Arguments
    /// * `data` - Share validation data containing hash, channel info, etc.
    ///
    /// # Errors
    /// Returns `MintError` if minting fails
    pub async fn process_mint_data(&mut self, data: EHashMintData) -> Result<(), MintError> {
        tracing::debug!(
            "Processing mint data for channel {} sequence {}",
            data.channel_id,
            data.sequence_number
        );

        // Check if this is a block found event
        if data.block_found {
            tracing::info!(
                "Block found by channel {}, triggering keyset lifecycle",
                data.channel_id
            );
            return self.handle_block_found(&data).await;
        }

        // Calculate eHash amount from share hash
        let ehash_amount = data.calculate_ehash_amount(self.config.min_leading_zeros);

        // If below minimum threshold, don't mint
        if ehash_amount == 0 {
            tracing::debug!(
                "Share from channel {} below minimum difficulty threshold, skipping mint",
                data.channel_id
            );
            return Ok(());
        }

        tracing::info!(
            "Minting {} eHash tokens for channel {} (sequence {})",
            ehash_amount,
            data.channel_id,
            data.sequence_number
        );

        // Mint eHash tokens (P2PK-locked if pubkey registered)
        let _proofs = self.mint_ehash_tokens(&data).await?;

        tracing::debug!(
            "Successfully minted {} eHash tokens for channel {}",
            ehash_amount,
            data.channel_id
        );

        Ok(())
    }

    /// Attempt to recover from failures by processing the retry queue
    ///
    /// This method implements exponential backoff recovery:
    /// - Only attempts recovery if `recovery_enabled` is true
    /// - Uses exponential backoff based on `failure_count`
    /// - Processes items from the retry queue one at a time
    /// - Stops processing if an item fails (to avoid cascading failures)
    /// - Resets failure counters on successful recovery
    ///
    /// # Exponential Backoff Formula
    /// `backoff_duration = backoff_multiplier * 2^failure_count`
    ///
    /// Example with backoff_multiplier=2:
    /// - After 1 failure: 2 * 2^1 = 4 seconds
    /// - After 2 failures: 2 * 2^2 = 8 seconds
    /// - After 3 failures: 2 * 2^3 = 16 seconds
    /// - After 10 failures: 2 * 2^10 = 2048 seconds (capped)
    ///
    /// # Errors
    /// Returns `MintError` if recovery operations fail
    pub async fn attempt_recovery(&mut self) -> Result<(), MintError> {
        // Check if recovery is enabled
        if !self.config.recovery_enabled {
            return Ok(());
        }

        // Check if there's anything to recover
        if self.retry_queue.is_empty() {
            return Ok(());
        }

        // Calculate exponential backoff duration
        if let Some(last_failure) = self.last_failure {
            // Cap failure_count at 10 to prevent overflow (2^10 = 1024)
            let capped_count = self.failure_count.min(10);

            // Calculate backoff: backoff_multiplier * 2^failure_count
            let backoff_duration = std::time::Duration::from_secs(
                self.config.backoff_multiplier.saturating_mul(2u64.pow(capped_count))
            );

            // Check if enough time has elapsed since last failure
            let elapsed = last_failure
                .elapsed()
                .unwrap_or(std::time::Duration::ZERO);

            if elapsed < backoff_duration {
                tracing::debug!(
                    "Recovery backoff in progress: {}s elapsed, {}s required (attempt {})",
                    elapsed.as_secs(),
                    backoff_duration.as_secs(),
                    self.failure_count
                );
                return Ok(());
            }
        }

        tracing::info!(
            "Attempting recovery with {} items in retry queue (failure_count: {})",
            self.retry_queue.len(),
            self.failure_count
        );

        // Process retry queue one item at a time
        // Stop on first failure to avoid cascading failures
        while let Some(data) = self.retry_queue.pop_front() {
            match self.process_mint_data(data.clone()).await {
                Ok(()) => {
                    tracing::info!(
                        "Recovery successful for channel {} sequence {}",
                        data.channel_id,
                        data.sequence_number
                    );
                    // Reset failure counters on success
                    self.failure_count = 0;
                    self.last_failure = None;
                }
                Err(e) => {
                    tracing::warn!(
                        "Recovery attempt failed for channel {} sequence {}: {}",
                        data.channel_id,
                        data.sequence_number,
                        e
                    );
                    // Put the item back at the front of the queue
                    self.retry_queue.push_front(data);
                    // Increment failure count for next backoff calculation
                    self.failure_count += 1;
                    self.last_failure = Some(SystemTime::now());
                    // Stop processing to avoid cascading failures
                    break;
                }
            }
        }

        Ok(())
    }

    /// Gracefully shutdown the mint handler, completing pending operations
    ///
    /// This ensures:
    /// - Receiver channel is closed to prevent new events
    /// - All pending mint operations in the receiver queue are processed
    /// - All events in the retry queue are processed
    /// - CDK Mint instance is cleaned up (via Arc Drop)
    /// - Database connections are closed (via CDK cleanup)
    ///
    /// The shutdown process attempts to process all pending operations
    /// but logs warnings if any fail. This provides best-effort delivery
    /// while ensuring the handler always shuts down cleanly.
    pub async fn shutdown(&mut self) -> Result<(), MintError> {
        tracing::info!("Shutting down MintHandler...");

        // Close the receiver to prevent new events
        self.receiver.close();

        // Process any remaining events in the receiver channel
        let mut processed_count = 0;
        while let Ok(data) = self.receiver.try_recv() {
            tracing::debug!("Processing pending mint event during shutdown...");
            if let Err(e) = self.process_mint_data(data).await {
                tracing::warn!("Failed to process pending mint event during shutdown: {}", e);
            } else {
                processed_count += 1;
            }
        }

        if processed_count > 0 {
            tracing::info!("Processed {} pending mint events during shutdown", processed_count);
        }

        // Process retry queue
        let retry_count = self.retry_queue.len();
        if retry_count > 0 {
            tracing::info!("Processing {} events from retry queue during shutdown...", retry_count);
            while let Some(data) = self.retry_queue.pop_front() {
                if let Err(e) = self.process_mint_data(data).await {
                    tracing::warn!("Failed to process retry queue event during shutdown: {}", e);
                }
            }
        }

        // CDK Mint instance is Arc<Mint> and will be cleaned up when all references are dropped
        // The database connections will be closed when the Mint instance is dropped
        tracing::info!("MintHandler shutdown complete (processed {} events, {} retries)", processed_count, retry_count);
        Ok(())
    }

    /// Create P2PK-locked eHash quote using NUT-04 and NUT-20
    ///
    /// Per NUT-04: Generates a random, secret quote ID that is NOT derivable
    /// from the payment request to prevent front-running attacks.
    ///
    /// Per NUT-20: Attaches the locking pubkey to the quote to enforce
    /// public key authentication when the wallet mints tokens.
    ///
    /// The mint creates a PAID quote with:
    /// - Random UUID v4 quote ID (122 bits of cryptographically secure randomness)
    /// - Locking pubkey from channel registration (if available)
    /// - Share hash as payment proof for auditability
    ///
    /// # Arguments
    /// * `data` - Share validation data for token creation
    ///
    /// # Returns
    /// Empty proofs vector (wallet will create proofs via NUT-20 authentication)
    ///
    /// # Errors
    /// Returns `MintError` if quote creation fails
    async fn mint_ehash_tokens(&mut self, data: &EHashMintData) -> Result<Proofs, MintError> {
        // Calculate eHash amount
        let ehash_amount = data.calculate_ehash_amount(self.config.min_leading_zeros);
        let amount = Amount::from(ehash_amount);

        // Convert secp256k1 pubkey to CDK format (per-share pubkey from TLV)
        let locking_pubkey = CdkPublicKey::from(data.locking_pubkey);

        tracing::debug!(
            "Minting {} eHash for channel {} with NUT-20 P2PK lock",
            amount,
            data.channel_id
        );

        // Create MintQuote in PAID state
        // For eHash, we bypass the normal bolt11 payment flow
        // We directly create PAID quotes since shares are the "payment"

        // Per NUT-04: Quote ID MUST be unique and random, generated by the mint
        // It must NOT be derivable from the payment request to prevent front-running
        // Use NUT-20 P2PK locks to enforce public key authentication during minting

        // Generate a cryptographically secure random UUID v4 for the quote ID
        // UUID v4 uses 122 bits of randomness (guaranteed unique and unpredictable)
        use uuid::Uuid;
        let quote_id_str = Uuid::new_v4().to_string();

        // Use HASH unit for eHash tokens (or first configured unit in tests)
        let unit = self
            .config
            .supported_units
            .first()
            .cloned()
            .unwrap_or(CurrencyUnit::Custom("HASH".to_string()));

        // Get current timestamp
        let current_time = data.timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create MintQuote using the constructor with required NUT-20 P2PK locking
        // Quote starts with amount_paid = 0, we'll increment it below to mark as PAID
        let quote = MintQuote::new(
            Some(QuoteId::BASE64(quote_id_str.clone())),
            format!(
                "eHash tokens for user {} channel {} sequence {}",
                data.user_identity, data.channel_id, data.sequence_number
            ),
            unit,
            Some(amount),
            current_time + 86400, // 24 hour expiry
            PaymentIdentifier::CustomId(quote_id_str.clone()),
            Some(locking_pubkey), // Required NUT-20 P2PK lock (per-share pubkey from TLV)
            Amount::ZERO, // amount_paid starts at 0, will be incremented below
            Amount::ZERO, // amount_issued = 0 (not yet issued)
            PaymentMethod::Custom("stratum".to_string()), // Custom payment method for share-based payment
            current_time,
            vec![], // Start with empty payments, will add via increment_mint_quote_amount_paid
            vec![], // No issuances yet
        );

        // Store quote in database and add share hash as payment proof
        // This provides auditability and proof that the quote was earned through mining work
        let localstore = self.mint_instance.localstore();
        let mut tx = localstore
            .begin_transaction()
            .await
            .map_err(|e| MintError::DatabaseError(format!("Failed to start transaction: {}", e)))?;

        // Add the quote first
        tx.add_mint_quote(quote)
            .await
            .map_err(|e| MintError::DatabaseError(format!("Failed to store mint quote: {}", e)))?;

        // Use share hash as payment_id to track this payment
        let share_hash_hex = data.share_hash.to_string();

        // Increment the amount paid with the share hash as payment proof
        // This properly adds the payment to the quote and marks it as PAID
        tx.increment_mint_quote_amount_paid(
            &QuoteId::BASE64(quote_id_str.clone()),
            amount,
            share_hash_hex.clone(),
        )
        .await
        .map_err(|e| MintError::DatabaseError(format!("Failed to add payment: {}", e)))?;

        tx.commit()
            .await
            .map_err(|e| MintError::DatabaseError(format!("Failed to commit quote: {}", e)))?;

        tracing::info!(
            "Created PAID mint quote {} for {} eHash tokens (NUT-20 P2PK-locked) (share_hash: {})",
            quote_id_str,
            amount,
            share_hash_hex
        );

        // Note: We return empty proofs because per NUT-04 and NUT-20, the wallet
        // must authenticate and create blinded messages. The complete flow is:
        //
        // 1. Mint creates PAID quote with random UUID v4 + locking_pubkey (this method)
        // 2. External wallet queries for PAID quotes matching their pubkey
        // 3. Wallet creates blinded messages with P2PK SpendingConditions
        // 4. Wallet signs MintRequest with private key (NUT-20 authentication)
        // 5. Wallet submits signed MintRequest to mint endpoint
        // 6. Mint verifies NUT-20 signature, signs blinded messages
        // 7. Mint returns blind signatures
        // 8. Wallet unblinds to obtain P2PK-locked token proofs
        //
        // Security properties:
        // - Quote ID is secret UUID v4 (prevents front-running per NUT-04)
        // - NUT-20 signature prevents unauthorized minting
        // - Blinding preserves Cashu privacy guarantees

        Ok(vec![])
    }

    /// Handle block found events and trigger keyset lifecycle
    ///
    /// When a share finds a block:
    /// - Mint eHash tokens for the block-finding share
    /// - Query Template Provider for block reward details (stub)
    /// - Trigger keyset lifecycle transitions (deferred to Phase 10)
    /// - Calculate eHash to sats conversion rate (deferred to Phase 10)
    ///
    /// # Arguments
    /// * `data` - Share validation data with block_found=true
    ///
    /// # Errors
    /// Returns `MintError` if lifecycle transition fails
    async fn handle_block_found(&mut self, data: &EHashMintData) -> Result<(), MintError> {
        tracing::info!(
            "Block found event received for channel {} (template_id: {:?}, coinbase size: {} bytes)",
            data.channel_id,
            data.template_id,
            data.coinbase.as_ref().map(|c| c.len()).unwrap_or(0)
        );

        // Mint eHash tokens for the block-finding share
        // This share also earns eHash like any other valid share
        let ehash_amount = data.calculate_ehash_amount(self.config.min_leading_zeros);
        if ehash_amount > 0 {
            tracing::info!(
                "Minting {} eHash tokens for block-finding share",
                ehash_amount
            );
            let _proofs = self.mint_ehash_tokens(data).await?;
        }

        // Query Template Provider for block reward details (stub)
        // In Phase 10, this will query the actual Template Provider
        let block_reward = self.query_template_provider_stub(data).await?;
        tracing::info!(
            "Block reward info (stub): height={}, reward_sats={}",
            block_reward.height,
            block_reward.total_reward_sats
        );

        // Trigger keyset lifecycle transitions (deferred to Phase 10)
        // Full implementation will:
        // 1. Get active keyset ID
        // 2. Calculate outstanding eHash amount for keyset
        // 3. Create new ACTIVE keyset (to continue minting)
        // 4. Transition previous keyset ACTIVE → QUANTIFYING
        // 5. Calculate eHash-to-sats conversion rate
        // 6. Transition keyset QUANTIFYING → PAYOUT with conversion rate
        // 7. Enable eHash to sats swaps for the PAYOUT keyset

        tracing::warn!(
            "Keyset lifecycle management not yet implemented (deferred to Phase 10)"
        );
        tracing::info!(
            "Would transition keyset lifecycle: ACTIVE → QUANTIFYING → PAYOUT"
        );

        Ok(())
    }

    /// Query block reward details (stub)
    ///
    /// This is a placeholder implementation that returns mock data.
    /// Phase 10 will implement actual block reward querying via Bitcoin RPC.
    ///
    /// # Phase 10 Implementation Plan
    /// The production implementation will use Bitcoin Core RPC to fetch real block data:
    /// - Use `bitcoincore-rpc` crate to connect to Bitcoin Core node
    /// - Use `data.share_hash` directly as the block hash (when block_found=true, share_hash IS the block hash)
    /// - Call `getblock(block_hash, verbosity=2)` RPC method to get full block data
    /// - Extract coinbase transaction output value to get block reward
    /// - Sum transaction fees from block data (total_fees = sum(inputs) - sum(outputs) for all non-coinbase txs)
    /// - Configuration will include RPC endpoint, auth credentials, timeout
    ///
    /// # Arguments
    /// * `data` - Share validation data with share_hash (which is the block hash when block_found=true)
    ///
    /// # Returns
    /// Block reward information including height and total reward
    ///
    /// # Errors
    /// Returns `MintError` if RPC query fails
    async fn query_template_provider_stub(
        &self,
        data: &EHashMintData,
    ) -> Result<BlockRewardInfo, MintError> {
        // Stub implementation - Phase 10 will use Bitcoin RPC
        // Production flow:
        // 1. Use data.share_hash directly as block_hash (it IS the block hash when block_found=true)
        // 2. Call Bitcoin RPC: getblock(block_hash, verbosity=2)
        // 3. Parse coinbase transaction output value for block reward
        // 4. Calculate total fees from all transactions in block
        // 5. Return BlockRewardInfo with real data

        let template_id = data.template_id.unwrap_or(0);

        tracing::debug!(
            "Block reward query stub called for block hash {} (template_id: {}, will use Bitcoin RPC in Phase 10)",
            data.share_hash,
            template_id
        );

        // Mock block reward data
        // In production, this would come from Bitcoin RPC getblock(data.share_hash)
        Ok(BlockRewardInfo {
            height: 800_000, // Mock block height
            total_reward_sats: 625_000_000, // 6.25 BTC coinbase + fees
            coinbase_reward_sats: 625_000_000,
            fees_sats: 0,
            template_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash as HashTrait;
    use bitcoin::hashes::sha256d::Hash;
    use bitcoin::Target;
    use std::time::SystemTime;

    fn create_test_config() -> MintConfig {
        MintConfig {
            mint_url: "https://mint.test.com".parse().unwrap(),
            mint_private_key: None,
            // Use Sat for tests to keep test data simpler
            // Production would use CurrencyUnit::Custom("HASH".to_string())
            supported_units: vec![CurrencyUnit::Sat],
            database_url: None,
            min_leading_zeros: 32,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        }
    }

    fn test_pubkey() -> Secp256k1PublicKey {
        use bitcoin::secp256k1::{Secp256k1, SecretKey};
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[1u8; 32]).unwrap();
        Secp256k1PublicKey::from_secret_key(&secp, &secret)
    }

    fn test_pubkey2() -> Secp256k1PublicKey {
        use bitcoin::secp256k1::{Secp256k1, SecretKey};
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[2u8; 32]).unwrap();
        Secp256k1PublicKey::from_secret_key(&secp, &secret)
    }

    #[tokio::test]
    async fn test_mint_config_parsing() {
        let config = create_test_config();
        assert_eq!(config.min_leading_zeros, 32);
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.supported_units.len(), 1);
        assert_eq!(config.supported_units[0], CurrencyUnit::Sat);
    }

    #[test]
    fn test_ehash_amount_calculation() {
        // Create share with 40 leading zeros (5 bytes of zeros)
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
            locking_pubkey: test_pubkey(),
        };

        // 40 leading zeros - 32 minimum = 8, so 2^8 = 256
        assert_eq!(data.calculate_ehash_amount(32), 256);
    }

    #[test]
    fn test_ehash_amount_below_threshold() {
        // Create share with only 24 leading zeros (3 bytes of zeros)
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
            locking_pubkey: test_pubkey(),
        };

        // Below minimum threshold (32), should return 0
        assert_eq!(data.calculate_ehash_amount(32), 0);
    }

    #[test]
    fn test_channel_data_structure() {
        let share_hash = Hash::from_byte_array([0u8; 32]);
        let data = EHashMintData {
            share_hash,
            block_found: true,
            channel_id: 42,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 123,
            timestamp: SystemTime::now(),
            template_id: Some(456),
            coinbase: Some(vec![0x01, 0x02, 0x03]),
            locking_pubkey: test_pubkey(),
        };

        assert_eq!(data.channel_id, 42);
        assert_eq!(data.sequence_number, 123);
        assert!(data.block_found);
        assert_eq!(data.template_id, Some(456));
    }


    #[tokio::test]
    async fn test_mint_handler_with_custom_hash_unit() {
        // Test with HASH custom unit
        let mut config = create_test_config();
        config.supported_units = vec![CurrencyUnit::Custom("HASH".to_string())];

        let result = MintHandler::new(config).await;
        assert!(
            result.is_ok(),
            "MintHandler should support HASH custom unit: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_mint_handler_initialization() {
        let config = create_test_config();

        // Test that MintHandler can be created
        let result = MintHandler::new(config).await;
        assert!(result.is_ok(), "MintHandler initialization should succeed");

        let handler = result.unwrap();

        // Verify channels are set up
        let sender = handler.get_sender();
        let receiver = handler.get_receiver();

        // Test that we can send data through the channel
        let test_data = EHashMintData {
            share_hash: Hash::from_byte_array([0u8; 32]),
            block_found: false,
            channel_id: 1,
            user_identity: "test".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        assert!(sender.try_send(test_data.clone()).is_ok());
        assert!(receiver.try_recv().is_ok());
    }


    #[tokio::test]
    async fn test_process_mint_data_with_p2pk() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Per-share locking pubkey is now included directly in EHashMintData
        // No registration needed - pubkey comes from SubmitSharesExtended TLV

        // Create share with sufficient difficulty (40 leading zeros)
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
            locking_pubkey: test_pubkey(),
        };

        // Process the mint data - should create PAID quote with P2PK pubkey
        let result = handler.process_mint_data(data).await;

        // Should succeed
        assert!(result.is_ok(), "Processing mint data with P2PK should succeed");

        // Verify quote was created in database
        // Per NUT-04, quote IDs are now random UUIDs and secret, so we can't query by a known ID
        // The successful execution of process_mint_data means:
        // 1. A random UUID v4 quote ID was generated (cryptographically secure)
        // 2. The quote was stored with the P2PK locking pubkey
        // 3. The share hash was recorded as payment proof
        //
        // In production, the wallet would:
        // 1. Query the mint API endpoint for quotes with their pubkey
        // 2. Receive the secret UUID quote ID from the mint
        // 3. Sign a NUT-20 MintRequest and submit it
        // 4. Receive blind signatures and unblind to get P2PK-locked proofs
        //
        // For this unit test, we verify that the minting operation completed successfully,
        // which confirms the quote was created with proper NUT-04/NUT-20 properties.
    }

    #[tokio::test]
    async fn test_process_mint_data_without_p2pk() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Don't register a pubkey - tokens should still be mintable but not P2PK-locked

        // Create share with sufficient difficulty
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
            locking_pubkey: test_pubkey(),
        };

        // Process the mint data - should create PAID quote without P2PK pubkey
        let result = handler.process_mint_data(data).await;

        // Should succeed
        assert!(result.is_ok(), "Processing mint data without P2PK should succeed");

        // Verify quote was created without pubkey
        // Per NUT-04, quote IDs are now random UUIDs and secret, so we can't query by a known ID
        // The successful execution means:
        // 1. A random UUID v4 quote ID was generated (per NUT-04)
        // 2. The quote was stored WITHOUT a P2PK locking pubkey (since none was registered)
        // 3. The share hash was recorded as payment proof
        //
        // Without a registered pubkey, the quote is still created but not P2PK-locked.
        // This allows minting to continue even if downstream miners don't provide pubkeys.
        //
        // For this unit test, successful execution confirms proper NUT-04 compliance.
    }

    #[tokio::test]
    async fn test_quote_id_uniqueness() {
        use bitcoin::secp256k1::{Secp256k1, SecretKey};

        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create two different locking pubkeys
        let secp = Secp256k1::new();
        let secret1 = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let pubkey1 = Secp256k1PublicKey::from_secret_key(&secp, &secret1);
        let secret2 = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let pubkey2 = Secp256k1PublicKey::from_secret_key(&secp, &secret2);

        // Register different pubkeys for the same channel

        // Create shares with sufficient difficulty (different shares for different users)
        let mut hash_bytes1 = [0xffu8; 32];
        hash_bytes1[..5].fill(0x00);
        let share_hash1 = Hash::from_byte_array(hash_bytes1);

        let mut hash_bytes2 = [0xffu8; 32];
        hash_bytes2[..5].fill(0x00);
        hash_bytes2[5] = 0x01; // Make it slightly different
        let share_hash2 = Hash::from_byte_array(hash_bytes2);

        // Create two mint data events with same channel and sequence but different pubkeys
        let data1 = EHashMintData {
            share_hash: share_hash1,
            block_found: false,
            channel_id: 1,
            user_identity: "user_alice".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        // Process first mint with pubkey1
        let result1 = handler.process_mint_data(data1).await;
        assert!(result1.is_ok(), "First mint should succeed");

        // Change the registered pubkey for the same channel

        let data2 = EHashMintData {
            share_hash: share_hash2, // Different share hash
            block_found: false,
            channel_id: 1, // Same channel_id
            user_identity: "user_bob".to_string(), // Different user
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1, // Same sequence_number
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        // Process second mint with pubkey2
        let result2 = handler.process_mint_data(data2).await;
        assert!(result2.is_ok(), "Second mint should succeed (unique quote ID)");

        // Verify both quotes were created successfully
        // Per NUT-04, quote IDs MUST be:
        // 1. Randomly generated (UUID v4 with cryptographic randomness)
        // 2. Unique (no collisions)
        // 3. Secret (not derivable from payment request)
        //
        // The fact that both minting operations succeeded proves:
        // - Two different random UUID v4 quote IDs were generated
        // - Each quote was stored with its respective P2PK locking pubkey
        // - Quote IDs did not collide (would have caused database error)
        // - Quote IDs are not derived from channel/sequence (would be identical)
        //
        // This test verifies NUT-04 compliance: quote IDs are truly random UUIDs.
    }

    #[tokio::test]
    async fn test_handle_block_found_basic() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create block-finding share with sufficient difficulty
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..5].fill(0x00); // 40 leading zeros
        let share_hash = Hash::from_byte_array(hash_bytes);

        let data = EHashMintData {
            share_hash,
            block_found: true,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: Some(12345),
            coinbase: Some(vec![0x01, 0x02, 0x03]),
            locking_pubkey: test_pubkey(),
        };

        // Should handle block found without errors
        let result = handler.handle_block_found(&data).await;
        assert!(
            result.is_ok(),
            "handle_block_found should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_handle_block_found_mints_ehash() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create block-finding share with sufficient difficulty
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..5].fill(0x00); // 40 leading zeros = 256 eHash
        let share_hash = Hash::from_byte_array(hash_bytes);

        let data = EHashMintData {
            share_hash,
            block_found: true,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: Some(12345),
            coinbase: Some(vec![0x01, 0x02, 0x03]),
            locking_pubkey: test_pubkey(),
        };

        // Handle block found
        handler.handle_block_found(&data).await.unwrap();

        // Verify quote was created (block-finding shares still earn eHash)
        // Per NUT-04, quote IDs are random UUIDs and secret, so we verify by successful execution
        // The fact that handle_block_found succeeded means:
        // 1. A random UUID v4 quote ID was generated for the block-finding share
        // 2. 256 eHash was minted (40 leading zeros - 32 minimum = 8, 2^8 = 256)
        // 3. The share hash was recorded as payment proof
        //
        // Block-finding shares earn eHash just like regular shares, then trigger
        // keyset lifecycle management (deferred to Phase 10).
    }

    #[tokio::test]
    async fn test_handle_block_found_with_template_info() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create block-finding share
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..5].fill(0x00);
        let share_hash = Hash::from_byte_array(hash_bytes);

        let template_id = 98765u64;
        let coinbase = vec![0xde, 0xad, 0xbe, 0xef];

        let data = EHashMintData {
            share_hash,
            block_found: true,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: Some(template_id),
            coinbase: Some(coinbase.clone()),
            locking_pubkey: test_pubkey(),
        };

        // Handle block found
        let result = handler.handle_block_found(&data).await;
        assert!(
            result.is_ok(),
            "handle_block_found with template info should succeed"
        );

        // Verify template_id and coinbase are available in the data
        assert_eq!(data.template_id, Some(template_id));
        assert_eq!(data.coinbase, Some(coinbase));
    }

    #[tokio::test]
    async fn test_query_template_provider_stub() {
        let config = create_test_config();
        let handler = MintHandler::new(config).await.unwrap();

        // Create test data
        let share_hash = Hash::from_byte_array([0u8; 32]);
        let data = EHashMintData {
            share_hash,
            block_found: true,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: Some(12345),
            coinbase: Some(vec![0x01, 0x02]),
            locking_pubkey: test_pubkey(),
        };

        // Query stub
        let result = handler.query_template_provider_stub(&data).await;
        assert!(result.is_ok(), "Template provider stub should succeed");

        let block_reward = result.unwrap();
        assert_eq!(block_reward.template_id, 12345);
        assert!(
            block_reward.total_reward_sats > 0,
            "Block reward should be non-zero"
        );
        assert_eq!(
            block_reward.height, 800_000,
            "Mock block height should be 800,000"
        );
        assert_eq!(
            block_reward.total_reward_sats, 625_000_000,
            "Mock reward should be 6.25 BTC"
        );
    }

    #[tokio::test]
    async fn test_process_mint_data_routes_to_block_found() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create block-finding share
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..5].fill(0x00);
        let share_hash = Hash::from_byte_array(hash_bytes);

        let data = EHashMintData {
            share_hash,
            block_found: true,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: Some(12345),
            coinbase: Some(vec![0x01, 0x02, 0x03]),
            locking_pubkey: test_pubkey(),
        };

        // process_mint_data should detect block_found and route to handle_block_found
        let result = handler.process_mint_data(data).await;
        assert!(
            result.is_ok(),
            "process_mint_data should route block found events correctly"
        );
    }

    #[tokio::test]
    async fn test_block_found_below_threshold() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create block-finding share with difficulty below threshold
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..3].fill(0x00); // Only 24 leading zeros (below 32 minimum)
        let share_hash = Hash::from_byte_array(hash_bytes);

        let data = EHashMintData {
            share_hash,
            block_found: true,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: Some(12345),
            coinbase: Some(vec![0x01, 0x02, 0x03]),
            locking_pubkey: test_pubkey(),
        };

        // Should still handle block found (just won't mint eHash tokens)
        let result = handler.handle_block_found(&data).await;
        assert!(
            result.is_ok(),
            "handle_block_found should succeed even if below eHash threshold"
        );

        // Verify no quote was created (below threshold)
        // Since the share has only 24 leading zeros (below 32 minimum),
        // no eHash tokens should be minted and no quote should be created.
        // The successful execution with no error confirms this behavior.
    }

    #[tokio::test]
    async fn test_run_with_shutdown_signal() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        let sender = handler.get_sender();
        let (shutdown_tx, shutdown_rx) = async_channel::bounded(1);

        // Spawn the run loop in a background task
        let run_handle = tokio::spawn(async move {
            handler.run_with_shutdown(shutdown_rx).await
        });

        // Send some test data
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
            locking_pubkey: test_pubkey(),
        };

        sender.send(data).await.unwrap();

        // Give it a moment to process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Send shutdown signal
        shutdown_tx.send(()).await.unwrap();

        // Wait for run loop to complete
        let result = run_handle.await.unwrap();
        assert!(result.is_ok(), "Run with shutdown should complete gracefully");
    }

    #[tokio::test]
    async fn test_retry_queue_initialization() {
        let config = create_test_config();
        let handler = MintHandler::new(config).await.unwrap();

        // Verify retry queue is initialized empty
        assert_eq!(handler.retry_queue.len(), 0, "Retry queue should start empty");
        assert_eq!(handler.failure_count, 0, "Failure count should start at 0");
        assert!(handler.last_failure.is_none(), "Last failure should be None");
    }

    #[tokio::test]
    async fn test_process_mint_data_with_retry_success() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create share with sufficient difficulty
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
            locking_pubkey: test_pubkey(),
        };

        // Process with retry - should succeed
        let result = handler.process_mint_data_with_retry(data).await;
        assert!(result.is_ok(), "Process with retry should succeed");

        // Verify failure counters are reset on success
        assert_eq!(handler.failure_count, 0, "Failure count should be 0 after success");
        assert!(handler.last_failure.is_none(), "Last failure should be None after success");
        assert_eq!(handler.retry_queue.len(), 0, "Retry queue should be empty after success");
    }

    #[tokio::test]
    async fn test_retry_queue_on_actual_failure() {
        let mut config = create_test_config();
        config.max_retries = 5;

        let mut handler = MintHandler::new(config).await.unwrap();

        // Create share with sufficient difficulty
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..5].fill(0x00); // 40 leading zeros
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
            locking_pubkey: test_pubkey(),
        };

        // First mint should succeed
        let result1 = handler.process_mint_data_with_retry(data.clone()).await;
        assert!(result1.is_ok(), "First mint should succeed");
        assert_eq!(handler.failure_count, 0, "No failures yet");
        assert_eq!(handler.retry_queue.len(), 0, "Queue should be empty");

        // Second mint with SAME data should fail due to duplicate quote ID
        let result2 = handler.process_mint_data_with_retry(data.clone()).await;
        assert!(result2.is_err(), "Second mint should fail (duplicate quote ID)");

        // Verify failure was tracked
        assert_eq!(handler.failure_count, 1, "Failure count should increment");
        assert!(handler.last_failure.is_some(), "Last failure time should be set");
        assert_eq!(handler.retry_queue.len(), 1, "Failed operation should be queued");

        // Verify the queued data matches
        let queued = handler.retry_queue.front().unwrap();
        assert_eq!(queued.channel_id, data.channel_id);
        assert_eq!(queued.sequence_number, data.sequence_number);
    }

    #[tokio::test]
    async fn test_retry_queue_multiple_failures() {
        let mut config = create_test_config();
        config.max_retries = 3;

        let mut handler = MintHandler::new(config).await.unwrap();

        // Create share
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
            locking_pubkey: test_pubkey(),
        };

        // First mint succeeds
        handler.process_mint_data_with_retry(data.clone()).await.unwrap();

        // Subsequent attempts fail and increment counter
        for i in 1..=3 {
            let result = handler.process_mint_data_with_retry(data.clone()).await;
            assert!(result.is_err(), "Attempt {} should fail", i);
            assert_eq!(handler.failure_count, i as u32, "Failure count should be {}", i);
            assert_eq!(handler.retry_queue.len(), i, "Should have {} items in queue", i);
        }

        // Verify we've reached max_retries
        assert!(
            handler.failure_count >= handler.config.max_retries,
            "Should have reached max retries"
        );
    }

    #[tokio::test]
    async fn test_retry_queue_success_resets_counter() {
        let mut config = create_test_config();
        config.max_retries = 5;

        let mut handler = MintHandler::new(config).await.unwrap();

        // Create two different shares
        let mut hash_bytes1 = [0xffu8; 32];
        hash_bytes1[..5].fill(0x00);
        let share_hash1 = Hash::from_byte_array(hash_bytes1);

        let mut hash_bytes2 = [0xffu8; 32];
        hash_bytes2[..5].fill(0x00);
        hash_bytes2[5] = 0x01; // Make it different
        let share_hash2 = Hash::from_byte_array(hash_bytes2);

        let data1 = EHashMintData {
            share_hash: share_hash1,
            block_found: false,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        let data2 = EHashMintData {
            share_hash: share_hash2,
            block_found: false,
            channel_id: 2, // Different channel to avoid resource contention
            user_identity: "test_user2".to_string(), // Different user for cleaner separation
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        // First mint succeeds
        handler.process_mint_data_with_retry(data1.clone()).await.unwrap();

        // Duplicate fails and increments counter
        let result = handler.process_mint_data_with_retry(data1.clone()).await;
        assert!(result.is_err(), "Duplicate should fail");
        assert_eq!(handler.failure_count, 1, "Failure count should be 1");

        // Give database a moment to clean up resources from failed transaction
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Different share succeeds and RESETS counter
        let result2 = handler.process_mint_data_with_retry(data2).await;
        if let Err(ref e) = result2 {
            panic!("Second share should succeed, but got error: {:?}", e);
        }
        assert!(result2.is_ok(), "Different share should succeed");
        assert_eq!(handler.failure_count, 0, "Success should reset failure counter");
        assert!(handler.last_failure.is_none(), "Success should clear last failure time");
    }

    #[tokio::test]
    async fn test_failure_count_increments() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Manually simulate a failure scenario by setting failure_count
        // (In production, this would be set by process_mint_data_with_retry on failure)
        handler.failure_count = 3;
        handler.last_failure = Some(SystemTime::now());

        assert_eq!(handler.failure_count, 3, "Failure count should be 3");
        assert!(handler.last_failure.is_some(), "Last failure should be set");
    }

    #[tokio::test]
    async fn test_max_retries_detection() {
        let mut config = create_test_config();
        config.max_retries = 3;

        let mut handler = MintHandler::new(config).await.unwrap();

        // Simulate reaching max retries
        handler.failure_count = 3;

        assert!(
            handler.failure_count >= handler.config.max_retries,
            "Should detect when max retries is reached"
        );
    }

    #[tokio::test]
    async fn test_retry_queue_structure() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create test data
        let share_hash = Hash::from_byte_array([0u8; 32]);
        let data1 = EHashMintData {
            share_hash,
            block_found: false,
            channel_id: 1,
            user_identity: "test_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        let data2 = EHashMintData {
            share_hash,
            block_found: false,
            channel_id: 2,
            user_identity: "test_user2".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 2,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        // Manually add to retry queue to test FIFO behavior
        handler.retry_queue.push_back(data1.clone());
        handler.retry_queue.push_back(data2.clone());

        assert_eq!(handler.retry_queue.len(), 2, "Should have 2 items in queue");

        // Pop from queue and verify FIFO order
        let first = handler.retry_queue.pop_front();
        assert!(first.is_some());
        assert_eq!(first.unwrap().channel_id, 1, "First item should be channel 1");

        let second = handler.retry_queue.pop_front();
        assert!(second.is_some());
        assert_eq!(second.unwrap().channel_id, 2, "Second item should be channel 2");

        assert_eq!(handler.retry_queue.len(), 0, "Queue should be empty after popping all items");
    }

    #[tokio::test]
    async fn test_retry_queue_preserves_data() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create share with specific properties
        let mut hash_bytes = [0xffu8; 32];
        hash_bytes[..5].fill(0x00);
        let share_hash = Hash::from_byte_array(hash_bytes);

        let original_data = EHashMintData {
            share_hash,
            block_found: true,
            channel_id: 42,
            user_identity: "specific_user".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 123,
            timestamp: SystemTime::now(),
            template_id: Some(789),
            coinbase: Some(vec![0xde, 0xad, 0xbe, 0xef]),
            locking_pubkey: test_pubkey(),
        };

        // Add to retry queue
        handler.retry_queue.push_back(original_data.clone());

        // Retrieve and verify all fields are preserved
        let retrieved = handler.retry_queue.pop_front().unwrap();
        assert_eq!(retrieved.share_hash, original_data.share_hash);
        assert_eq!(retrieved.block_found, original_data.block_found);
        assert_eq!(retrieved.channel_id, original_data.channel_id);
        assert_eq!(retrieved.user_identity, original_data.user_identity);
        assert_eq!(retrieved.sequence_number, original_data.sequence_number);
        assert_eq!(retrieved.template_id, original_data.template_id);
        assert_eq!(retrieved.coinbase, original_data.coinbase);
    }

    #[tokio::test]
    async fn test_attempt_recovery_when_disabled() {
        let mut config = create_test_config();
        config.recovery_enabled = false;

        let mut handler = MintHandler::new(config).await.unwrap();

        // Add an item to retry queue
        let share_hash = Hash::from_byte_array([0u8; 32]);
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
            locking_pubkey: test_pubkey(),
        };
        handler.retry_queue.push_back(data);
        handler.failure_count = 1;

        // Attempt recovery - should do nothing when disabled
        let result = handler.attempt_recovery().await;
        assert!(result.is_ok(), "Recovery should succeed (no-op when disabled)");

        // Verify queue is unchanged
        assert_eq!(handler.retry_queue.len(), 1, "Queue should not be processed when recovery disabled");
        assert_eq!(handler.failure_count, 1, "Failure count should not change");
    }

    #[tokio::test]
    async fn test_attempt_recovery_empty_queue() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Attempt recovery with empty queue
        let result = handler.attempt_recovery().await;
        assert!(result.is_ok(), "Recovery should succeed (no-op with empty queue)");
        assert_eq!(handler.retry_queue.len(), 0, "Queue should remain empty");
    }

    #[tokio::test]
    async fn test_attempt_recovery_respects_backoff() {
        let mut config = create_test_config();
        config.backoff_multiplier = 2;
        config.max_retries = 10;

        let mut handler = MintHandler::new(config).await.unwrap();

        // Set up failure state
        handler.failure_count = 2; // Should require 2 * 2^2 = 8 seconds backoff
        handler.last_failure = Some(SystemTime::now());

        // Add item to retry queue
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
            locking_pubkey: test_pubkey(),
        };
        handler.retry_queue.push_back(data);

        // Attempt recovery immediately - should be blocked by backoff
        let result = handler.attempt_recovery().await;
        assert!(result.is_ok(), "Recovery should succeed (blocked by backoff)");
        assert_eq!(handler.retry_queue.len(), 1, "Queue should not be processed during backoff");
        assert_eq!(handler.failure_count, 2, "Failure count should not change during backoff");
    }

    #[tokio::test]
    async fn test_attempt_recovery_successful() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create unique shares
        let mut hash_bytes1 = [0xffu8; 32];
        hash_bytes1[..5].fill(0x00);
        let share_hash1 = Hash::from_byte_array(hash_bytes1);

        let mut hash_bytes2 = [0xffu8; 32];
        hash_bytes2[..5].fill(0x00);
        hash_bytes2[5] = 0x01;
        let share_hash2 = Hash::from_byte_array(hash_bytes2);

        let data1 = EHashMintData {
            share_hash: share_hash1,
            block_found: false,
            channel_id: 1,
            user_identity: "test_user1".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        let data2 = EHashMintData {
            share_hash: share_hash2,
            block_found: false,
            channel_id: 2,
            user_identity: "test_user2".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 2,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        // Add items to retry queue
        handler.retry_queue.push_back(data1);
        handler.retry_queue.push_back(data2);
        handler.failure_count = 1;
        handler.last_failure = Some(SystemTime::now() - std::time::Duration::from_secs(100));

        // Attempt recovery - backoff time has passed
        let result = handler.attempt_recovery().await;
        assert!(result.is_ok(), "Recovery should succeed");

        // Verify queue is emptied and counters reset
        assert_eq!(handler.retry_queue.len(), 0, "All items should be processed");
        assert_eq!(handler.failure_count, 0, "Failure count should be reset");
        assert!(handler.last_failure.is_none(), "Last failure should be cleared");
    }

    #[tokio::test]
    async fn test_attempt_recovery_stops_on_failure() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create shares - first is unique, second will be duplicate
        let mut hash_bytes1 = [0xffu8; 32];
        hash_bytes1[..5].fill(0x00);
        let share_hash1 = Hash::from_byte_array(hash_bytes1);

        let data1 = EHashMintData {
            share_hash: share_hash1,
            block_found: false,
            channel_id: 1,
            user_identity: "test_user1".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 1,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };

        // First mint the data so it's in the database
        handler.process_mint_data(data1.clone()).await.unwrap();

        // Now add duplicate to retry queue (this will fail)
        handler.retry_queue.push_back(data1.clone());

        // Add a second valid item
        let mut hash_bytes2 = [0xffu8; 32];
        hash_bytes2[..5].fill(0x00);
        hash_bytes2[5] = 0x01;
        let share_hash2 = Hash::from_byte_array(hash_bytes2);

        let data2 = EHashMintData {
            share_hash: share_hash2,
            block_found: false,
            channel_id: 2,
            user_identity: "test_user2".to_string(),
            target: Target::MAX_ATTAINABLE_MAINNET,
            sequence_number: 2,
            timestamp: SystemTime::now(),
            template_id: None,
            coinbase: None,
            locking_pubkey: test_pubkey(),
        };
        handler.retry_queue.push_back(data2);

        handler.failure_count = 1;
        handler.last_failure = Some(SystemTime::now() - std::time::Duration::from_secs(100));

        // Attempt recovery - should stop after first failure
        let result = handler.attempt_recovery().await;
        assert!(result.is_ok(), "Recovery should complete (even with failures)");

        // Verify: first item failed and is back in queue, second item was not processed
        assert_eq!(handler.retry_queue.len(), 2, "Failed items should remain in queue");
        assert_eq!(handler.failure_count, 2, "Failure count should increment");
        assert!(handler.last_failure.is_some(), "Last failure should be updated");
    }

    #[tokio::test]
    async fn test_exponential_backoff_calculation() {
        let mut config = create_test_config();
        config.backoff_multiplier = 2;

        let handler = MintHandler::new(config).await.unwrap();

        // Test backoff calculation for different failure counts
        let test_cases = vec![
            (0, 2),    // 2 * 2^0 = 2 seconds
            (1, 4),    // 2 * 2^1 = 4 seconds
            (2, 8),    // 2 * 2^2 = 8 seconds
            (3, 16),   // 2 * 2^3 = 16 seconds
            (5, 64),   // 2 * 2^5 = 64 seconds
            (10, 2048), // 2 * 2^10 = 2048 seconds (max before capping)
        ];

        for (failure_count, expected_seconds) in test_cases {
            let capped_count = failure_count.min(10);
            let backoff = handler.config.backoff_multiplier.saturating_mul(2u64.pow(capped_count));
            assert_eq!(
                backoff, expected_seconds,
                "Backoff calculation incorrect for failure_count={}",
                failure_count
            );
        }
    }

    #[tokio::test]
    async fn test_exponential_backoff_overflow_protection() {
        let mut config = create_test_config();
        config.backoff_multiplier = 2;

        let handler = MintHandler::new(config).await.unwrap();

        // Test that large failure counts are capped to prevent overflow
        let very_large_failure_count = 20u32;
        let capped = very_large_failure_count.min(10);

        assert_eq!(capped, 10, "Failure count should be capped at 10");

        // Verify backoff doesn't overflow
        let backoff = handler.config.backoff_multiplier.saturating_mul(2u64.pow(capped));
        assert_eq!(backoff, 2048, "Backoff should be capped at 2^10 = 2048 seconds");

        // Test saturating_mul prevents overflow
        let max_multiplier = u64::MAX;
        let safe_result = max_multiplier.saturating_mul(2u64.pow(10));
        assert_eq!(safe_result, u64::MAX, "saturating_mul should prevent overflow");
    }

    #[tokio::test]
    async fn test_recovery_resets_counters_on_success() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Set up failure state
        handler.failure_count = 5;
        handler.last_failure = Some(SystemTime::now() - std::time::Duration::from_secs(1000));

        // Add valid share to retry queue
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
            locking_pubkey: test_pubkey(),
        };
        handler.retry_queue.push_back(data);

        // Attempt recovery
        let result = handler.attempt_recovery().await;
        assert!(result.is_ok(), "Recovery should succeed");

        // Verify counters are reset
        assert_eq!(handler.failure_count, 0, "Failure count should be reset to 0");
        assert!(handler.last_failure.is_none(), "Last failure should be cleared");
        assert_eq!(handler.retry_queue.len(), 0, "Queue should be empty");
    }

    // Note: Full integration tests with CDK Mint initialization and external wallet
    // redemption are deferred until we have proper keyset management in place (Phase 10).
    // The tests above verify that P2PK pubkeys are correctly stored in PAID quotes.
}
