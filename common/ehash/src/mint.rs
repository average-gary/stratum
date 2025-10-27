//! Mint handler implementation for eHash token minting
//!
//! This module provides the `MintHandler` type which manages:
//! - CDK Mint instance initialization and lifecycle
//! - Async channel communication for share validation events
//! - eHash token minting based on share difficulty
//! - P2PK token locking for secure wallet redemption (via PAID quotes with locking pubkeys)
//! - Block found event handling and keyset lifecycle
//!
//! ## P2PK Token Locking
//!
//! The MintHandler supports P2PK (Pay-to-Public-Key) token locking by storing
//! locking public keys in the PAID MintQuotes it creates. The actual P2PK-locked
//! tokens are created by external wallets following the standard Cashu protocol:
//!
//! 1. Pool extracts locking_pubkey from TLV during channel setup
//! 2. MintHandler receives locking_pubkey via `register_channel_pubkey()`
//! 3. When minting, MintHandler creates PAID quote with locking_pubkey attached
//! 4. External wallet authenticates with the private key for locking_pubkey
//! 5. External wallet queries for PAID quotes matching their pubkey
//! 6. External wallet creates blinded messages with P2PK SpendingConditions
//! 7. Mint signs the blinded messages and returns blind signatures
//! 8. Wallet unblinds to obtain P2PK-locked token proofs
//!
//! This approach maintains Cashu's privacy guarantees while enabling secure
//! token redemption tied to specific public keys.
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
use bitcoin::secp256k1::PublicKey as Secp256k1PublicKey;
use cdk::amount::Amount;
use cdk::mint::MintBuilder;
use cdk::nuts::{CurrencyUnit, PaymentMethod, Proofs, PublicKey as CdkPublicKey};
use cdk::Mint;
use cdk_common::mint::MintQuote;
use cdk_common::payment::PaymentIdentifier;
use cdk_common::quote_id::QuoteId;
use cdk_sqlite::mint::memory;
use std::collections::HashMap;
use std::sync::Arc;

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
    /// Maps channel_id -> CdkPublicKey (stored in CDK format for efficiency)
    channel_pubkeys: HashMap<u32, CdkPublicKey>,
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

    /// Register locking pubkey for a channel (from TLV during channel setup)
    ///
    /// # Arguments
    /// * `channel_id` - The channel identifier
    /// * `pubkey` - The locking public key for P2PK token creation (secp256k1 format from SV2)
    pub fn register_channel_pubkey(&mut self, channel_id: u32, pubkey: Secp256k1PublicKey) {
        // Convert once at registration time, not on every mint
        let cdk_pubkey = CdkPublicKey::from(pubkey);
        self.channel_pubkeys.insert(channel_id, cdk_pubkey);
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

        // Create quote ID using locking_pubkey (or user_identity as fallback), channel_id, and sequence
        // Using the locking_pubkey ensures cryptographic uniqueness and ties the quote to redemption key
        // CDK requires QuoteId::BASE64 to be valid base64-encoded strings
        use bitcoin::base64::{engine::general_purpose, Engine as _};
        let quote_id_raw = if let Some(pubkey) = locking_pubkey {
            // Use locking pubkey hex for maximum uniqueness
            let pubkey_bytes = pubkey.to_bytes();
            let pubkey_hex = pubkey_bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            format!(
                "ehash_{}_{}_{}", pubkey_hex, data.channel_id, data.sequence_number
            )
        } else {
            // Fallback to user_identity if no locking pubkey
            format!(
                "ehash_{}_{}_{}",
                data.user_identity, data.channel_id, data.sequence_number
            )
        };
        let quote_id_str = general_purpose::URL_SAFE.encode(quote_id_raw.as_bytes());

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

        // Create MintQuote using the constructor
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
            locking_pubkey, // Already in CDK format, no conversion needed
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
            "Created PAID mint quote {} for {} eHash tokens{} (share_hash: {})",
            quote_id_str,
            amount,
            if locking_pubkey.is_some() {
                " (P2PK-locked)"
            } else {
                ""
            },
            share_hash_hex
        );

        // Note: We return empty proofs because Cashu protocol requires wallets
        // to provide blinded messages for privacy. The actual P2PK token creation flow is:
        // 1. Mint creates PAID quote with locking_pubkey (this method)
        // 2. External wallet authenticates with private key for locking_pubkey
        // 3. External wallet queries PAID quotes by pubkey
        // 4. External wallet creates blinded messages with P2PK SpendingConditions
        // 5. External wallet submits MintRequest to mint the tokens
        // 6. Mint signs blinded messages and returns blind signatures
        // 7. External wallet unblinds to get P2PK-locked token proofs
        //
        // This preserves Cashu's privacy guarantees while enabling P2PK locking.

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

    #[test]
    fn test_register_channel_pubkey() {
        use bitcoin::secp256k1::{Secp256k1, SecretKey};

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let pubkey = Secp256k1PublicKey::from_secret_key(&secp, &secret_key);

        // Convert pubkey to CDK format
        let cdk_pubkey = CdkPublicKey::from(pubkey);

        // Verify conversion worked
        assert_eq!(cdk_pubkey.to_bytes(), pubkey.serialize());

        // In actual usage with an async handler, this would be:
        // handler.register_channel_pubkey(channel_id, pubkey);
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
        };

        assert!(sender.try_send(test_data.clone()).is_ok());
        assert!(receiver.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_p2pk_pubkey_registration() {
        use bitcoin::secp256k1::{Secp256k1, SecretKey};

        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Create test pubkeys
        let secp = Secp256k1::new();
        let secret1 = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let pubkey1 = Secp256k1PublicKey::from_secret_key(&secp, &secret1);
        let secret2 = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let pubkey2 = Secp256k1PublicKey::from_secret_key(&secp, &secret2);

        // Register pubkeys for different channels
        handler.register_channel_pubkey(1, pubkey1);
        handler.register_channel_pubkey(2, pubkey2);

        // Verify pubkeys are stored (we can't directly access the HashMap,
        // but we can test that registration doesn't panic)
        assert_eq!(handler.channel_pubkeys.len(), 2);

        // Verify we can retrieve them
        let stored_pubkey1 = handler.channel_pubkeys.get(&1);
        assert!(stored_pubkey1.is_some());
        assert_eq!(stored_pubkey1.unwrap().to_bytes(), pubkey1.serialize());
    }

    #[tokio::test]
    async fn test_process_mint_data_with_p2pk() {
        let config = create_test_config();
        let mut handler = MintHandler::new(config).await.unwrap();

        // Register a locking pubkey for channel 1
        use bitcoin::secp256k1::{Secp256k1, SecretKey};
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[42u8; 32]).unwrap();
        let pubkey = Secp256k1PublicKey::from_secret_key(&secp, &secret);
        handler.register_channel_pubkey(1, pubkey);

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
        };

        // Process the mint data - should create PAID quote with P2PK pubkey
        let result = handler.process_mint_data(data).await;

        // Should succeed
        assert!(result.is_ok(), "Processing mint data with P2PK should succeed");

        // Verify quote was created in database
        let localstore = handler.mint_instance.localstore();
        // Encode the quote ID in base64 URL-safe format as required by CDK
        // Format: ehash_{pubkey_hex}_{channel_id}_{sequence_number}
        use bitcoin::base64::{engine::general_purpose, Engine as _};
        let pubkey_bytes = pubkey.serialize();
        let pubkey_hex = pubkey_bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let quote_id_str = format!("ehash_{}_1_1", pubkey_hex);
        let quote_id_b64 = general_purpose::URL_SAFE.encode(quote_id_str.as_bytes());
        let quote_id = QuoteId::BASE64(quote_id_b64);
        let quote = localstore.get_mint_quote(&quote_id).await.unwrap();

        // Quote should exist and have the locking pubkey
        assert!(quote.is_some(), "Quote should be created in database");
        let quote = quote.unwrap();
        assert!(quote.pubkey.is_some(), "Quote should have P2PK pubkey");
        assert_eq!(quote.pubkey.unwrap().to_bytes(), pubkey.serialize());

        // Verify share hash is in payments field
        assert_eq!(quote.payments.len(), 1, "Quote should have one payment");
        let payment = &quote.payments[0];
        assert_eq!(
            payment.payment_id,
            share_hash.to_string(),
            "Payment ID should be the share hash"
        );
        assert_eq!(payment.amount, Amount::from(256u64), "Payment amount should match eHash amount");
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
        };

        // Process the mint data - should create PAID quote without P2PK pubkey
        let result = handler.process_mint_data(data).await;

        // Should succeed
        assert!(result.is_ok(), "Processing mint data without P2PK should succeed");

        // Verify quote was created without pubkey
        let localstore = handler.mint_instance.localstore();
        // Encode the quote ID in base64 URL-safe format as required by CDK
        // Format: ehash_{user_identity}_{channel_id}_{sequence_number} (fallback when no pubkey)
        use bitcoin::base64::{engine::general_purpose, Engine as _};
        let quote_id_str = format!("ehash_test_user_1_1");
        let quote_id_b64 = general_purpose::URL_SAFE.encode(quote_id_str.as_bytes());
        let quote_id = QuoteId::BASE64(quote_id_b64);
        let quote = localstore.get_mint_quote(&quote_id).await.unwrap();

        assert!(quote.is_some(), "Quote should be created in database");
        let quote = quote.unwrap();
        assert!(quote.pubkey.is_none(), "Quote should not have P2PK pubkey");

        // Verify share hash is still tracked in payments field even without P2PK
        assert_eq!(quote.payments.len(), 1, "Quote should have one payment");
        let payment = &quote.payments[0];
        assert_eq!(
            payment.payment_id,
            share_hash.to_string(),
            "Payment ID should be the share hash"
        );
        assert_eq!(payment.amount, Amount::from(256u64), "Payment amount should match eHash amount");
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
        handler.register_channel_pubkey(1, pubkey1);

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
        };

        // Process first mint with pubkey1
        let result1 = handler.process_mint_data(data1).await;
        assert!(result1.is_ok(), "First mint should succeed");

        // Change the registered pubkey for the same channel
        handler.register_channel_pubkey(1, pubkey2);

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
        };

        // Process second mint with pubkey2
        let result2 = handler.process_mint_data(data2).await;
        assert!(result2.is_ok(), "Second mint should succeed (unique quote ID)");

        // Verify both quotes exist with different IDs
        let localstore = handler.mint_instance.localstore();
        use bitcoin::base64::{engine::general_purpose, Engine as _};

        // Calculate quote IDs based on pubkeys
        let pubkey1_bytes = pubkey1.serialize();
        let pubkey1_hex = pubkey1_bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let quote_id1_str = format!("ehash_{}_1_1", pubkey1_hex);
        let quote_id1_b64 = general_purpose::URL_SAFE.encode(quote_id1_str.as_bytes());
        let quote1 = localstore
            .get_mint_quote(&QuoteId::BASE64(quote_id1_b64))
            .await
            .unwrap();

        let pubkey2_bytes = pubkey2.serialize();
        let pubkey2_hex = pubkey2_bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let quote_id2_str = format!("ehash_{}_1_1", pubkey2_hex);
        let quote_id2_b64 = general_purpose::URL_SAFE.encode(quote_id2_str.as_bytes());
        let quote2 = localstore
            .get_mint_quote(&QuoteId::BASE64(quote_id2_b64))
            .await
            .unwrap();

        assert!(quote1.is_some(), "First quote should exist");
        assert!(quote2.is_some(), "Second quote should exist");
        assert_ne!(
            quote1.as_ref().unwrap().id,
            quote2.as_ref().unwrap().id,
            "Quote IDs should be different due to different pubkeys"
        );
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
        };

        // Handle block found
        handler.handle_block_found(&data).await.unwrap();

        // Verify quote was created (block-finding shares still earn eHash)
        let localstore = handler.mint_instance.localstore();
        use bitcoin::base64::{engine::general_purpose, Engine as _};
        let quote_id_str = format!("ehash_test_user_1_1");
        let quote_id_b64 = general_purpose::URL_SAFE.encode(quote_id_str.as_bytes());
        let quote_id = QuoteId::BASE64(quote_id_b64);
        let quote = localstore.get_mint_quote(&quote_id).await.unwrap();

        assert!(
            quote.is_some(),
            "Block-finding share should still mint eHash tokens"
        );
        let quote = quote.unwrap();
        assert_eq!(
            quote.payments.len(),
            1,
            "Quote should have one payment"
        );
        assert_eq!(
            quote.payments[0].amount,
            Amount::from(256u64),
            "Payment amount should be 256 eHash"
        );
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
        };

        // Should still handle block found (just won't mint eHash tokens)
        let result = handler.handle_block_found(&data).await;
        assert!(
            result.is_ok(),
            "handle_block_found should succeed even if below eHash threshold"
        );

        // Verify no quote was created (below threshold)
        let localstore = handler.mint_instance.localstore();
        use bitcoin::base64::{engine::general_purpose, Engine as _};
        let quote_id_str = format!("ehash_test_user_1_1");
        let quote_id_b64 = general_purpose::URL_SAFE.encode(quote_id_str.as_bytes());
        let quote_id = QuoteId::BASE64(quote_id_b64);
        let quote = localstore.get_mint_quote(&quote_id).await.unwrap();

        assert!(
            quote.is_none(),
            "No quote should be created for shares below eHash threshold"
        );
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

    // Note: Full integration tests with CDK Mint initialization and external wallet
    // redemption are deferred until we have proper keyset management in place (Phase 10).
    // The tests above verify that P2PK pubkeys are correctly stored in PAID quotes.
}
