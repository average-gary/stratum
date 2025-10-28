# Implementation Tasks - eHash Persistence

This document breaks down the eHash persistence implementation into small, focused tasks that can be implemented as minimal commits. Each task is designed to be independently reviewable and testable.

## Phase 1: Foundation - Shared Module Setup

### 1.1 Create common/ehash crate scaffold
- [x] Create `common/ehash/Cargo.toml` with basic dependencies
- [x] Create `common/ehash/src/lib.rs` with module structure
- [x] Add ehash crate to workspace `Cargo.toml`
- **Requirements**: 1.1, 4.1
- **Files**: `common/ehash/Cargo.toml`, `common/ehash/src/lib.rs`, `Cargo.toml`

### 1.2 Add CDK dependencies
- [x] Add CDK mint and wallet dependencies to `common/ehash/Cargo.toml`
- [x] Add CDK re-exports to `common/ehash/src/lib.rs`
- [x] Verify CDK compiles with required features
- **Requirements**: 4.1, 4.2
- **Files**: `common/ehash/Cargo.toml`

### 1.3 Add hashpool ehash protocol dependencies
- [x] Add ehash protocol dependency from `deps/hashpool/protocols/ehash`
- [x] Re-export core hashpool functions (`calculate_ehash_amount`, `calculate_difficulty`)
- [x] Add basic documentation for hashpool integration
- **Requirements**: 1.1, 1.3
- **Files**: `common/ehash/Cargo.toml`, `common/ehash/src/lib.rs`
- **Note**: Implemented work calculation functions directly in `work.rs` instead of using external dependency to avoid binary_sv2/sv2 protocol complexity. Functions are functionally identical to hashpool implementation.

## Phase 2: Core Data Structures

### 2.1 Define EHashMintData structure
- [x] Create `common/ehash/src/types.rs` with `EHashMintData` struct
- [x] Add all required fields (share_hash, channel_id, user_identity, target, sequence_number, etc.)
- [x] Add required `locking_pubkey: bitcoin::secp256k1::PublicKey` field for per-share NUT-20 P2PK locking
- [x] Implement Clone and Debug traits
- **Requirements**: 1.2, 2.1, 3.1
- **Files**: `common/ehash/src/types.rs`
- **Note**: Per NUT-04 and NUT-20, each share MUST include a locking pubkey. The pubkey is extracted from the direct `locking_pubkey: PubKey33` field in SubmitSharesExtended messages (see Task 5.4).

### 2.2 Add EHashMintData helper methods
- [x] Implement `calculate_ehash_amount(&self, min_leading_zeros: u32) -> Amount`
- [x] Add conversion from share hash bytes to hashpool format
- [x] Add timestamp and block_found accessors
- **Requirements**: 1.3, 3.2
- **Files**: `common/ehash/src/types.rs`

### 2.3 Define WalletCorrelationData structure
- [x] Add `WalletCorrelationData` struct to `types.rs`
- [x] Include channel_id, sequence_number, user_identity, ehash_tokens_minted
- [x] Implement Clone and Debug traits
- **Requirements**: 3.4
- **Files**: `common/ehash/src/types.rs`

### 2.4 Define configuration structures
- [x] Create `common/ehash/src/config.rs` with `MintConfig` struct
- [x] Add `WalletConfig` struct with locking_pubkey support
- [x] Add `JdcEHashConfig` with mode enum (Mint/Wallet)
- **Requirements**: 5.1, 5.2, 5.3
- **Files**: `common/ehash/src/config.rs`

### 2.5 Define error types
- [x] Create `common/ehash/src/error.rs` with `MintError` enum
- [x] Add `WalletError` enum
- [x] Implement Display and Error traits
- **Requirements**: 4.2, 6.4
- **Files**: `common/ehash/src/error.rs`

### 2.6 Add unit tests for data structures
- [x] Test EHashMintData creation and eHash calculation
- [x] Test WalletCorrelationData creation
- [x] Test configuration deserialization from TOML
- **Requirements**: 1.3, 3.2
- **Files**: `common/ehash/src/types.rs`, `common/ehash/src/config.rs`

## Phase 3: MintHandler Implementation

### 3.1 Create MintHandler structure scaffold
- [x] Create `common/ehash/src/mint.rs` with `MintHandler` struct
- [x] Add async channel fields (receiver, sender)
- [x] Add CDK Mint instance field
- [x] Add basic constructor `new(config, status_tx)`
- **Requirements**: 1.1, 1.5, 7.1
- **Files**: `common/ehash/src/mint.rs`

### 3.2 Implement MintHandler initialization
- [x] Initialize CDK Mint with database backend
- [x] Configure "HASH" currency unit
- [x] Set up async channels for EHashMintData
- [x] Add get_sender() and get_receiver() methods
- **Requirements**: 4.1, 4.3
- **Files**: `common/ehash/src/mint.rs`

### 3.3 Add mint processing core logic
- [x] Implement `process_mint_data(&mut self, data: EHashMintData)`
- [x] Create MintQuote in PAID state using CDK
- [x] Calculate eHash amount from share hash
- [x] Return minted token proofs
- **Requirements**: 1.2, 1.3
- **Files**: `common/ehash/src/mint.rs`

### 3.4 Implement NUT-04 and NUT-20 compliant P2PK quote creation
- [x] Generate random UUID v4 quote IDs (NUT-04: prevents front-running attacks)
- [x] Create PAID MintQuotes with per-share locking_pubkey from EHashMintData (NUT-20)
- [x] Store share hash as payment proof for auditability
- [x] Return empty proofs (wallet mints via NUT-20 authenticated flow)
- **Requirements**: 2.1, 8.1, 8.2, 8.5, 8.6, 8.7, 8.8
- **Files**: `common/ehash/src/mint.rs`
- **Note**: Per NUT-04, quote IDs MUST be random and NOT derivable. Per NUT-20, P2PK locks enforce authentication. Each share has its own pubkey (per-share granularity). External wallets query quotes by pubkey, sign MintRequest with their private key, and receive P2PK-locked tokens after NUT-20 signature verification.

### 3.5 Add block found event handling
- [x] Implement `handle_block_found(&mut self, data: &EHashMintData)`
- [x] Trigger keyset lifecycle transition
- [x] Query Template Provider for block reward (stub for now)
- **Requirements**: 9.1, 9.2
- **Files**: `common/ehash/src/mint.rs`

### 3.6 Implement MintHandler run loop
- [x] Add `run(&mut self)` method with async channel receiver loop
- [x] Process incoming EHashMintData events
- [x] Call process_mint_data for each event
- **Requirements**: 1.2, 3.3
- **Files**: `common/ehash/src/mint.rs`

### 3.7 Add fault tolerance - retry queue
- [x] Add retry_queue field to MintHandler
- [x] Add failure_count and last_failure tracking
- [x] Implement `process_mint_data_with_retry` wrapper
- [x] Add unit tests for retry queue functionality
- **Requirements**: 6.1, 6.3
- **Files**: `common/ehash/src/mint.rs`

### 3.8 Add fault tolerance - exponential backoff
- [x] Add backoff_multiplier and max_retries config
- [x] Implement `attempt_recovery()` method
- [x] Calculate exponential backoff duration
- **Requirements**: 6.1, 6.5
- **Files**: `common/ehash/src/mint.rs`

### 3.9 Add graceful shutdown support
- [x] Implement `run_with_shutdown(shutdown_rx)` method
- [x] Add tokio::select! for shutdown signal handling
- [x] Implement `shutdown()` to complete pending operations
- **Requirements**: 6.1, 6.5
- **Files**: `common/ehash/src/mint.rs`

### 3.10 Add hpub utility functions
- [x] Create `common/ehash/src/hpub.rs` with hpub encoding/decoding functions
- [x] Implement `parse_hpub(hpub: &str) -> Result<PublicKey, Error>` with bech32 validation
- [x] Implement `encode_hpub(pubkey: &PublicKey) -> String` for configuration
- [x] Add validation for 'hpub' HRP and 33-byte pubkey length
- [x] Add unit tests for hpub encoding/decoding
- **Requirements**: 2.1, 2.2, 2.3
- **Files**: `common/ehash/src/hpub.rs`

### 3.11 Add MintHandler unit tests
- [x] Test CDK initialization and configuration
- [x] Test P2PK token creation with per-share pubkeys
- [x] Test retry queue and backoff logic
- [x] Test graceful shutdown
- **Requirements**: 1.5, 6.1
- **Files**: `common/ehash/src/mint.rs`

## Phase 4: WalletHandler Implementation

### 4.1 Create WalletHandler structure scaffold
- [x] Create `common/ehash/src/wallet.rs` with `WalletHandler` struct
- [x] Add async channel fields for WalletCorrelationData
- [x] Add optional CDK Wallet instance field
- [x] Add ehash_balances and channel_stats fields (tracking per-miner accounting)
- **Requirements**: 7.5, 7.6
- **Files**: `common/ehash/src/wallet.rs`
- **Note**: WalletHandler tracks eHash accounting for multiple downstream miners (multi-miner support), does NOT redeem tokens (external wallets handle redemption)

### 4.2 Implement WalletHandler initialization
- [x] Add constructor `new(config)`
- [x] Initialize optional CDK Wallet for "HASH" unit (if mint_url provided)
- [x] Set up async channels for WalletCorrelationData
- [x] Initialize ehash_balances HashMap for tracking eHash per pubkey
- [x] Initialize channel_stats HashMap for tracking per-channel statistics
- **Requirements**: 8.1
- **Files**: `common/ehash/src/wallet.rs`
- **Note**: TProxy tracks multiple downstream miners' pubkeys (from user_identity hpubs), so no single locking_pubkey config. Pubkeys come from correlation events.

### 4.3 Add correlation processing logic
- [x] Implement `process_correlation_data(&mut self, data: WalletCorrelationData)`
- [x] Track ehash_tokens_minted counter per downstream miner's pubkey
- [x] Update channel statistics (share count, last activity, total eHash)
- [x] Log correlation events for display/monitoring
- **Requirements**: 3.4
- **Files**: `common/ehash/src/wallet.rs`
- **Note**: Accounting/tracking only - external wallets handle redemption via authenticated mint API

### 4.4 Add P2PK token query support
- [x] Implement `query_p2pk_tokens(&self, pubkey) -> Vec<Proof>`
- [x] Check if CDK Wallet is configured
- [x] Add stub for querying P2PK-locked tokens (full CDK integration deferred)
- [x] Return empty vec if wallet not configured
- **Requirements**: 8.2, 8.3
- **Files**: `common/ehash/src/wallet.rs`
- **Note**: Full CDK Wallet query implementation can be added when needed. External wallets query mint directly via authenticated API.

### 4.5 Implement WalletHandler run loop
- [x] Add `run(&mut self)` method with async channel receiver loop
- [x] Process incoming WalletCorrelationData events
- [x] Call process_correlation_data_with_retry for each event
- [x] Add periodic recovery attempt for retry queue
- **Requirements**: 3.4
- **Files**: `common/ehash/src/wallet.rs`

### 4.6 Add fault tolerance - retry queue
- [x] Add retry_queue field to WalletHandler (VecDeque)
- [x] Add failure tracking fields (failure_count, last_failure)
- [x] Implement `process_correlation_data_with_retry` wrapper
- [x] Queue failed operations for retry
- **Requirements**: 6.2, 6.3
- **Files**: `common/ehash/src/wallet.rs`

### 4.7 Add fault tolerance - recovery logic
- [x] Use recovery_enabled config option from WalletConfig
- [x] Implement `attempt_recovery()` method
- [x] Process retry queue with exponential backoff
- [x] Stop on first failure and wait for next attempt
- **Requirements**: 6.2, 6.5
- **Files**: `common/ehash/src/wallet.rs`

### 4.8 Add graceful shutdown support
- [x] Implement `run_with_shutdown(shutdown_rx)` method
- [x] Add tokio::select! for shutdown signal handling
- [x] Implement `shutdown()` to complete pending operations in retry queue
- [x] Process all remaining retry queue items before terminating
- **Requirements**: 6.2, 6.5
- **Files**: `common/ehash/src/wallet.rs`

### 4.9 Add pubkey accessors
- [x] Implement `get_ehash_balance(pubkey) -> u64` (get balance for specific downstream miner)
- [x] Implement `get_channel_stats(channel_id) -> Option<&ChannelStats>` (get stats for specific channel)
- [x] Implement `get_all_balances() -> &HashMap<PublicKey, u64>` (all downstream miners' balances)
- [x] Implement `get_all_channel_stats() -> &HashMap<u32, ChannelStats>` (all channel statistics)
- **Requirements**: 2.4, 8.1
- **Files**: `common/ehash/src/wallet.rs`
- **Note**: Accessors provide read-only access to accounting data for display/monitoring purposes

### 4.10 Add WalletHandler unit tests
- [x] Test wallet initialization and configuration
- [x] Test wallet initialization with mint_url
- [x] Test correlation data processing (single and multiple events)
- [x] Test multi-miner support (multiple downstream miners with different pubkeys)
- [x] Test retry queue functionality
- [x] Test channel sender/receiver
- [x] Test graceful shutdown with retry queue processing
- [x] Test P2PK token query (returns empty when wallet not configured)
- [x] Test all balance and stats accessors
- **Requirements**: 8.1, 8.5, 6.2
- **Files**: `common/ehash/src/wallet.rs`
- **Test Results**: 16 tests passing (all wallet-related tests)

## Phase 5: Pool Role Integration

### 5.1 Add MintConfig to Pool TOML config
- [x] Extend Pool configuration structs to include optional MintConfig
- [x] Add deserialization support
- [x] Document configuration options
- **Requirements**: 5.1, 5.5
- **Files**: `roles/pool/src/lib/config.rs`, `config-examples/pool-config-local-tp-with-ehash-example.toml`

### 5.2 Add mint thread spawning function
- [x] Create `spawn_mint_thread(task_manager, config, status_tx)` helper
- [x] Instantiate MintHandler
- [x] Spawn thread using task_manager
- [x] Return sender channel
- **Requirements**: 1.1, 7.3
- **Files**: `roles/pool/src/lib/mod.rs`

### 5.3 Integrate mint_sender into Pool initialization
- [x] Modify Pool initialization to call spawn_mint_thread if configured
- [x] Pass mint_sender to ChannelManager
- [x] No channel_pubkeys HashMap needed (per-share pubkeys from direct PubKey33 field)
- **Requirements**: 5.1
- **Files**: `roles/pool/src/lib/mod.rs`
- **Implementation Note**: Uses direct PubKey33 field in SubmitSharesExtended (Task 5.4)

### 5.4 Add PubKey33 field to SubmitSharesExtended and extraction to ChannelManager
- [x] Add PubKey33 type alias to binary-sv2 datatypes (33-byte compressed secp256k1)
- [x] Implement full codec support for PubKey33 (Encodable, Decodable, GetMarker traits)
- [x] Add `locking_pubkey: PubKey33<'decoder>` field to SubmitSharesExtended message
- [x] Implement `extract_pubkey_from_share(&self, msg: &SubmitSharesExtended)` method in ChannelManager
- [x] Validate pubkey format (reject all-zeros, validate secp256k1)
- [x] Return error if pubkey missing or invalid
- **Requirements**: 2.5, 2.6
- **Files**: `protocols/v2/binary-sv2/src/datatypes/non_copy_data_types/mod.rs`, `protocols/v2/binary-sv2/src/codec/decodable.rs`, `protocols/v2/binary-sv2/src/codec/encodable.rs`, `protocols/v2/binary-sv2/src/codec/impls.rs`, `protocols/v2/subprotocols/mining/src/submit_shares.rs`, `roles/pool/src/lib/channel_manager/mod.rs`
- **Implementation Note**: Used direct PubKey33 field instead of TLV 0x0004 for cleaner, more type-safe protocol extension. PubKey33 is a fixed-size 33-byte field integrated into the Stratum v2 codec system.

### 5.5 Hook share validation in handle_submit_shares_standard
- [x] Task not applicable - Standard channels don't support eHash per protocol design
- [x] Only extended channels include locking_pubkey field for per-share P2PK authentication
- **Requirements**: 1.2, 3.1, 3.3, 6.1
- **Files**: Pool message handler for SubmitSharesStandard
- **Implementation Note**: Standard channels don't include locking_pubkey (extended channels only)

### 5.6 Hook share validation in handle_submit_shares_extended
- [x] Extract share hash from ShareValidationResult::Valid
- [x] Extract locking_pubkey from msg.locking_pubkey field using extract_pubkey_from_share()
- [x] Create EHashMintData with all required fields including locking_pubkey
- [x] Send via mint_sender.try_send() (non-blocking)
- [x] Log errors but continue mining
- **Requirements**: 1.2, 2.5, 2.6, 3.1, 3.3, 6.1
- **Files**: `roles/pool/src/lib/channel_manager/mining_message_handler.rs` (lines 692-766)

### 5.7 Handle BlockFound variant
- [x] Extract share hash, template_id, coinbase from BlockFound
- [x] Create EHashMintData with block_found=true
- [x] Send to mint_sender for keyset lifecycle trigger
- **Requirements**: 9.1
- **Files**: `roles/pool/src/lib/channel_manager/mining_message_handler.rs` (lines 740-766)

### 5.8 Add Pool integration tests
- [x] Test configuration parsing with eHash mint enabled
- [x] Test configuration parsing without eHash mint (optional section)
- [x] Test optional fields have sensible defaults
- **Requirements**: 1.6, 6.1
- **Files**: `roles/pool/tests/config_test.rs`
- **Test Results**: 3 tests passing (all config-related tests)

## Phase 6: TProxy Role Integration

### 6.1 Add WalletConfig to TProxy TOML config
- [x] Extend TProxy configuration structs to include WalletConfig
- [x] Add ehash_wallet and default_locking_pubkey fields (hpub format)
- [x] Add deserialization support
- [x] Add validation requiring default_locking_pubkey when ehash_wallet configured
- [x] Create example config file with eHash support
- **Requirements**: 5.2, 5.5
- **Files**: `roles/translator/src/lib/config.rs`, `config-examples/tproxy-config-local-pool-with-ehash-example.toml`
- **Implementation Note**: Used hpub format (bech32-encoded) for pubkeys instead of hex

### 6.2 Add wallet thread spawning function
- [x] Create `spawn_wallet_thread(task_manager, config, status_tx)` helper
- [x] Instantiate WalletHandler
- [x] Spawn thread using task_manager
- [x] Return sender channel
- **Requirements**: 7.6
- **Files**: `roles/translator/src/lib/mod.rs:283-311`

### 6.3 Integrate wallet_sender into TProxy initialization
- [x] Modify TProxy initialization to call spawn_wallet_thread if configured
- [x] Store wallet_sender in ChannelManagerData
- [x] Add wallet_sender parameter to ChannelManager::new()
- [x] Spawn wallet thread at startup when ehash_wallet configured
- **Requirements**: 5.2
- **Files**: `roles/translator/src/lib/mod.rs:108-144`, `roles/translator/src/lib/sv2/channel_manager/data.rs`, `roles/translator/src/lib/sv2/channel_manager/channel_manager.rs`

### 6.4 Hook SubmitSharesSuccess message handling
- [x] Add hook point in handle_submit_shares_success
- [x] Check for wallet_sender configuration
- [x] Add TODO comments for full correlation tracking implementation
- [x] Infrastructure ready for extracting ehash_tokens_minted from TLV
- [ ] TODO: Implement sequence_number -> downstream_id tracking for correlation
- [ ] TODO: Extract ehash_tokens_minted from TLV (default 0 if not present)
- [ ] TODO: Create and send WalletCorrelationData
- **Requirements**: 3.4, 8.2
- **Files**: `roles/translator/src/lib/sv2/channel_manager/message_handler.rs:289-342`
- **Implementation Note**: Hook infrastructure in place, needs downstream correlation tracking

### 6.5 Add TProxy integration tests
- [ ] Test wallet thread spawning and initialization
- [ ] Test SubmitSharesSuccess creates correlation events
- [ ] Test translation continues during wallet failures
- **Requirements**: 6.2
- **Files**: TProxy integration tests

### 6.6 Implement hpub extraction from miner username
- [ ] Create `extract_hpub_from_username()` helper function
- [ ] Parse miner username in `handle_authorize()` to extract hpub
- [ ] Support multiple formats: direct hpub, HIP-2 format (username.hpub1...), custom formats
- [ ] Use `ehash_integration::hpub::parse_hpub()` to validate and decode
- [ ] Fall back to `config.decode_default_locking_pubkey()` if extraction fails
- [ ] Store extracted pubkey in `DownstreamData.locking_pubkey` for share submissions
- **Requirements**: 2.5, 5.2
- **Files**: `roles/translator/src/lib/sv1/downstream/message_handler.rs` (handle_authorize function)
- **Implementation Note**: Enables per-miner eHash accounting where each downstream miner can have their own locking pubkey for receiving eHash tokens

## Phase 7: JDC Role Integration

### 7.1 Add JdcEHashConfig to JDC TOML config
- [ ] Add JdcEHashConfig with mode enum and optional mint/wallet configs
- [ ] Add deserialization support
- [ ] Document configuration options
- **Requirements**: 5.3, 5.4
- **Files**: `roles/jd-client/src/lib.rs`, example config files

### 7.2 Add JDC mint mode support
- [ ] Add mint thread spawning when mode=Mint
- [ ] Hook share validation to create mint events
- [ ] Integrate mint_sender into JDC ChannelManager
- **Requirements**: 7.4
- **Files**: `roles/jd-client/src/lib.rs`

### 7.3 Add JDC wallet mode support
- [ ] Add wallet thread spawning when mode=Wallet
- [ ] Hook SubmitSharesSuccess to create correlation events
- [ ] Integrate wallet_sender into JDC context
- **Requirements**: 7.5
- **Files**: `roles/jd-client/src/lib.rs`

### 7.4 Add JDC integration tests
- [ ] Test JDC mint mode configuration and operation
- [ ] Test JDC wallet mode configuration and operation
- [ ] Test configuration validation
- **Requirements**: 7.1, 7.2, 5.6
- **Files**: JDC integration tests

## Phase 8: Per-Share NUT-20 P2PK Protocol Implementation

### 8.1 Add hpub validation to TProxy downstream connection handling
- [ ] Implement hpub parsing from user_identity in OpenExtendedMiningChannel
- [ ] Validate bech32 format with 'hpub' HRP
- [ ] Extract secp256k1 public key from hpub
- [ ] Disconnect client if hpub invalid (no jobs sent)
- [ ] Store pubkey for channel if valid
- **Requirements**: 2.1, 2.2, 2.3
- **Files**: TProxy downstream connection handler

### 8.2 Add locking_pubkey to SubmitSharesExtended in TProxy
- [x] Use PubKey33 field in SubmitSharesExtended message (completed in Task 5.4)
- [ ] Extract pubkey from channel's stored hpub
- [ ] Set msg.locking_pubkey to 33-byte compressed secp256k1 pubkey when submitting upstream
- [ ] Add to SubmitSharesExtended message construction
- **Requirements**: 2.5, 2.6
- **Files**: TProxy upstream share submission
- **Implementation Note**: Uses direct PubKey33 field instead of TLV 0x0004

### 8.3 Add locking_pubkey extraction in Pool
- [x] PubKey33 field extraction implemented in Task 5.4 via extract_pubkey_from_share()
- [x] Validation implemented (rejects all-zeros, validates secp256k1 format)
- [ ] Integrate into share submission handler
- **Requirements**: 2.6
- **Files**: Pool share submission handler (completed in `roles/pool/src/lib/channel_manager/mod.rs:534-571`)
- **Implementation Note**: Direct field extraction instead of TLV parsing

### 8.4 Update Pool share validation to include locking_pubkey
- [ ] Call extract_pubkey_from_share() for SubmitSharesExtended messages
- [ ] Pass extracted pubkey to EHashMintData creation
- [ ] Include locking_pubkey in all mint events
- [ ] Ensure pubkey flows through to mint quote creation
- **Requirements**: 2.6
- **Files**: Pool share validation integration

### 8.5 Add hpub configuration examples
- [ ] Document hpub format in TProxy configuration
- [ ] Provide example hpub values for testing
- [ ] Document disconnection behavior for invalid hpub
- **Requirements**: 2.1, 2.2, 2.3
- **Files**: Configuration documentation, example configs

### 8.6 Add per-share protocol integration tests
- [ ] Test hpub validation and disconnection on invalid format
- [ ] Test PubKey33 field encoding/decoding in SubmitSharesExtended
- [ ] Test per-share pubkey extraction and mint quote creation
- [ ] Test multi-miner support (different hpubs per downstream)
- [ ] Test all-zeros pubkey rejection
- [ ] Test invalid secp256k1 pubkey rejection
- **Requirements**: 2.1, 2.5, 2.7, 2.8
- **Files**: Protocol integration tests

## Phase 9: Integration Testing

### 9.1 Add end-to-end Pool→Mint→Wallet test
- [ ] Set up test Pool with mint configuration
- [ ] Set up test TProxy with wallet configuration
- [ ] Submit shares and verify mint events
- [ ] Verify P2PK token creation
- **Requirements**: 1.6, 8.4
- **Files**: Integration test suite

### 9.2 Test external wallet redemption flow
- [ ] Create external wallet with locking keypair
- [ ] Query mint for P2PK-locked tokens
- [ ] Verify token redemption using CDK
- **Requirements**: 8.3, 8.4, 8.5
- **Files**: Integration test suite

### 9.3 Test fault tolerance and recovery
- [ ] Test mint failures don't affect mining
- [ ] Test wallet failures don't affect translation
- [ ] Test automatic recovery mechanisms
- [ ] Test retry queue processing
- **Requirements**: 6.1, 6.2, 6.3, 6.5
- **Files**: Integration test suite

### 9.4 Test graceful shutdown
- [ ] Test mint thread completes pending operations on shutdown
- [ ] Test wallet thread completes pending operations on shutdown
- [ ] Verify no data loss during shutdown
- **Requirements**: 6.5
- **Files**: Integration test suite

### 9.5 Test JDC dual mode operation
- [ ] Test JDC mint mode with share processing
- [ ] Test JDC wallet mode with correlation tracking
- [ ] Test mode switching via configuration
- **Requirements**: 7.1, 7.2, 7.7
- **Files**: Integration test suite

## Phase 10: Keyset Lifecycle (Future)

_Note: This phase implements the full keyset lifecycle management. Can be deferred until basic minting is working._

### 10.1 Add keyset lifecycle data structures
- [ ] Define KeysetState enum (ACTIVE, QUANTIFYING, PAYOUT, EXPIRED)
- [ ] Add KeysetInfo struct with state and metadata
- [ ] Add keyset tracking to MintHandler
- **Requirements**: 9.1, 9.2
- **Files**: `common/ehash/src/keyset.rs`

### 10.2 Implement keyset rotation on block found
- [ ] Handle block_found=true in MintHandler
- [ ] Create new ACTIVE keyset
- [ ] Transition previous keyset to QUANTIFYING
- **Requirements**: 9.1, 9.3
- **Files**: `common/ehash/src/mint.rs`

### 10.3 Add Bitcoin RPC integration for block reward querying
- [ ] Add `bitcoincore-rpc` dependency to `common/ehash/Cargo.toml`
- [ ] Add RPC configuration to `MintConfig` (endpoint, auth, timeout)
- [ ] Implement `BitcoinRpcClient` wrapper in `common/ehash/src/rpc.rs`
- [ ] Use `EHashMintData.share_hash` directly as block hash (when block_found=true, share_hash IS the block hash)
- [ ] Call Bitcoin RPC `getblock(block_hash, verbosity=2)` to fetch full block data
- [ ] Parse coinbase transaction output value to extract block reward amount
- [ ] Calculate total transaction fees from block data (sum of all tx fees)
- [ ] Replace `query_template_provider_stub()` with `query_block_reward_rpc()`
- [ ] Add error handling for RPC connection failures and timeouts
- [ ] Add configuration validation for RPC credentials
- **Requirements**: 9.1
- **Files**: `common/ehash/src/mint.rs`, `common/ehash/src/rpc.rs`, `common/ehash/src/config.rs`
- **Implementation Notes**:
  - Use async RPC client for non-blocking calls
  - Block hash is already available as share_hash in EHashMintData
  - Cache block reward data to avoid redundant RPC calls
  - Support multiple RPC endpoints for failover
  - Handle Bitcoin Core sync status (don't query unconfirmed blocks)

### 10.4 Add BOLT12 payment detection (stub)
- [ ] Define PayoutTrigger enum with BlockReward and Bolt12Payment
- [ ] Add stub for LDK integration
- [ ] Handle payout trigger in keyset lifecycle
- **Requirements**: 9.2
- **Files**: `common/ehash/src/mint.rs`

### 10.5 Implement quantification phase
- [ ] Calculate eHash-to-sats conversion rate
- [ ] Store conversion rate with keyset
- [ ] Transition keyset to PAYOUT state
- **Requirements**: 9.4
- **Files**: `common/ehash/src/mint.rs`

### 10.6 Implement eHash to sats swap
- [ ] Add swap_ehash_for_sats method
- [ ] Verify keyset is in PAYOUT state
- [ ] Calculate sats amount using conversion rate
- [ ] Mint sats tokens and burn eHash tokens
- **Requirements**: 9.5
- **Files**: `common/ehash/src/mint.rs`

### 10.7 Implement keyset expiration
- [ ] Add timeout or redemption completion tracking
- [ ] Transition keyset to EXPIRED state
- [ ] Archive keyset metadata
- **Requirements**: 9.6
- **Files**: `common/ehash/src/mint.rs`

### 10.8 Add keyset lifecycle tests
- [ ] Test keyset rotation on block found
- [ ] Test conversion rate calculation
- [ ] Test eHash to sats swap
- [ ] Test keyset expiration
- **Requirements**: 9.1-9.6
- **Files**: `common/ehash/src/keyset.rs`, `common/ehash/src/mint.rs`

## Phase 11: Pubkey Encoding (Future)

_Note: This phase implements bech32 encoding for locking pubkeys. Can be deferred._

### 11.1 Add bech32 encoding module
- [ ] Create `common/ehash/src/encoding.rs`
- [ ] Implement `encode_locking_pubkey(pubkey) -> String` with 'hpub' prefix
- [ ] Implement `decode_locking_pubkey(encoded) -> PublicKey`
- **Files**: `common/ehash/src/encoding.rs`

### 11.2 Update config parsing
- [ ] Parse bech32 locking pubkeys from TOML config
- [ ] Validate 'hpub' prefix
- [ ] Convert to raw PublicKey for internal use
- **Files**: `common/ehash/src/config.rs`

### 11.3 Add encoding tests
- [ ] Test round-trip encoding/decoding
- [ ] Test invalid prefix handling
- [ ] Test checksum validation
- **Files**: `common/ehash/src/encoding.rs`

## Notes

- Tasks marked with `*` in the original plan have been expanded into multiple focused tasks
- Each task should result in a minimal, reviewable commit
- Phase 10 (Keyset Lifecycle) and Phase 11 (Pubkey Encoding) can be deferred until basic functionality is working
- All tests should be run after each commit to ensure no regressions
- Use `git add -p` for careful staging of changes
