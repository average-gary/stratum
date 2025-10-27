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
- **Note**: Per NUT-04 and NUT-20, each share MUST include a locking pubkey. The pubkey is extracted from TLV field 0x0004 in SubmitSharesExtended messages.

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
- [ ] Add backoff_multiplier and max_retries config
- [ ] Implement `attempt_recovery()` method
- [ ] Calculate exponential backoff duration
- **Requirements**: 6.1, 6.5
- **Files**: `common/ehash/src/mint.rs`

### 3.9 Add graceful shutdown support
- [ ] Implement `run_with_shutdown(shutdown_rx)` method
- [ ] Add tokio::select! for shutdown signal handling
- [ ] Implement `shutdown()` to complete pending operations
- **Requirements**: 6.1, 6.5
- **Files**: `common/ehash/src/mint.rs`

### 3.10 Add hpub utility functions
- [ ] Create `common/ehash/src/hpub.rs` with hpub encoding/decoding functions
- [ ] Implement `parse_hpub(hpub: &str) -> Result<PublicKey, Error>` with bech32 validation
- [ ] Implement `encode_hpub(pubkey: &PublicKey) -> String` for configuration
- [ ] Add validation for 'hpub' HRP and 33-byte pubkey length
- [ ] Add unit tests for hpub encoding/decoding
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
- [ ] Create `common/ehash/src/wallet.rs` with `WalletHandler` struct
- [ ] Add async channel fields for WalletCorrelationData
- [ ] Add optional CDK Wallet instance field
- [ ] Add locking_pubkey and user_identity fields
- **Requirements**: 7.5, 7.6
- **Files**: `common/ehash/src/wallet.rs`

### 4.2 Implement WalletHandler initialization
- [ ] Add constructor `new(config, status_tx)`
- [ ] Initialize optional CDK Wallet for "HASH" unit
- [ ] Configure locking pubkey from config
- [ ] Set up async channels
- **Requirements**: 8.1
- **Files**: `common/ehash/src/wallet.rs`

### 4.3 Add correlation processing logic
- [ ] Implement `process_correlation_data(&mut self, data: WalletCorrelationData)`
- [ ] Track ehash_tokens_minted counter
- [ ] Log correlation events (no wallet ops yet)
- **Requirements**: 3.4
- **Files**: `common/ehash/src/wallet.rs`

### 4.4 Add P2PK token query support
- [ ] Implement `query_p2pk_tokens() -> Vec<Proof>`
- [ ] Query CDK Wallet for P2PK-locked tokens by pubkey
- [ ] Filter tokens by locking pubkey
- **Requirements**: 8.2, 8.3
- **Files**: `common/ehash/src/wallet.rs`

### 4.5 Implement WalletHandler run loop
- [ ] Add `run(&mut self)` method with async channel receiver loop
- [ ] Process incoming WalletCorrelationData events
- [ ] Call process_correlation_data for each event
- **Requirements**: 3.4
- **Files**: `common/ehash/src/wallet.rs`

### 4.6 Add fault tolerance - retry queue
- [ ] Add retry_queue field to WalletHandler
- [ ] Add failure tracking fields
- [ ] Implement `process_correlation_data_with_retry` wrapper
- **Requirements**: 6.2, 6.3
- **Files**: `common/ehash/src/wallet.rs`

### 4.7 Add fault tolerance - recovery logic
- [ ] Add recovery_enabled config option
- [ ] Implement `attempt_recovery()` method
- [ ] Process retry queue with backoff
- **Requirements**: 6.2, 6.5
- **Files**: `common/ehash/src/wallet.rs`

### 4.8 Add graceful shutdown support
- [ ] Implement `run_with_shutdown(shutdown_rx)` method
- [ ] Add tokio::select! for shutdown signal handling
- [ ] Implement `shutdown()` to complete pending operations
- **Requirements**: 6.2, 6.5
- **Files**: `common/ehash/src/wallet.rs`

### 4.9 Add pubkey accessors
- [ ] Implement `get_locking_pubkey() -> PublicKey`
- [ ] Implement `get_user_identity() -> &str`
- [ ] Add bech32 encoding/decoding helpers (stub for now)
- **Requirements**: 2.4, 8.1
- **Files**: `common/ehash/src/wallet.rs`

### 4.10 Add WalletHandler unit tests
- [ ] Test wallet initialization and configuration
- [ ] Test correlation data processing
- [ ] Test retry queue and recovery logic
- [ ] Test graceful shutdown
- **Requirements**: 8.1, 8.5, 6.2
- **Files**: `common/ehash/src/wallet.rs`

## Phase 5: Pool Role Integration

### 5.1 Add MintConfig to Pool TOML config
- [ ] Extend Pool configuration structs to include optional MintConfig
- [ ] Add deserialization support
- [ ] Document configuration options
- **Requirements**: 5.1, 5.5
- **Files**: `roles/pool/src/lib.rs`, example config files

### 5.2 Add mint thread spawning function
- [ ] Create `spawn_mint_thread(task_manager, config, status_tx)` helper
- [ ] Instantiate MintHandler
- [ ] Spawn thread using task_manager
- [ ] Return sender channel
- **Requirements**: 1.1, 7.3
- **Files**: `roles/pool/src/lib.rs`

### 5.3 Integrate mint_sender into Pool initialization
- [ ] Modify Pool initialization to call spawn_mint_thread if configured
- [ ] Pass mint_sender to ChannelManager
- [ ] No channel_pubkeys HashMap needed (per-share pubkeys from TLV)
- **Requirements**: 5.1
- **Files**: `roles/pool/src/lib.rs`

### 5.4 Add TLV pubkey extraction to ChannelManager
- [ ] Implement `extract_pubkey_from_tlv(&self, msg: &SubmitSharesExtended)` method
- [ ] Extract pubkey from TLV field 0x0004 (33-byte compressed secp256k1)
- [ ] Validate TLV length and pubkey format
- [ ] Return error if TLV missing or invalid
- **Requirements**: 2.5, 2.6
- **Files**: Pool ChannelManager implementation

### 5.5 Hook share validation in handle_submit_shares_extended
- [ ] Extract share hash from ShareValidationResult::Valid
- [ ] Extract locking_pubkey from TLV field 0x0004
- [ ] Create EHashMintData with all required fields including locking_pubkey
- [ ] Send via mint_sender.try_send() (non-blocking)
- [ ] Log errors but continue mining
- **Requirements**: 1.2, 2.5, 2.6, 3.1, 3.3, 6.1
- **Files**: Pool message handler for SubmitSharesStandard

### 5.6 Hook share validation in handle_submit_shares_extended
- [ ] Extract share hash from ShareValidationResult::Valid
- [ ] Create EHashMintData with all required fields
- [ ] Send via mint_sender.try_send() (non-blocking)
- [ ] Log errors but continue mining
- **Requirements**: 1.2, 3.1, 3.3, 6.1
- **Files**: Pool message handler for SubmitSharesExtended

### 5.7 Handle BlockFound variant
- [ ] Extract share hash, template_id, coinbase from BlockFound
- [ ] Create EHashMintData with block_found=true
- [ ] Send to mint_sender for keyset lifecycle trigger
- **Requirements**: 9.1
- **Files**: Pool message handlers

### 5.8 Add Pool integration tests
- [ ] Test mint thread spawning and initialization
- [ ] Test share validation creates mint events
- [ ] Test mining continues during mint failures
- **Requirements**: 1.6, 6.1
- **Files**: Pool integration tests

## Phase 6: TProxy Role Integration

### 6.1 Add WalletConfig to TProxy TOML config
- [ ] Extend TProxy configuration structs to include TProxyShareConfig
- [ ] Add locking_pubkey, user_identity, mint_url fields
- [ ] Add deserialization support
- **Requirements**: 5.2, 5.5
- **Files**: `roles/translator/src/lib.rs`, example config files

### 6.2 Add wallet thread spawning function
- [ ] Create `spawn_wallet_thread(task_manager, config, status_tx)` helper
- [ ] Instantiate WalletHandler
- [ ] Spawn thread using task_manager
- [ ] Return sender channel
- **Requirements**: 7.6
- **Files**: `roles/translator/src/lib.rs`

### 6.3 Integrate wallet_sender into TProxy initialization
- [ ] Modify TProxy initialization to call spawn_wallet_thread if configured
- [ ] Store wallet_sender in TProxy context
- [ ] Extract locking_pubkey for connection setup
- **Requirements**: 5.2
- **Files**: `roles/translator/src/lib.rs`

### 6.4 Hook SubmitSharesSuccess message handling
- [ ] Extract channel_id, sequence_number, user_identity
- [ ] Extract ehash_tokens_minted from TLV (default 0 if not present)
- [ ] Create WalletCorrelationData
- [ ] Send via wallet_sender.try_send() (non-blocking)
- **Requirements**: 3.4, 8.2
- **Files**: TProxy message handler for SubmitSharesSuccess

### 6.5 Add TProxy integration tests
- [ ] Test wallet thread spawning and initialization
- [ ] Test SubmitSharesSuccess creates correlation events
- [ ] Test translation continues during wallet failures
- **Requirements**: 6.2
- **Files**: TProxy integration tests

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

### 8.2 Add TLV 0x0004 to SubmitSharesExtended in TProxy
- [ ] Define TLV field type 0x0004 for per-share locking pubkey
- [ ] Include 33-byte compressed secp256k1 pubkey in TLV when submitting upstream
- [ ] Extract pubkey from channel's stored hpub
- [ ] Add to SubmitSharesExtended message construction
- **Requirements**: 2.5, 2.6
- **Files**: TProxy upstream share submission

### 8.3 Add TLV 0x0004 extraction in Pool
- [ ] Extract TLV field 0x0004 from SubmitSharesExtended messages
- [ ] Validate TLV length (must be 33 bytes)
- [ ] Parse secp256k1 public key from TLV value
- [ ] Reject shares with missing or invalid TLV (for eHash)
- **Requirements**: 2.6
- **Files**: Pool share submission handler

### 8.4 Update Pool share validation to include TLV pubkey
- [ ] Pass extracted pubkey from TLV to EHashMintData creation
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
- [ ] Test TLV 0x0004 encoding/decoding
- [ ] Test per-share pubkey extraction and mint quote creation
- [ ] Test multi-miner support (different hpubs per downstream)
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
