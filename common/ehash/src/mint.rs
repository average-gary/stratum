//! Mint handler implementation for eHash token minting
//!
//! This module provides the `MintHandler` type which manages:
//! - CDK Mint instance initialization and lifecycle
//! - Async channel communication for share validation events
//! - eHash token minting based on share difficulty
//! - P2PK token locking for secure wallet redemption
//! - Block found event handling and keyset lifecycle

use crate::config::MintConfig;
use crate::error::MintError;
use crate::types::EHashMintData;
use async_channel::{Receiver, Sender};
use bitcoin::secp256k1::PublicKey;
use cdk::amount::Amount;
use cdk::mint::MintBuilder;
use cdk::nuts::{CurrencyUnit, MintQuoteState, Proofs};
use cdk::Mint;
use cdk_common::mint::MintQuote;
use cdk_sqlite::mint::memory;
use std::collections::HashMap;
use std::sync::Arc;

/// Handler for eHash token minting operations
///
/// The MintHandler manages all aspects of eHash token creation:
/// - Receives share validation events via async channels
/// - Calculates eHash amounts based on share difficulty
/// - Creates P2PK-locked tokens using CDK
/// - Handles block found events for keyset lifecycle transitions
///
/// # Thread Safety
/// The MintHandler is designed to run in a dedicated thread spawned via
/// the task manager, completely isolated from mining operations.
pub struct MintHandler {
    /// CDK Mint instance with native database and accounting
    mint_instance: Arc<Mint>,

    /// Receiver for incoming share validation events
    receiver: Receiver<EHashMintData>,

    /// Sender for share validation events (cloneable for distribution)
    sender: Sender<EHashMintData>,

    /// Configuration for mint operations
    config: MintConfig,

    /// Channel locking pubkey mapping for P2PK token creation
    /// Maps channel_id -> PublicKey
    channel_pubkeys: HashMap<u32, PublicKey>,
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
            channel_pubkeys: HashMap::new(),
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

        // Create the DbSignatory with supported units
        use cdk_signatory::db_signatory::DbSignatory;
        use cdk_signatory::embedded::Service;

        let db_signatory = DbSignatory::new(
            keystore.clone(),
            &seed,
            supported_units,
            HashMap::new(), // custom_paths (empty for now)
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

    /// Register locking pubkey for a channel (from TLV during channel setup)
    ///
    /// # Arguments
    /// * `channel_id` - The channel identifier
    /// * `pubkey` - The locking public key for P2PK token creation
    pub fn register_channel_pubkey(&mut self, channel_id: u32, pubkey: PublicKey) {
        self.channel_pubkeys.insert(channel_id, pubkey);
    }

    /// Main processing loop for the mint thread
    ///
    /// Continuously receives and processes share validation events until
    /// the channel is closed.
    ///
    /// # Errors
    /// Returns `MintError` if processing fails unrecoverably
    pub async fn run(&mut self) -> Result<(), MintError> {
        // TODO: Implement main run loop
        // This will be implemented in task 3.6
        Err(MintError::ConfigError(
            "MintHandler run loop not yet implemented".to_string(),
        ))
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

    /// Gracefully shutdown the mint handler, completing pending operations
    ///
    /// This ensures:
    /// - All pending mint operations are completed
    /// - CDK Mint instance is properly closed
    /// - Database connections are cleaned up
    pub async fn shutdown(&mut self) -> Result<(), MintError> {
        tracing::info!("Shutting down MintHandler...");
        // Close the receiver to prevent new events
        self.receiver.close();

        // TODO: Process any remaining events in the queue
        // TODO: Close CDK Mint instance properly
        // TODO: Cleanup database connections

        tracing::info!("MintHandler shutdown complete");
        Ok(())
    }

    /// Create P2PK-locked eHash tokens using CDK's native minting
    ///
    /// Creates MintQuote in PAID state and mints tokens with P2PK spending conditions.
    ///
    /// # Arguments
    /// * `data` - Share validation data for token creation
    ///
    /// # Returns
    /// Vector of minted token proofs
    ///
    /// # Errors
    /// Returns `MintError` if token creation fails
    async fn mint_ehash_tokens(&mut self, data: &EHashMintData) -> Result<Proofs, MintError> {
        // Calculate eHash amount
        let ehash_amount = data.calculate_ehash_amount(self.config.min_leading_zeros);
        let amount = Amount::from(ehash_amount);

        // Get locking pubkey for this channel (if registered)
        let locking_pubkey = self.channel_pubkeys.get(&data.channel_id).copied();

        tracing::debug!(
            "Minting {} eHash for channel {}, P2PK locked: {}",
            amount,
            data.channel_id,
            locking_pubkey.is_some()
        );

        // Create MintQuote in PAID state
        // For eHash, we bypass the normal bolt11 payment flow
        // We directly create PAID quotes since shares are the "payment"
        let quote_id = format!(
            "ehash_{}_{}",
            data.channel_id, data.sequence_number
        );

        // Create quote using CDK's internal API
        // TODO: This is a stub - actual implementation will need to:
        // 1. Create a MintQuote with state=PAID
        // 2. Store quote in database
        // 3. Mint tokens with P2PK conditions if pubkey available
        // 4. Return the minted proofs

        tracing::warn!(
            "P2PK token minting not fully implemented yet - quote {} created",
            quote_id
        );

        // Return empty proofs for now
        Ok(vec![])
    }

    /// Handle block found events and trigger keyset lifecycle
    ///
    /// When a share finds a block:
    /// - Query Template Provider for block reward
    /// - Trigger keyset lifecycle transitions
    /// - Calculate eHash to sats conversion rate
    ///
    /// # Arguments
    /// * `data` - Share validation data with block_found=true
    ///
    /// # Errors
    /// Returns `MintError` if lifecycle transition fails
    async fn handle_block_found(&mut self, data: &EHashMintData) -> Result<(), MintError> {
        tracing::info!(
            "Block found event received for channel {} (template_id: {:?})",
            data.channel_id,
            data.template_id
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

        // TODO: Implement full keyset lifecycle (Phase 10)
        // This includes:
        // 1. Query Template Provider for block reward details
        // 2. Create new ACTIVE keyset (to continue minting)
        // 3. Transition previous keyset ACTIVE → QUANTIFYING → PAYOUT
        // 4. Calculate eHash-to-sats conversion rate
        // 5. Enable eHash to sats swaps for the PAYOUT keyset

        tracing::warn!(
            "Keyset lifecycle management not yet implemented (deferred to Phase 10)"
        );

        Ok(())
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
            supported_units: vec![
                CurrencyUnit::Custom("HASH".to_string()),
                CurrencyUnit::Sat,
            ],
            database_url: None,
            min_leading_zeros: 32,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        }
    }

    #[tokio::test]
    async fn test_mint_config_parsing() {
        let config = create_test_config();
        assert_eq!(config.min_leading_zeros, 32);
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.supported_units.len(), 2);
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
        };

        assert_eq!(data.channel_id, 42);
        assert_eq!(data.sequence_number, 123);
        assert!(data.block_found);
        assert_eq!(data.template_id, Some(456));
    }

    // Note: Full integration tests with CDK Mint initialization are deferred
    // until we have proper keyset management in place (Phase 10).
    // For now, we test the logic that doesn't require full CDK initialization.
}
