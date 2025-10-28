# Design Document

## Overview

This design implements hashpool.dev by integrating Cashu ecash functionality into the Stratum v2 reference implementation. The system adds two new components:

1. **Mint** - A Cashu mint daemon (mool) that replaces the existing file-based share persistence with ecash token minting
2. **Wallet** - A Cashu wallet daemon (walloxy) that automatically redeems ecash tokens based on successful share submissions

The design leverages the existing Stratum v2 architecture patterns and integrates with the CDK (Cashu Development Kit) from the existing git submodule at `deps/cdk`.

## Hashpool Research Findings

Based on analysis of the hashpool submodule at `deps/hashpool/protocols/ehash/`:

### eHash Calculation Algorithm
- **Formula**: `2^(leading_zeros - min_leading_zeros)` where `leading_zeros` is counted from share hash bytes
- **Leading Zero Calculation**: Counts leading zero bits across all 32 bytes, stopping at first non-zero bit
- **Minimum Threshold**: Configurable `min_leading_zeros` parameter (default 32 in tests) - shares below earn 0 eHash
- **Maximum Cap**: Results capped at `2^63` to stay within u64 bounds
- **Implementation**: Available in `deps/hashpool/protocols/ehash/src/work.rs`

### CDK Integration Details
- **Currency Unit**: Uses custom "HASH" unit type in CDK
- **Single-Unit Architecture**: Mint uses HASH unit for eHash token issuance; TProxy tracks accounting only (no wallet needed)
- **Keyset Structure**: 64 signing keys with amounts as powers of 2 (1, 2, 4, 8, ..., 2^63)
- **Share Hash Format**: 32-byte canonical representation with SV2 type conversions
- **Quote Protocol**: Structured requests with share hash, amount, per-share locking pubkey, and "HASH" unit
- **eHash-to-Sats Conversion**: Use eHash tokens to create PAID quotes for sats redemption (custom mint logic)

### Key Functions Available
- `calculate_ehash_amount(hash: [u8; 32], min_leading_zeros: u32) -> u64`
- `calculate_difficulty(hash: [u8; 32]) -> u32` - counts leading zero bits
- `ShareHash` type with conversions to/from SV2 types (PubKey, U256)
- `build_mint_quote_request()` and `parse_mint_quote_request()` for SV2 protocol

### Dependency Considerations
- **Current approach**: Use `ehash = { path = "../../deps/hashpool/protocols/ehash" }`
- **Fallback option**: If API issues arise, rewrite core calculation functions in our own repo
- **Core functions needed**: `calculate_ehash_amount()`, `calculate_difficulty()`, and `ShareHash` type

## Architecture

### High-Level Component Interaction

```mermaid
sequenceDiagram
    participant Miner
    participant TProxy
    participant Pool
    participant Mint
    participant ExternalWallet

    Note over Miner, TProxy: 1. Connection Setup with hpub
    Miner->>TProxy: OpenExtendedMiningChannel (user_identity="hpub1qw508d...")
    TProxy->>TProxy: Parse & validate hpub (disconnect if invalid)
    TProxy->>TProxy: Extract secp256k1 pubkey from hpub
    TProxy->>Miner: OpenMiningChannel.Success

    Note over TProxy, Pool: 2. Mining Loop
    Pool->>TProxy: NewMiningJob
    Miner->>TProxy: SubmitShares
    TProxy->>Pool: SubmitSharesExtended.locking_pubkey = 33-byte pubkey

    Note over Pool, Mint: 3. Share Processing & NUT-20 P2PK Minting
    Pool->>Pool: Extract pubkey from locking_pubkey field
    Pool->>Pool: Validate Share & Calculate eHash Amount
    Pool->>Mint: Create PAID MintQuote (quote_id=UUID_v4, pubkey=per_share_pubkey)
    Note over Mint: NUT-04: Random UUID prevents front-running<br/>NUT-20: P2PK lock enforces authentication
    Mint->>Mint: Store quote with share hash as payment proof
    Pool->>TProxy: SubmitSharesSuccess

    Note over ExternalWallet, Mint: 4. NUT-20 Authenticated Redemption
    ExternalWallet->>Mint: Query quotes by locking pubkey (authenticated)
    Mint->>ExternalWallet: Return secret UUID quote IDs
    ExternalWallet->>ExternalWallet: Create blinded messages (normal secrets)
    ExternalWallet->>ExternalWallet: Sign MintRequest with private key (NUT-20)
    ExternalWallet->>Mint: Submit signed MintRequest
    Mint->>Mint: Verify signature matches quote's locking pubkey
    Mint->>ExternalWallet: Return blind signatures
    ExternalWallet->>ExternalWallet: Unblind to get normal eHash tokens
```

### KeySet Sequence Diagram

```mermaid
sequenceDiagram
    participant Pool
    participant Mint
    participant ExternalWallet
    
    Note over Mint: 5. Block Found Trigger & Keyset Lifecycle
    Mint->>Mint: Receive ShareAccountingEvent with block_found=true
    Mint->>Mint: Create new ACTIVE keyset (ensure continuous minting)
    Mint->>Mint: Transition previous keyset ACTIVE → QUANTIFYING → PAYOUT
    Mint->>Mint: Query Template Provider for block reward details
    Mint->>Mint: Calculate eHash-to-sats conversion rate
    
    Note over ExternalWallet, Mint: 6. eHash to Sats Conversion
    ExternalWallet->>Mint: Submit eHash tokens for sats quote payment
    Mint->>Mint: Validate eHash tokens and mark sats quote as PAID
    ExternalWallet->>Mint: Redeem PAID sats quote
    Mint->>ExternalWallet: Return sats tokens
    
    Mint->>Mint: All eHash redeemed or timeout reached
    Mint->>Mint: Transition keyset PAYOUT → EXPIRED
```

### Thread Architecture

The system uses dedicated threads for eHash operations that run independently of mining operations:

- **Pool/JDC**: Spawn a Mint thread using `task_manager.spawn()` that receives EHashMintData via async channels from share validation results
- **TProxy**: Spawn a Wallet thread using the same task manager pattern that receives WalletCorrelationData via async channels from SubmitSharesSuccess events

The implementation follows this pattern:
1. Handler structs (MintHandler/WalletHandler) manage CDK instances and async channel communication
2. The main role creates handlers and gets sender/receiver channels
3. Background tasks are spawned via `task_manager.spawn()` with graceful shutdown support
4. eHash operations run independently - failures don't affect mining operations
5. Handlers include fault tolerance with retry queues and automatic recovery
6. Integration occurs at share validation points

## Share Validation Integration Strategy

### Refactored Share Validation Architecture

The recent refactoring has significantly simplified the data flow by making share hashes directly available from share validation results:

#### Available from ShareValidationResult:
- `share_hash` - Directly returned from `ShareValidationResult::Valid(share_hash)` and `ShareValidationResult::BlockFound(share_hash, template_id, coinbase)`
- `block_found` - Determined by the variant (`BlockFound` vs `Valid`)
- `template_id` - Available in `BlockFound` variant for solution propagation
- `coinbase` - Available in `BlockFound` variant

#### Available from Channel API:
- `channel_id` - Channel identifier from message context
- `user_identity` - Available via channel.get_user_identity()
- `target` - Available via channel.get_target()
- `sequence_number` - Available from SubmitSharesSuccess.last_sequence_number
- `share_work` - Calculated from target difficulty
- `extranonce_prefix` - Available via channel.get_extranonce_prefix()

#### Available from Message Context:
- `downstream_id` - Client identifier from message handling
- `timestamp` - Current system time when processing
- `share_accounting` - Available via channel.get_share_accounting()

### Integration Strategy: Event-Driven Architecture with Thread Separation

The system integrates into the share validation flow using async channels to communicate with dedicated processing threads:

1. **Share Validation Hook**: Extract data from share validation results in `handle_submit_shares_standard` and `handle_submit_shares_extended`
2. **Event Creation**: Create EHashMintData events containing all required data from the validation context
3. **Async Channel Communication**: Send events to dedicated Mint/Wallet threads via async channels
4. **Thread Separation**: Maintain clean separation between mining operations and eHash processing

### Implementation Approach
```rust
// Integration point in mining message handler
match res {
    Ok(ShareValidationResult::Valid(share_hash)) => {
        // Extract all required data from validation context
        let mint_data = EHashMintData {
            share_hash,
            channel_id,
            user_identity: standard_channel.get_user_identity().clone(),
            target: standard_channel.get_target(),
            sequence_number: msg.sequence_number,
            timestamp: SystemTime::now(),
            block_found: false,
            locking_pubkey: self.extract_pubkey_from_share(&msg)?,  // From locking_pubkey field
        };

        // Send to dedicated Mint thread via async channel
        if let Some(mint_sender) = &self.mint_sender {
            let _ = mint_sender.try_send(mint_data).map_err(|e| {
                error!(target = "mint_integration", "Failed to send mint data: {}", e);
            });
        }
        
        // Continue with existing share accounting logic...
    }
    Ok(ShareValidationResult::BlockFound(share_hash, template_id, coinbase)) => {
        // Create event for block found case
        let mint_data = EHashMintData {
            share_hash,
            channel_id,
            user_identity: standard_channel.get_user_identity().clone(),
            target: standard_channel.get_target(),
            sequence_number: msg.sequence_number,
            timestamp: SystemTime::now(),
            block_found: true,
            template_id: Some(template_id),
            coinbase: Some(coinbase.clone()),
            locking_pubkey: self.extract_pubkey_from_share(&msg)?,  // From locking_pubkey field
        };
        
        // Send to Mint thread for both minting and keyset lifecycle
        if let Some(mint_sender) = &self.mint_sender {
            let _ = mint_sender.try_send(mint_data);
        }
        
        // Continue with existing solution propagation...
    }
}
```

## Components and Interfaces

### 1. Shared eHash Module

A new shared module `common/ehash` that provides common types and functionality:

```rust
// common/ehash/src/lib.rs
#[derive(Debug, Clone)]
pub struct ShareEvent {
    pub channel_id: u32,
    pub user_identity: String,
    pub share_work: u64,
    pub share_sequence_number: u64,
    pub timestamp: SystemTime,
    pub block_found: bool,
    // Additional fields for mint payment evaluation
    pub difficulty: f64,
    pub target: U256,
}

// Shared mint handler that can be used by Pool and JDC
pub mod mint;
// Shared wallet handler that can be used by TProxy  
pub mod wallet;
// Common configuration types
pub mod config;
// Common error types
pub mod error;
```

### 2. Mint Component

The Mint component integrates directly with share validation results and leverages CDK's native database and accounting:

```rust
use cdk::{Mint as CdkMint, Amount, nuts::CurrencyUnit};
use cdk_common::database::DynMintDatabase;

pub struct MintHandler {
    /// CDK Mint instance with native database and accounting
    mint_instance: CdkMint,
    receiver: async_channel::Receiver<EHashMintData>,
    sender: async_channel::Sender<EHashMintData>,
    config: MintConfig,
}

impl MintHandler {
    /// Create new MintHandler with CDK's native database backend
    pub async fn new(config: MintConfig, status_tx: Sender<Status>) -> Result<Self, MintError>;
    
    pub fn get_receiver(&self) -> async_channel::Receiver<EHashMintData>;
    pub fn get_sender(&self) -> async_channel::Sender<EHashMintData>;
    
    /// Main processing loop for the mint thread
    pub async fn run(&mut self) -> Result<(), MintError>;
    
    /// Main processing loop with graceful shutdown handling
    /// Completes pending mint operations before terminating
    pub async fn run_with_shutdown(&mut self, shutdown_rx: async_channel::Receiver<()>) -> Result<(), MintError>;
    
    /// Process share validation data and mint eHash tokens
    /// Uses CDK's native MintQuote and database for accounting
    pub async fn process_mint_data(&mut self, data: EHashMintData) -> Result<(), MintError>;
    
    /// Gracefully shutdown the mint handler, completing pending operations
    pub async fn shutdown(&mut self) -> Result<(), MintError>;

    /// Create NUT-04/NUT-20 compliant PAID quotes with per-share P2PK locking
    /// - Generates random UUID v4 quote ID (NUT-04: prevents front-running)
    /// - Attaches locking_pubkey from EHashMintData (NUT-20: enforces authentication)
    /// - Returns empty proofs (wallet mints via NUT-20 authenticated flow)
    async fn mint_ehash_tokens(&mut self, data: &EHashMintData) -> Result<Vec<Proof>, MintError>;

    /// Handle block found events and trigger keyset lifecycle
    async fn handle_block_found(&mut self, data: &EHashMintData) -> Result<(), MintError>;

    /// Store pubkey-to-quote mapping in CDK KV store for authenticated queries
    async fn store_pubkey_quote_mapping(&mut self, pubkey: &PublicKey, quote_id: &QuoteId) -> Result<(), MintError>;

    /// Query quotes by pubkey with NUT-20 signature authentication
    async fn get_quotes_by_pubkey_authenticated(
        &self,
        pubkey: &PublicKey,
        signature: &Signature,
        message: &str,
    ) -> Result<Vec<QuoteSummary>, MintError>;
}

#[derive(Clone, Debug)]
pub struct MintSender {
    sender: async_channel::Sender<EHashMintData>,
}
```

### 3. EHash Handler Component

The EhashHandler tracks eHash accounting statistics for downstream miners in TProxy. It does not redeem tokens - external wallets with private keys handle redemption via the authenticated quote discovery API:

```rust
use bitcoin::secp256k1::PublicKey;
use std::collections::HashMap;

pub struct EhashHandler {
    /// Accounting data for tracking eHash issued per pubkey
    ehash_balances: HashMap<PublicKey, u64>,  // pubkey -> total eHash earned
    channel_stats: HashMap<u32, ChannelStats>,  // channel_id -> statistics
    receiver: async_channel::Receiver<EhashCorrelationData>,
    sender: async_channel::Sender<EhashCorrelationData>,
}

#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel_id: u32,
    pub locking_pubkey: PublicKey,
    pub user_identity: String,
    pub total_ehash: u64,
    pub share_count: u64,
    pub last_share_time: SystemTime,
}

#[derive(Debug, Clone)]
pub struct EhashCorrelationData {
    pub channel_id: u32,
    pub sequence_number: u32,
    pub user_identity: String,
    pub timestamp: SystemTime,
    pub ehash_amount: u64,           // Amount of eHash issued for this share (for accounting/display)
    pub locking_pubkey: PublicKey,   // Downstream miner's pubkey (for tracking by user)
}

impl EhashHandler {
    /// Create new EhashHandler (used by TProxy and JDC-ehash mode)
    /// Tracks eHash accounting for downstream miners but does not redeem tokens
    pub fn new() -> Self;

    pub fn get_receiver(&self) -> async_channel::Receiver<EhashCorrelationData>;
    pub fn get_sender(&self) -> async_channel::Sender<EhashCorrelationData>;

    /// Main processing loop for the ehash handler thread
    pub async fn run(&mut self) -> Result<(), EhashError>;

    /// Main processing loop with graceful shutdown handling
    pub async fn run_with_shutdown(&mut self, shutdown_rx: async_channel::Receiver<()>) -> Result<(), EhashError>;

    /// Process SubmitSharesSuccess correlation data
    /// Updates accounting statistics for display purposes
    pub async fn process_correlation_data(&mut self, data: EhashCorrelationData) -> Result<(), EhashError>;

    /// Gracefully shutdown the ehash handler
    pub async fn shutdown(&mut self) -> Result<(), EhashError>;

    /// Get total eHash earned by a specific pubkey
    pub fn get_ehash_balance(&self, pubkey: &PublicKey) -> u64;

    /// Get accounting statistics for a channel
    pub fn get_channel_stats(&self, channel_id: u32) -> Option<ChannelStats>;
}
```

### 4. Thread Architecture and Integration

The system follows the existing Stratum v2 pattern of spawning dedicated threads for background processing:

```rust
// Integration in ChannelManager
pub struct ChannelManager {
    // Existing fields...
    mint_sender: Option<async_channel::Sender<EHashMintData>>,
}

impl ChannelManager {
    /// Initialize with mint thread communication
    pub fn with_mint_sender(mut self, mint_sender: async_channel::Sender<EHashMintData>) -> Self {
        self.mint_sender = Some(mint_sender);
        self
    }

    /// Extract per-share locking pubkey from SubmitSharesExtended locking_pubkey field
    pub fn extract_pubkey_from_share(
        msg: &stratum_apps::stratum_core::mining_sv2::SubmitSharesExtended,
    ) -> Result<bitcoin::secp256k1::PublicKey, PoolError> {
        use bitcoin::secp256k1::PublicKey;

        // Get locking pubkey bytes from PubKey33 (always 33 bytes fixed)
        let pubkey_bytes: &[u8] = msg.locking_pubkey.inner_as_ref();

        // Check if all zeros (indicates locking_pubkey not set for eHash)
        if pubkey_bytes.iter().all(|&b| b == 0) {
            return Err(PoolError::Custom(
                "Locking pubkey not set (all zeros) in SubmitSharesExtended".to_string(),
            ));
        }

        // PubKey33 is always 33 bytes, no need to validate length
        // Parse secp256k1 public key from bytes (compressed SEC1 format)
        PublicKey::from_slice(pubkey_bytes).map_err(|e| {
            PoolError::Custom(format!("Invalid secp256k1 pubkey in locking_pubkey field: {}", e))
        })
    }
}

// Thread spawning pattern with graceful shutdown:
// 1. Create MintHandler with config and status_tx
// 2. Get sender/receiver channels from handler
// 3. Pass sender to ChannelManager initialization
// 4. Spawn MintHandler::run() task with shutdown signal handling
// 5. Share validation events automatically flow to mint thread
// 6. Mint thread processes events independently of mining operations
// 7. On shutdown, mint thread completes pending operations before terminating

pub async fn spawn_mint_thread(
    task_manager: &mut TaskManager,
    config: MintConfig,
    status_tx: &Sender<Status>,
    shutdown_rx: async_channel::Receiver<()>,
) -> Result<async_channel::Sender<EHashMintData>, MintError> {
    let mut mint_handler = MintHandler::new(config, status_tx.clone()).await?;
    let sender = mint_handler.get_sender();
    
    task_manager.spawn("mint_handler", async move {
        mint_handler.run_with_shutdown(shutdown_rx).await
    });
    
    Ok(sender)
}

pub fn spawn_ehash_handler_thread(
    task_manager: &mut TaskManager,
    shutdown_rx: async_channel::Receiver<()>,
) -> async_channel::Sender<EhashCorrelationData> {
    let mut ehash_handler = EhashHandler::new();
    let sender = ehash_handler.get_sender();

    task_manager.spawn("ehash_handler", async move {
        ehash_handler.run_with_shutdown(shutdown_rx).await
    });

    sender
}
```

## Data Models

### JDC Configuration Modes

The JDC role supports flexible configuration as either a Mint or Wallet:

```rust
#[derive(Debug, Deserialize)]
pub struct JdcEHashConfig {
    /// JDC eHash mode: "mint" or "wallet"
    pub mode: JdcEHashMode,

    /// Mint configuration (used when mode = "mint")
    pub mint: Option<MintConfig>,

    /// Wallet configuration (used when mode = "wallet")
    pub wallet: Option<WalletConfig>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JdcEHashMode {
    /// JDC acts as a mint, processing share validation results
    Mint,
    /// JDC tracks eHash accounting, processing SubmitSharesSuccess correlation
    Wallet,
}

// JDC initialization based on configuration
impl JdcChannelManager {
    pub async fn with_ehash_config(
        mut self,
        config: JdcEHashConfig,
        task_manager: &mut TaskManager,
        status_tx: &Sender<Status>,
    ) -> Result<Self, JdcError> {
        match config.mode {
            JdcEHashMode::Mint => {
                if let Some(mint_config) = config.mint {
                    let mint_sender = spawn_mint_thread(task_manager, mint_config, status_tx).await?;
                    self.mint_sender = Some(mint_sender);
                }
            }
            JdcEHashMode::Wallet => {
                if let Some(wallet_config) = config.wallet {
                    let wallet_sender = spawn_wallet_thread(task_manager, wallet_config, shutdown_rx).await?;
                    self.wallet_sender = Some(wallet_sender);
                }
            }
        }
        Ok(self)
    }
}
```

### Configuration Models

Integration with existing Stratum v2 TOML configuration:

```rust
use cdk::{mint_url::MintUrl, nuts::CurrencyUnit, Amount};
use cdk_common::database::MintDatabase;

#[derive(Debug, Deserialize)]
pub struct MintConfig {
    // CDK mint configuration - maps to cdk::Mint::new() parameters
    pub mint_url: MintUrl,  // cdk::mint_url::MintUrl
    pub mint_private_key: Option<String>,  // For cdk::Mint initialization
    pub supported_units: Vec<CurrencyUnit>,  // cdk::nuts::CurrencyUnit (Sat, Msat, custom units)

    // CDK database configuration - for cdk::cdk_database::MintDatabase
    pub database_url: Option<String>,  // For CDK database backends (sqlite, postgres, redb)

    // Payment logic (hashpool-specific)
    pub min_leading_zeros: u32,  // Minimum leading zero bits required to earn 1 unit of ehash (hashpool default: 32)

    // Bitcoin RPC configuration (Phase 10) - for querying block reward data
    pub bitcoin_rpc_url: Option<String>,  // Bitcoin Core RPC endpoint (e.g., "http://127.0.0.1:8332")
    pub bitcoin_rpc_user: Option<String>,  // RPC authentication username
    pub bitcoin_rpc_password: Option<String>,  // RPC authentication password
    pub bitcoin_rpc_timeout_secs: Option<u64>,  // RPC call timeout in seconds (default: 30)

    // Fault tolerance configuration
    pub max_retries: Option<u32>,  // Maximum retry attempts before disabling (default: 10)
    pub backoff_multiplier: Option<u64>,  // Backoff multiplier in seconds (default: 2)
    pub recovery_enabled: Option<bool>,  // Enable automatic recovery (default: true)

    // Integration with existing Stratum v2 config
    pub log_level: Option<String>,
}

// Re-export CDK types for convenience
pub use cdk::nuts::CurrencyUnit as UnitType;
pub use cdk::Amount;
pub use cdk::mint_url::MintUrl;
```

### Data Models and Sources

The refactored architecture provides direct access to all required data at share validation time:

#### EHashMintData Structure
```rust
use bitcoin::hashes::sha256d::Hash;
use bitcoin::Target;

#[derive(Debug, Clone)]
pub struct EHashMintData {
    // Core share data (available from ShareValidationResult)
    pub share_hash: Hash,
    pub block_found: bool,

    // Channel context (available from message and channel API)
    pub channel_id: u32,
    pub user_identity: String,  // hpub format from downstream miner
    pub target: Target,
    pub sequence_number: u32,
    pub timestamp: SystemTime,

    // Optional template data (available in BlockFound case)
    pub template_id: Option<u64>,
    pub coinbase: Option<Vec<u8>>,

    // Required per-share locking pubkey for NUT-20 P2PK authentication
    // Extracted from SubmitSharesExtended.locking_pubkey field (PubKey33)
    pub locking_pubkey: bitcoin::secp256k1::PublicKey,
}

impl EHashMintData {
    /// Create from share validation context in Pool/JDC
    pub fn from_validation_result(
        validation_result: &ShareValidationResult,
        channel_id: u32,
        sequence_number: u32,
        channel: &dyn ChannelApi,  // Generic channel interface
    ) -> Self {
        match validation_result {
            ShareValidationResult::Valid(share_hash) => {
                Self {
                    share_hash: *share_hash,
                    block_found: false,
                    channel_id,
                    user_identity: channel.get_user_identity().clone(),
                    target: channel.get_target(),
                    sequence_number,
                    timestamp: SystemTime::now(),
                    template_id: None,
                    coinbase: None,
                }
            }
            ShareValidationResult::BlockFound(share_hash, template_id, coinbase) => {
                Self {
                    share_hash: *share_hash,
                    block_found: true,
                    channel_id,
                    user_identity: channel.get_user_identity().clone(),
                    target: channel.get_target(),
                    sequence_number,
                    timestamp: SystemTime::now(),
                    template_id: Some(*template_id),
                    coinbase: Some(coinbase.clone()),
                }
            }
        }
    }
    
    /// Calculate eHash amount using hashpool's exponential valuation method
    /// Formula: 2^(leading_zero_bits - minimum_difficulty)
    /// Returns 0 if share doesn't meet minimum difficulty threshold
    pub fn calculate_ehash_amount(&self, minimum_difficulty: u32) -> Amount {
        let hash_bytes: [u8; 32] = self.share_hash.as_byte_array().try_into().unwrap_or([0; 32]);
        let ehash_amount = ehash::calculate_ehash_amount(hash_bytes, minimum_difficulty);
        Amount::from(ehash_amount)
    }
}
```

#### JDC Role - Share Accounting Data
```rust
// JDC has access to share accounting but uses NoPersistence
// Generate ShareEvent from SubmitSharesSuccess context in JDC
impl ShareEvent {
    pub fn from_jdc_context(
        channel_id: u32,
        last_sequence_number: u32,
        new_submits_accepted_count: u32,
        new_shares_sum: u64,
        user_identity: String,
        block_found: bool,
    ) -> Self {
        ShareEvent {
            channel_id,
            user_identity,
            share_work: new_shares_sum,
            share_sequence_number: last_sequence_number as u64,
            timestamp: SystemTime::now(),
            block_found,
            share_hash: None, // Not available in JDC context
            total_shares_accepted: Some(new_submits_accepted_count),
            total_share_work_sum: Some(new_shares_sum),
            difficulty: 0.0,
            target: U256::default(),
        }
    }
}
```

#### TProxy Role - SubmitSharesSuccess
```rust
// TProxy creates correlation data for eHash accounting (external wallets handle redemption)
impl ShareEvent {
    pub fn from_submit_shares_success(
        msg: SubmitSharesSuccess, 
        user_identity: String
    ) -> Self {
        let correlation_key = ShareCorrelationKey {
            channel_id: msg.channel_id,
            sequence_number: msg.last_sequence_number as u64,
            user_identity: user_identity.clone(),
        };
        
        ShareEvent {
            channel_id: msg.channel_id,
            user_identity,
            share_work: msg.new_shares_sum,
            share_sequence_number: msg.last_sequence_number as u64,
            timestamp: SystemTime::now(),
            block_found: false, // TProxy doesn't know about block finds
            share_hash: None, // Not available in TProxy
            total_shares_accepted: Some(msg.new_submits_accepted_count),
            total_share_work_sum: Some(msg.new_shares_sum),
            difficulty: 0.0,
            target: U256::default(),
            correlation_key,
        }
    }
}

// External wallets can query mint for tokens using correlation data
pub struct WalletRedemptionQuery {
    pub mint_url: MintUrl,
    pub correlation_key: ShareCorrelationKey,
    pub sequence_range: Option<(u64, u64)>,  // For batch redemptions
}
```

#### Updated ShareEvent Structure
```rust
use bitcoin::hashes::sha256d::Hash;

#[derive(Debug, Clone)]
pub struct ShareEvent {
    // Core correlation fields (available in both Pool/JDC and TProxy)
    pub channel_id: u32,
    pub user_identity: String,
    pub share_sequence_number: u64,
    pub timestamp: SystemTime,
    pub share_work: u64,
    
    // Pool/JDC specific fields (for eHash calculation)
    pub share_hash: Option<Hash>,  // Used to calculate leading zeros for eHash amount
    pub block_found: bool,
    pub total_shares_accepted: Option<u32>,
    pub total_share_work_sum: Option<u64>,
    pub difficulty: f64,
    pub target: U256,
    
    // Correlation metadata
    pub correlation_key: ShareCorrelationKey,
}

impl ShareEvent {
    /// Calculate eHash amount using hashpool's exponential valuation method
    /// Formula: 2^(leading_zero_bits - minimum_difficulty)
    /// Returns 0 if share doesn't meet minimum difficulty threshold
    pub fn calculate_ehash_amount(&self, minimum_difficulty: u32) -> Amount {
        if let Some(hash) = &self.share_hash {
            let hash_bytes: [u8; 32] = hash.as_byte_array().try_into().unwrap_or([0; 32]);
            let ehash_amount = ehash::calculate_ehash_amount(hash_bytes, minimum_difficulty);
            Amount::from(ehash_amount)
        } else {
            Amount::from(0)
        }
    }
}

// Re-export hashpool's calculation functions from deps/hashpool/protocols/ehash
pub use ehash::{calculate_ehash_amount, calculate_difficulty};
```

## Error Handling

### Error Types

Integration with existing Stratum v2 Status system:

```rust
#[derive(Debug)]
pub enum MintError {
    CdkError(cdk::Error),
    ConfigError(String),
    ChannelError(String),
    PaymentEvaluationError(String),
}

#[derive(Debug)]
pub enum WalletError {
    CdkError(cdk::Error),
    ConfigError(String),
    ChannelError(String),
    RedemptionError(String),
    NetworkError(String),
}

// Integration with existing Status system
impl From<MintError> for Status {
    fn from(error: MintError) -> Self {
        Status {
            state: State::MintError(error.to_string()),
        }
    }
}
```

### Error Recovery and Fault Tolerance

The system is designed with fault tolerance to ensure mining operations continue even when eHash operations fail:

#### Thread Independence
- **Mining Thread Isolation**: Mining operations (share validation, solution propagation) continue normally even if Mint/Wallet threads fail
- **eHash Thread Isolation**: Mint/Wallet thread failures don't affect core mining functionality
- **Optional eHash Operations**: All eHash functionality is optional - mining works without it

#### Failure Scenarios and Recovery

##### Mint Thread Failures
- **Pool/JDC Behavior**: Continue normal mining operations, log mint failures
- **Event Queuing**: Failed mint events are queued for retry with exponential backoff
- **Automatic Recovery**: Attempt to restart mint thread after configurable delay
- **Fallback Mode**: If mint consistently fails, disable eHash minting but continue mining
- **Status Reporting**: Report mint status via existing Status system without affecting mining

##### Wallet Thread Failures  
- **TProxy Behavior**: Continue normal translation operations, log wallet failures
- **Correlation Queuing**: Failed correlation events are queued for retry
- **Automatic Recovery**: Attempt to restart wallet thread after configurable delay
- **Fallback Mode**: If wallet consistently fails, disable correlation tracking but continue translation
- **Status Reporting**: Report wallet status independently of translation status

##### CDK/Network Failures
- **Transient Failures**: Implement exponential backoff for CDK operations (mint, wallet, network)
- **Persistent Failures**: After max retries, disable eHash operations but continue mining
- **Network Partitions**: Queue operations during network issues, replay when connectivity restored
- **Database Failures**: Use CDK's built-in database recovery mechanisms

#### Recovery Implementation Strategy

```rust
pub struct MintHandler {
    // ... existing fields ...
    retry_queue: VecDeque<EHashMintData>,
    failure_count: u32,
    last_failure: Option<SystemTime>,
    max_retries: u32,
    backoff_multiplier: u64,
    recovery_enabled: bool,
}

impl MintHandler {
    /// Process mint data with automatic retry on failure
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
                
                // Disable if too many failures
                if self.failure_count > self.max_retries {
                    warn!("Mint thread disabled after {} failures", self.max_retries);
                    self.recovery_enabled = false;
                }
                
                Err(e)
            }
        }
    }
    
    /// Attempt to recover from failures
    pub async fn attempt_recovery(&mut self) -> Result<(), MintError> {
        if !self.recovery_enabled {
            return Ok(());
        }
        
        // Exponential backoff
        if let Some(last_failure) = self.last_failure {
            let backoff_duration = Duration::from_secs(
                self.backoff_multiplier * (2_u64.pow(self.failure_count.min(10)))
            );
            
            if last_failure.elapsed().unwrap_or(Duration::ZERO) < backoff_duration {
                return Ok(());
            }
        }
        
        // Process retry queue
        while let Some(data) = self.retry_queue.pop_front() {
            match self.process_mint_data(data.clone()).await {
                Ok(()) => {
                    info!("Mint recovery successful");
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
}
```

#### Integration with Mining Operations

```rust
// In ChannelManager share validation
match res {
    Ok(ShareValidationResult::Valid(share_hash)) => {
        // ALWAYS continue with mining operations regardless of mint status
        let response = SubmitSharesSuccess { /* ... */ };
        
        // TRY to send to mint thread (non-blocking)
        if let Some(mint_sender) = &self.mint_sender {
            let mint_data = EHashMintData { /* ... */ };
            
            // Use try_send to avoid blocking mining operations
            if let Err(e) = mint_sender.try_send(mint_data) {
                // Log error but don't fail the mining operation
                warn!("Failed to send mint data: {}, mining continues", e);
            }
        }
        
        // Continue with normal mining flow
        Ok(response)
    }
}
```

### Graceful Shutdown Strategy

The Mint and Wallet threads require special shutdown handling to ensure data integrity:

#### Mint Thread Shutdown
1. **Signal Reception**: Mint thread receives shutdown signal via dedicated channel
2. **Channel Closure**: Stop accepting new EHashMintData events by closing receiver
3. **Pending Operations**: Complete all pending mint operations in the queue
4. **CDK Cleanup**: Properly close CDK Mint instance and database connections
5. **State Persistence**: Ensure all minted tokens are properly recorded
6. **Thread Termination**: Exit gracefully after cleanup completion

#### Wallet Thread Shutdown
1. **Signal Reception**: Wallet thread receives shutdown signal via dedicated channel
2. **Channel Closure**: Stop accepting new WalletCorrelationData events
3. **Pending Redemptions**: Complete any in-progress token redemption operations
4. **CDK Cleanup**: Properly close CDK Wallet instance and network connections
5. **State Persistence**: Ensure wallet state is properly saved
6. **Thread Termination**: Exit gracefully after cleanup completion

#### Implementation Pattern
```rust
pub async fn run_with_shutdown(&mut self, shutdown_rx: async_channel::Receiver<()>) -> Result<(), MintError> {
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
                info!("Mint thread received shutdown signal, completing pending operations...");
                self.shutdown().await?;
                break;
            }
        }
    }
    Ok(())
}
```

## Testing Strategy

### Unit Testing

Focus on eHash-specific functionality (Stratum v2 and CDK unit tests are handled by upstream crates):

1. **eHash-Specific Tests**
   - ShareEvent processing and conversion from share validation results
   - EHashMintData creation from validation context
   - WalletCorrelationData processing from SubmitSharesSuccess
   - eHash amount calculation using hashpool functions
   - Share hash extraction and conversion to CDK types

### Integration Testing

1. **End-to-End Flow Tests**
   - Pool → Mint → Wallet redemption flow
   - JDC → Mint → Wallet redemption flow
   - Multi-unit type support (sats and hash)

2. **Thread Communication Tests**
   - Share validation to EHashMintData flow
   - SubmitSharesSuccess to WalletCorrelationData flow
   - Async channel reliability under load

## Module Structure and Dependencies

### Shared eHash Module
```toml
# common/ehash/Cargo.toml
[dependencies]
cdk = { path = "../../deps/cdk/crates/cdk", features = ["mint", "wallet"] }
cdk-common = { path = "../../deps/cdk/crates/cdk-common" }
ehash = { path = "../../deps/hashpool/protocols/ehash" }
stratum-apps = { path = "../../roles/stratum-apps" }  # For share validation integration
bitcoin = { version = "0.32.2", features = ["secp256k1"] }
# Other common dependencies...
```

### Role Dependencies

```toml
# Pool and JDC Cargo.toml files (need full CDK mint functionality)
[dependencies]
ehash = { path = "../../common/ehash" }
stratum-apps = { path = "../stratum-apps" }  # For updated persistence traits
# Existing dependencies...

# TProxy Cargo.toml file (only needs share correlation functionality)
[dependencies]
ehash = { path = "../../common/ehash", default-features = false, features = ["share-correlation"] }
# Existing dependencies...
```

## Implementation Notes

### Hashpool Research Results

Based on analysis of the hashpool submodule at `deps/hashpool`, the following has been determined:

1. **eHash Calculation Algorithm**: Uses exponential valuation `2^(leading_zero_bits - minimum_difficulty)` from `deps/hashpool/protocols/ehash/src/work.rs`
2. **Minimum Threshold Logic**: Configurable `minimum_difficulty` parameter (default: 32 leading zero bits) from shared config
3. **Calculation Implementation**: Available functions `calculate_ehash_amount()` and `calculate_difficulty()` in ehash crate
4. **Configuration**: Uses `EhashConfig { minimum_difficulty: u32 }` structure in shared configuration
5. **Integration Pattern**: Pool processes shares and sends mint quote requests to separate mint daemon via SV2 messaging

### Remaining Research Items

1. **Keyset Lifecycle Timing**: Understand the exact triggers and timing for keyset state transitions
2. **External Wallet Integration**: Research the specific protocols hashpool uses for external wallet redemption
3. **SV2 Extension Protocol**: Analyze hashpool's SV2 extension implementation for eHash negotiation

### CDK Integration Points

1. **Submodule Integration**: Use the existing CDK submodule at `deps/cdk` (tagged v0.13.x version)
2. **Mint Initialization**: Use CDK's `cdk::Mint` with custom unit support via the `mint` feature
3. **Wallet Initialization**: Use CDK's `cdk::Wallet` with multi-mint support via the `wallet` feature  
4. **Token Operations**: Leverage CDK's minting and redemption protocols from `cdk::nuts`
5. **Unit Support**: Configure CDK for both Bitcoin (sats) and custom hash units using `cdk::Amount`
6. **Database Integration**: Use CDK's database abstractions (`cdk::cdk_database`) for persistence

### Compatibility Considerations

1. **CDK Submodule**: Leverage the existing CDK submodule at `deps/cdk` (tagged v0.13.x version)
2. **Hashpool Submodule**: Use the existing hashpool submodule at `deps/hashpool` for ehash calculations
3. **Stratum-Apps Integration**: Use the current share validation architecture from `stratum-apps` crate
6. **Network Difficulty**: Access current network difficulty from existing Stratum v2 template/job context
7. **Configuration**: Extend existing TOML structures without breaking changes
8. **Status System**: Integrate new error types with existing Status enum
9. **Thread Management**: Follow existing patterns for thread spawning and management

### Deployment Flexibility

1. **Optional Components**: Allow disabling ecash functionality via configuration
2. **Independent Operation**: eHash operations run independently of core mining functionality
3. **Multi-Instance**: Support multiple mint/wallet instances for different unit types
4. **Network Configuration**: Flexible mint URL configuration for different deployment scenarios

### Key Protocol Elements

- **Locking Pubkeys**: Each downstream miner specifies their pubkey via user_identity (hpub format) at connection setup; enables NUT-20 authentication
- **PAID Quotes**: Pool creates quotes in PAID status when shares are validated, with per-share locking pubkey from direct PubKey33 field
- **Channel/Sequence Correlation**: Unique identifiers (channel_id, sequence_number) for tracking shares
- **Blinded Secrets**: Standard Cashu protocol for privacy-preserving token minting

### Implementation Impact

This uses per-miner locking pubkeys for authentication:

1. **Downstream Miners**: Each miner specifies their own locking pubkey in `user_identity` field (hpub format) when connecting to TProxy
2. **TProxy**: Receives hpub from each downstream miner, validates format (disconnect if invalid), extracts secp256k1 pubkey, stores per-channel
3. **Per-Share Submission**: TProxy sets `SubmitSharesExtended.locking_pubkey` (PubKey33 field) to the downstream miner's pubkey when submitting shares upstream
4. **Pool (Mool)**: Extracts pubkey from `locking_pubkey` field, includes in `EHashMintData.locking_pubkey`, creates NUT-20 P2PK-locked PAID quotes
5. **External Wallet**: Authenticates with their private key (NUT-20 signature), queries mint for quotes by their locking pubkey
6. **Mint**: Verifies NUT-20 signature, returns quote IDs associated with authenticated pubkey
7. **Multi-Miner Support**: TProxy can handle multiple downstream miners simultaneously, each with their own unique locking pubkey

## eHash as Stratum v2 Extension

Following the SV2 extension specification, eHash functionality will be implemented as Extension Type `0x0003`:

### Extension Negotiation

1. **Connection Setup**:
   ```
   Client --- SetupConnection ---> Server
   Client <--- SetupConnection.Success ---- Server
   ```

2. **Extension Request**:
   ```
   Client --- RequestExtensions [0x0003] ---> Server
   Client <--- RequestExtensions.Success [0x0003] ---- Server
   ```

### Protocol Extension for eHash

eHash extends `SubmitSharesExtended` with a direct `locking_pubkey` field for per-share pubkey locking:

| Field Name | Type | Message | Description |
|------------|------|---------|-------------|
| `locking_pubkey` | `PubKey33` | `SubmitSharesExtended` | 33-byte compressed secp256k1 public key (per-share, TProxy→Pool) |

#### PubKey33 Field Format

**Type:** `PubKey33` - Fixed 33-byte public key field

**Structure:**

| Field | Type | Size | Description |
|-------|------|------|-------------|
| locking_pubkey | PubKey33 | 33 bytes | Compressed secp256k1 public key (SEC1 format) |

**Value Format (SEC1 Compressed):**

The 33-byte value is a compressed secp256k1 public key in SEC1 format:
- First byte: `0x02` or `0x03` (compression prefix indicating y-coordinate parity)
- Next 32 bytes: x-coordinate of the public key point

Example:
```
02 1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
^  ^                                                              ^
|  |                                                              |
|  +------ 32-byte x-coordinate --------------------------------+
|
+-- Compression prefix (0x02 = even y, 0x03 = odd y)
```

### Modified Message Flow

#### 1. Downstream Connection Setup
- **Miner → TProxy**: `OpenExtendedMiningChannel` with:
  ```
  user_identity: "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k..."
  ```
- **TProxy**: Parses hpub, validates bech32 format, extracts secp256k1 pubkey
- **TProxy**: If invalid → disconnect + no jobs
- **TProxy**: If valid → store pubkey for the channel

#### 2. Share Submission with Per-Share Pubkey
- **TProxy → Pool**: `SubmitSharesExtended` with `locking_pubkey` field set to 33-byte compressed secp256k1 pubkey
- Each share includes the locking pubkey from the downstream miner's hpub
- Pool extracts pubkey from `locking_pubkey` field using `extract_pubkey_from_share()`

#### 3. Share Response
- **Pool → TProxy**: `SubmitSharesSuccess` (standard response)
- No quote_id communicated in response
- TProxy will query mint later using authenticated API to discover quote IDs

#### Implementation Examples

**TProxy - Extract and Include Pubkey in locking_pubkey Field:**
```rust
// On downstream connection
fn handle_open_channel(user_identity: String, channel_id: u32) -> Result<(), Error> {
    // Parse hpub from user_identity
    let pubkey = parse_hpub(&user_identity)?;

    // If invalid, disconnect
    if pubkey.is_none() {
        return Err(Error::InvalidHpub);
    }

    // Store pubkey for this channel
    self.channel_pubkeys.insert(channel_id, pubkey.unwrap());
    Ok(())
}

// On share submission upstream
fn submit_share_upstream(&self, channel_id: u32, share: Share) -> Result<(), Error> {
    // Get pubkey for this channel
    let pubkey = self.channel_pubkeys.get(&channel_id)?;

    // Set locking_pubkey field (PubKey33) in SubmitSharesExtended
    let mut extended_share = SubmitSharesExtended::from(share);
    extended_share.locking_pubkey = PubKey33::from_slice(&pubkey.serialize())?;

    // Submit with locking_pubkey set
    submit_shares_extended(extended_share)?;
    Ok(())
}
```

**Pool - Extract Pubkey from locking_pubkey Field:**
```rust
pub fn extract_pubkey_from_share(
    msg: &SubmitSharesExtended,
) -> Result<bitcoin::secp256k1::PublicKey, PoolError> {
    use bitcoin::secp256k1::PublicKey;

    // Get locking pubkey bytes from PubKey33 (always 33 bytes fixed)
    let pubkey_bytes: &[u8] = msg.locking_pubkey.inner_as_ref();

    // Check if all zeros (indicates locking_pubkey not set for eHash)
    if pubkey_bytes.iter().all(|&b| b == 0) {
        return Err(PoolError::Custom(
            "Locking pubkey not set (all zeros)".to_string(),
        ));
    }

    // Parse secp256k1 public key from bytes (compressed SEC1 format)
    PublicKey::from_slice(pubkey_bytes).map_err(|e| {
        PoolError::Custom(format!("Invalid secp256k1 pubkey: {}", e))
    })
}

fn process_share(&self, share: SubmitSharesExtended) -> Result<(), Error> {
    // Extract locking pubkey from locking_pubkey field
    let locking_pubkey = Self::extract_pubkey_from_share(&share)?;

    // Validate share and create EHashMintData
    let mint_data = EHashMintData {
        // ... other fields ...
        locking_pubkey,  // Required per-share pubkey from PubKey33 field
    };

    // Send to mint handler
    mint_sender.send(mint_data).await?;
    Ok(())
}
```

**Mint - Create NUT-04/NUT-20 Compliant Quote:**
```rust
async fn mint_ehash_tokens(&mut self, data: &EHashMintData) -> Result<Proofs, MintError> {
    // Convert to CDK format
    let locking_pubkey = CdkPublicKey::from(data.locking_pubkey);

    // Generate random UUID v4 quote ID (NUT-04: prevents front-running)
    let quote_id = Uuid::new_v4().to_string();

    // Create PAID quote with NUT-20 P2PK lock
    let quote = MintQuote::new(
        Some(QuoteId::BASE64(quote_id)),
        // ... other fields ...
        Some(locking_pubkey),  // Required NUT-20 P2PK lock (per-share)
        // ... other fields ...
    );

    // Store quote in database
    localstore.add_mint_quote(quote).await?;

    Ok(vec![])  // Wallet will mint via NUT-20 authenticated flow
}
```

### Protocol Flow with NUT-04 and NUT-20

1. **Connection Setup**: Miner specifies hpub in user_identity when connecting to TProxy
2. **Validation**: TProxy parses and validates hpub format (disconnect if invalid)
3. **Per-Share Submission**: TProxy sets `SubmitSharesExtended.locking_pubkey` (PubKey33 field) from downstream miner's hpub
4. **Quote Creation**: Pool/Mint creates PAID quotes with:
   - Random UUID v4 quote ID (NUT-04: prevents front-running)
   - Per-share locking pubkey from PubKey33 field (NUT-20: authorization control)
   - Share hash as payment proof (auditability)
5. **External Wallet Redemption**: Wallets use NUT-20 authenticated flow:
   - Query mint for quotes by locking pubkey (signature-authenticated)
   - Receive secret UUID quote IDs
   - Create blinded messages (normal secrets - P2PK optional)
   - Sign MintRequest with private key (NUT-20 authentication proves pubkey ownership)
   - Mint verifies signature matches quote's locking pubkey
   - Mint returns blind signatures
   - Wallet unblinds to obtain normal eHash bearer tokens

### Implementation Benefits

- **Standards Compliant**: Follows established SV2 extension patterns
- **Backward Compatible**: Non-eHash clients work normally
- **Minimal Overhead**: Only adds PubKey33 field to SubmitSharesExtended (33 bytes, always present)
- **Clean Separation**: eHash logic is clearly separated from core mining protocol##
 eHash Keyset Lifecycle Management

### Keyset State Machine

eHash keysets follow a specific lifecycle with trigger events from the Pool:

```
ACTIVE → QUANTIFYING → PAYOUT → EXPIRED
```

#### State Machine Diagram

```mermaid
stateDiagram-v2
    [*] --> ACTIVE : New keyset created
    
    ACTIVE : Minting new eHash tokens
    ACTIVE : Accepting share events
    
    QUANTIFYING : Calculating final values
    QUANTIFYING : No new minting
    
    PAYOUT : eHash ↔ sats swaps enabled
    PAYOUT : Conversion rate fixed
    
    EXPIRED : All operations disabled
    EXPIRED : Keyset archived
    
    ACTIVE --> QUANTIFYING : StartQuantification event
    QUANTIFYING --> PAYOUT : StartPayout event
    PAYOUT --> EXPIRED : ExpireKeyset event
    
    note right of ACTIVE
        Always exactly one
        ACTIVE keyset exists
    end note
    
    note right of QUANTIFYING
        Pool calculates final
        eHash token values
    end note
    
    note right of PAYOUT
        External wallets can
        swap eHash for sats
    end note
```

#### State Definitions

- **ACTIVE**: Keyset is actively minting new eHash tokens for incoming shares
- **QUANTIFYING**: Pool is calculating final eHash token values (no new minting)
- **PAYOUT**: eHash tokens can be swapped for sats (ecash or LN) at determined rates
- **EXPIRED**: Keyset is no longer valid

### Mint-Driven Lifecycle Management

The Mint monitors for payout events and manages keyset transitions autonomously:

```rust
#[derive(Debug, Clone)]
pub enum PayoutTrigger {
    /// Block found - detected via Template Provider integration
    BlockReward {
        block_height: u64,
        reward_amount: Amount,  // Total block reward in sats
        timestamp: SystemTime,
    },
    /// BOLT12 payment received - detected via LDK integration
    Bolt12Payment {
        payment_amount: Amount,  // Payment amount in sats
        payment_hash: String,    // For verification this is a mining payout
        timestamp: SystemTime,
    },
}
```

### Mint Payout Detection and Processing

The Mint monitors for payout events and manages keyset transitions:

```rust
impl MintHandler {
    /// Process ShareAccountingEvent and check for block found trigger
    pub async fn process_share_event(&mut self, event: ShareAccountingEvent) -> Result<(), MintError> {
        match event {
            ShareAccountingEvent::ShareAccepted { block_found, .. } => {
                // Mint eHash tokens for the share
                self.mint_ehash_for_share(&event).await?;
                
                // Check if this share found a block
                if block_found {
                    // Query Template Provider for detailed block reward information
                    let block_reward = self.get_block_reward_from_template_provider().await?;
                    
                    self.handle_payout_trigger(PayoutTrigger::BlockReward {
                        block_height: block_reward.height,
                        reward_amount: block_reward.amount,
                        timestamp: event.timestamp,
                    }).await?;
                }
            },
            ShareAccountingEvent::BestDifficultyUpdated { .. } => {
                // Handle difficulty updates if needed
            }
        }
        
        // Monitor LDK for BOLT12 payments
        if let Some(bolt12_payment) = self.check_ldk_for_mining_payouts().await? {
            self.handle_payout_trigger(PayoutTrigger::Bolt12Payment {
                payment_amount: bolt12_payment.amount,
                payment_hash: bolt12_payment.hash,
                timestamp: SystemTime::now(),
            }).await?;
        }
        
        Ok(())
    }
    
    pub async fn handle_payout_trigger(&mut self, trigger: PayoutTrigger) -> Result<(), MintError> {
        let active_keyset_id = self.get_active_keyset_id();
        let outstanding_ehash_amount = self.get_outstanding_ehash_amount(active_keyset_id).await?;
        
        // Calculate eHash to sats conversion rate
        let payout_amount = match trigger {
            PayoutTrigger::BlockReward { reward_amount, .. } => reward_amount,
            PayoutTrigger::Bolt12Payment { payment_amount, .. } => payment_amount,
        };
        
        let ehash_to_sats_rate = payout_amount.as_f64() / outstanding_ehash_amount.as_f64();
        
        // Transition keyset lifecycle
        self.rotate_to_new_active_keyset().await?;
        self.transition_keyset_to_quantifying(active_keyset_id).await?;
        self.transition_keyset_to_payout(active_keyset_id, ehash_to_sats_rate).await?;
        
        Ok(())
    }
    
    /// Query Template Provider for block reward details
    async fn get_block_reward_from_template_provider(&self, block_height: u64) -> Result<BlockReward, MintError> {
        // Query Template Provider for detailed block information
        // Calculate total reward (coinbase + fees) for the specified block
    }
    
    /// Check LDK for BOLT12 mining payout payments
    async fn check_ldk_for_mining_payouts(&self) -> Result<Option<Bolt12Payment>, MintError> {
        // Integration with LDK to detect BOLT12 payments
        // Filter for payments that match mining payout criteria
    }
}
```

### eHash to Sats Conversion

During the PAYOUT phase, eHash tokens can be swapped for sats:

```rust
pub struct EHashSwapRequest {
    pub ehash_tokens: Vec<Proof>,  // P2PK-locked eHash tokens
    pub target_unit: CurrencyUnit, // Sat or other supported unit
    pub swap_rate: f64,            // eHash to sats conversion rate
}

impl MintHandler {
    /// Swap eHash tokens for sats during PAYOUT phase
    pub async fn swap_ehash_for_sats(
        &mut self, 
        request: EHashSwapRequest
    ) -> Result<Vec<Proof>, MintError> {
        // 1. Verify keyset is in PAYOUT state
        // 2. Validate eHash tokens
        // 3. Calculate sats amount using swap_rate
        // 4. Mint new sats tokens
        // 5. Burn eHash tokens
    }
}
```
## Locking Pubkey Encoding

### bech32 Encoding with 'hpub' Prefix

For configuration and display purposes, locking pubkeys use bech32 encoding with the 'hpub' prefix:

```rust
use bitcoin::bech32::{self, ToBase32, FromBase32};

/// Encode a public key to bech32 format with 'hpub' prefix
pub fn encode_locking_pubkey(pubkey: &PublicKey) -> Result<String, EncodingError> {
    let data = pubkey.serialize().to_base32();
    bech32::encode("hpub", data, bech32::Variant::Bech32)
        .map_err(EncodingError::Bech32)
}

/// Decode a bech32-encoded locking pubkey with 'hpub' prefix
pub fn decode_locking_pubkey(encoded: &str) -> Result<PublicKey, EncodingError> {
    let (hrp, data, _variant) = bech32::decode(encoded)
        .map_err(EncodingError::Bech32)?;
    
    if hrp != "hpub" {
        return Err(EncodingError::InvalidPrefix(hrp));
    }
    
    let bytes = Vec::<u8>::from_base32(&data)
        .map_err(EncodingError::Bech32)?;
    
    PublicKey::from_slice(&bytes)
        .map_err(EncodingError::InvalidPubkey)
}
```

### hpub Format Specification

**Format:** Bech32 (BIP 173)

**HRP (Human Readable Part):** `hpub`

**Data:** 33-byte compressed secp256k1 public key

**Example:**
```
hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7kxqz5p9
^   ^                                             ^
|   |                                             |
|   +--- Bech32-encoded 33-byte pubkey -----------+
|
+-- Human Readable Part (identifies as hashpool pubkey)
```

**Validation:**
```rust
fn parse_hpub(hpub: &str) -> Result<PublicKey, Error> {
    // Decode bech32
    let (hrp, data) = bech32::decode(hpub)?;

    // Verify HRP
    if hrp != "hpub" {
        return Err(Error::InvalidHRP);
    }

    // Verify length (33 bytes for compressed pubkey)
    if data.len() != 33 {
        return Err(Error::InvalidLength);
    }

    // Parse as secp256k1 pubkey
    let pubkey = PublicKey::from_slice(&data)?;

    Ok(pubkey)
}
```

### Configuration Examples

#### Downstream Miner Configuration
```
# Miner specifies their hpub in user_identity when connecting to proxy
user_identity = "hpub1qw508d6qejxtdg4y5r3zarvary0c5xw7k..."
```

#### TProxy Configuration
```toml
# TProxy validates hpub from downstream miners and tracks eHash accounting
# No eHash-specific configuration needed - accounting happens automatically
# Downstream miners specify their hpub in user_identity when connecting
```

#### Pool/Mint Configuration (Phase 10 - with Bitcoin RPC)
```toml
# Pool configuration with eHash mint
[ehash_mint]
mint_url = "https://mint.hashpool.dev"
supported_units = ["HASH"]
min_leading_zeros = 32
database_url = "sqlite://ehash_mint.db"

# Bitcoin RPC configuration for block reward querying (Phase 10)
bitcoin_rpc_url = "http://127.0.0.1:8332"
bitcoin_rpc_user = "bitcoinrpc"
bitcoin_rpc_password = "your_rpc_password_here"
bitcoin_rpc_timeout_secs = 30

# Fault tolerance
max_retries = 10
backoff_multiplier = 2
recovery_enabled = true
```

### Benefits

- **Human-readable**: bech32 encoding with checksums prevents typos
- **Distinctive prefix**: 'hpub' clearly identifies hashpool locking pubkeys
- **Standard format**: Consistent with Bitcoin address encoding practices
- **Error detection**: Built-in checksum validation
- **Flexible RPC**: Supports both local and remote Bitcoin Core nodes

## Authenticated Quote Discovery by Pubkey

### Problem Statement

When the Pool creates PAID mint quotes for shares, external wallets and the wallet-proxy need to discover outstanding quote IDs. Since quote IDs are random UUIDs (NUT-04 compliant), they cannot be derived. However, allowing unauthenticated queries by pubkey would enable snooping by external parties.

### Solution: Authenticated Query API with Signature Verification

We'll use CDK's KV store to maintain pubkey-to-quote mappings and require NUT-20 signature authentication for all queries:

```rust
use cdk_common::database::KVStoreDatabase;
use bitcoin::secp256k1::{PublicKey, Signature, Message};
use cashu::quote_id::QuoteId;
use uuid::Uuid;

impl MintHandler {
    /// Store pubkey-to-quote mapping when creating PAID quotes
    async fn store_pubkey_quote_mapping(
        &mut self,
        pubkey: &PublicKey,
        quote_id: &QuoteId,
    ) -> Result<(), MintError> {
        // Encode pubkey as hex string for KV key
        let pubkey_hex = hex::encode(pubkey.serialize());

        // Use KV store with namespace structure:
        // primary_namespace: "ehash_pubkey_quotes"
        // secondary_namespace: pubkey_hex
        // key: quote_id_string
        // value: timestamp (for cleanup/expiry tracking)

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let value = timestamp.to_be_bytes();

        let mut tx = self.mint_instance.localstore.begin_transaction().await?;
        tx.kv_write(
            "ehash_pubkey_quotes",
            &pubkey_hex,
            &quote_id.to_string(),
            &value,
        ).await?;
        tx.commit().await?;

        Ok(())
    }

    /// Query quotes by pubkey with signature authentication
    async fn get_quotes_by_pubkey_authenticated(
        &self,
        pubkey: &PublicKey,
        signature: &Signature,
        message: &str,
    ) -> Result<Vec<QuoteSummary>, MintError> {
        // 1. Verify signature
        let secp = Secp256k1::verification_only();
        let msg = Message::from_hashed_data::<sha256::Hash>(message.as_bytes());

        secp.verify_ecdsa(&msg, signature, pubkey)
            .map_err(|_| MintError::InvalidSignature)?;

        // 2. Query KV store for quote IDs
        let pubkey_hex = hex::encode(pubkey.serialize());
        let quote_id_keys = self.mint_instance.localstore
            .kv_list("ehash_pubkey_quotes", &pubkey_hex)
            .await?;

        // 3. Fetch quote details for each quote_id
        let mut summaries = Vec::new();
        for key in quote_id_keys {
            if let Ok(quote_id) = QuoteId::from_str(&key) {
                if let Some(quote) = self.mint_instance.localstore
                    .get_mint_quote(&quote_id)
                    .await?
                {
                    summaries.push(QuoteSummary {
                        quote_id: quote.id.clone(),
                        amount: quote.amount.unwrap_or(Amount::ZERO),
                        unit: quote.unit.clone(),
                        state: quote.state(),
                        created_time: quote.created_time,
                        expiry: quote.expiry,
                    });
                }
            }
        }

        Ok(summaries)
    }

    /// Update mint_ehash_tokens to store mapping after quote creation
    async fn mint_ehash_tokens(&mut self, data: &EHashMintData) -> Result<Vec<Proof>, MintError> {
        // Convert to CDK format
        let locking_pubkey = CdkPublicKey::from(data.locking_pubkey);

        // Generate random UUID v4 quote ID (NUT-04)
        let quote_id_str = Uuid::new_v4().to_string();
        let quote_id = QuoteId::BASE64(quote_id_str.clone());

        // Create PAID MintQuote with NUT-20 P2PK lock
        let quote = MintQuote::new(
            Some(quote_id.clone()),
            // ... other fields ...
            Some(locking_pubkey),
            // ... other fields ...
        );

        // Store quote in database
        let mut tx = self.mint_instance.localstore.begin_transaction().await?;
        tx.add_mint_quote(quote).await?;
        tx.commit().await?;

        // Store pubkey-to-quote mapping for authenticated discovery
        self.store_pubkey_quote_mapping(&data.locking_pubkey, &quote_id).await?;

        Ok(vec![])  // Empty proofs - wallet mints via NUT-20 flow
    }
}

// EhashHandler - Track eHash accounting for downstream miners
impl EhashHandler {
    /// Process correlation data and update accounting statistics
    async fn process_correlation_data(&mut self, data: EhashCorrelationData) -> Result<(), EhashError> {
        // Update balance for this pubkey
        *self.ehash_balances.entry(data.locking_pubkey).or_insert(0) += data.ehash_amount;

        // Update channel statistics
        let stats = self.channel_stats.entry(data.channel_id).or_insert(ChannelStats {
            channel_id: data.channel_id,
            locking_pubkey: data.locking_pubkey,
            user_identity: data.user_identity.clone(),
            total_ehash: 0,
            share_count: 0,
            last_share_time: data.timestamp,
        });

        stats.total_ehash += data.ehash_amount;
        stats.share_count += 1;
        stats.last_share_time = data.timestamp;

        info!(
            "Updated eHash balance for {}: +{} (total: {})",
            data.user_identity,
            data.ehash_amount,
            stats.total_ehash
        );

        Ok(())
    }

    /// Get total eHash earned by a specific pubkey
    pub fn get_ehash_balance(&self, pubkey: &PublicKey) -> u64 {
        self.ehash_balances.get(pubkey).copied().unwrap_or(0)
    }

    /// Get accounting statistics for a channel
    pub fn get_channel_stats(&self, channel_id: u32) -> Option<ChannelStats> {
        self.channel_stats.get(&channel_id).cloned()
    }
}
```

### KV Store Namespace Structure

```
Primary Namespace: "ehash_pubkey_quotes"
├── Secondary Namespace: <pubkey_hex_1>
│   ├── Key: <quote_id_1> → Value: timestamp
│   ├── Key: <quote_id_2> → Value: timestamp
│   └── Key: <quote_id_3> → Value: timestamp
├── Secondary Namespace: <pubkey_hex_2>
│   ├── Key: <quote_id_4> → Value: timestamp
│   └── Key: <quote_id_5> → Value: timestamp
└── ...
```

**Example:**
```
ehash_pubkey_quotes/
├── 02a1234...abcd/  (compressed pubkey hex)
│   ├── 550e8400-e29b-41d4-a716-446655440000 → 1698765432
│   └── 7c9e6679-7425-40de-944b-e07fc1f90ae7 → 1698765445
└── 03def456...9876/
    └── f47ac10b-58cc-4372-a567-0e02b2c3d479 → 1698765490
```

### API Endpoints

#### Request/Response Structures
```rust
pub struct QuoteQueryRequest {
    pub pubkey: PublicKey,
    pub signature: Signature,
    pub message: String,
}

pub struct QuoteQueryResponse {
    pub quotes: Vec<QuoteSummary>,
}

pub struct QuoteSummary {
    pub quote_id: QuoteId,
    pub amount: Amount,
    pub unit: CurrencyUnit,
    pub state: MintQuoteState,
    pub created_time: u64,
    pub expiry: u64,
}
```

#### HTTP REST Endpoint

**Endpoint:** `POST /v1/ehash/quotes/by-pubkey`

**Request:**
```json
{
  "pubkey": "02a1234567890abcdef...",
  "signature": "304402...",
  "message": "ehash_quote_query_1698765432"
}
```

**Response (Success - 200 OK):**
```json
{
  "quotes": [
    {
      "quote_id": "550e8400-e29b-41d4-a716-446655440000",
      "amount": 1024,
      "unit": "HASH",
      "state": "PAID",
      "created_time": 1698765432,
      "expiry": 1698851832
    }
  ]
}
```

**Response (Auth Failure - 401 Unauthorized):**
```json
{
  "error": "Invalid signature",
  "code": 401
}
```

### Protocol Flow

```mermaid
sequenceDiagram
    participant Miner
    participant TProxy
    participant Pool
    participant Mint
    participant EhashHandler
    participant ExternalWallet

    Note over Miner, TProxy: 1. Share Submission
    Miner->>TProxy: SubmitShares
    TProxy->>Pool: SubmitSharesExtended.locking_pubkey = pubkey

    Note over Pool, Mint: 2. Mint Creates Quote & Stores Mapping
    Pool->>Pool: Validate share, calculate eHash amount
    Pool->>Mint: EHashMintData (with locking_pubkey, ehash_amount)
    Mint->>Mint: Create PAID quote with UUID v4 + P2PK lock
    Mint->>Mint: Store pubkey→quote_id mapping in KV store
    Pool->>TProxy: SubmitSharesSuccess

    Note over TProxy, EhashHandler: 3. TProxy Accounting
    TProxy->>EhashHandler: EhashCorrelationData (ehash_amount, pubkey)
    EhashHandler->>EhashHandler: Update accounting stats for display

    Note over ExternalWallet: 4. External Wallet Discovery (Periodic)
    ExternalWallet->>ExternalWallet: Create challenge message
    ExternalWallet->>ExternalWallet: Sign with private key

    Note over ExternalWallet, Mint: 5. Authenticated Quote Query
    ExternalWallet->>Mint: POST /v1/ehash/quotes/by-pubkey (pubkey, signature, message)
    Mint->>Mint: Verify signature
    Mint->>Mint: Query KV store for quote IDs
    Mint->>Mint: Fetch quote details
    Mint-->>ExternalWallet: QuoteQueryResponse (quote summaries)

    Note over ExternalWallet: 6. Token Redemption
    loop For each PAID quote
        ExternalWallet->>ExternalWallet: Create blinded messages (normal secrets)
        ExternalWallet->>ExternalWallet: Sign MintRequest (NUT-20 - proves pubkey ownership)
        ExternalWallet->>Mint: POST /v1/mint (quote_id, blinded_messages, signature)
        Mint->>Mint: Verify signature matches quote's locking pubkey
        Mint-->>ExternalWallet: Blind signatures
        ExternalWallet->>ExternalWallet: Unblind to get normal eHash tokens
    end
```

### Cleanup and Maintenance

**Optional Cleanup Strategy:**
```rust
impl MintHandler {
    /// Remove pubkey-to-quote mapping when quote is fully redeemed or expired
    async fn cleanup_quote_mapping(
        &mut self,
        pubkey: &PublicKey,
        quote_id: &QuoteId,
    ) -> Result<(), MintError> {
        let pubkey_hex = hex::encode(pubkey.serialize());

        let mut tx = self.mint_instance.localstore.begin_transaction().await?;
        tx.kv_remove(
            "ehash_pubkey_quotes",
            &pubkey_hex,
            &quote_id.to_string(),
        ).await?;
        tx.commit().await?;

        Ok(())
    }

    /// Periodic cleanup of expired quote mappings
    async fn cleanup_expired_mappings(&mut self) -> Result<(), MintError> {
        // List all pubkey namespaces
        let pubkeys = self.mint_instance.localstore
            .kv_list("ehash_pubkey_quotes", "")
            .await?;

        for pubkey_hex in pubkeys {
            // Get all quote IDs for this pubkey
            let quote_id_keys = self.mint_instance.localstore
                .kv_list("ehash_pubkey_quotes", &pubkey_hex)
                .await?;

            for quote_id_key in quote_id_keys {
                if let Ok(quote_id) = QuoteId::from_str(&quote_id_key) {
                    // Check if quote is expired or fully redeemed
                    if let Some(quote) = self.mint_instance.localstore
                        .get_mint_quote(&quote_id)
                        .await?
                    {
                        if quote.state() == MintQuoteState::Issued
                            || SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() > quote.expiry
                        {
                            // Remove mapping for expired/redeemed quotes
                            let pubkey = PublicKey::from_slice(
                                &hex::decode(&pubkey_hex).unwrap()
                            ).unwrap();
                            self.cleanup_quote_mapping(&pubkey, &quote_id).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
```

### Benefits

- **Privacy**: Signature authentication prevents unauthorized snooping of quotes
- **Security**: Only pubkey owners can discover their quotes (proof of ownership required)
- **NUT-04 Compliant**: Random UUID quote IDs remain secret until authenticated query
- **Scalable**: KV store indexes provide O(1) lookup by pubkey
- **Recovery**: TProxy can rediscover quotes after restart by querying with signature
- **External Wallet Support**: Any wallet with the private key can discover and redeem quotes