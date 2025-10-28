# Requirements Document

## Introduction

This feature implements hashpool.dev, a system that integrates Cashu ecash minting and wallet functionality into the Stratum v2 reference implementation. The implementation involves refactoring the existing Pool role's share persistence into a Mint daemon (mool), and creating a new Wallet component in the TProxy role (walloxy). The system integrates with the CDK (Cashu Development Kit) from https://github.com/cashubtc/cdk to enable miners to earn ecash tokens (eHash) based on their mining shares, supporting both traditional sats/BTC units and custom "hash" units.

## Requirements

### Requirement 1: Mint Implementation

**User Story:** As a mining pool operator, I want to integrate eHash minting into the share validation flow using a separate mint thread so that I can issue ecash tokens to miners based on their submitted shares using the share hash returned from validation.

#### Acceptance Criteria

1. WHEN the Pool role starts THEN the system SHALL initialize a Mint daemon thread using the CDK crate from https://github.com/cashubtc/cdk
2. WHEN share validation returns ShareValidationResult::Valid(share_hash) THEN the system SHALL extract share hash and channel data, then send EHashMintData via async channel to the Mint thread
3. WHEN the Mint thread receives EHashMintData THEN it SHALL calculate eHash amount using the share hash leading zeros
4. WHEN EHashMintData is created THEN the system SHALL include all required data (share_hash, channel_id, user_identity, target, sequence_number, locking_pubkey) from the validation context
5. WHEN minting operations occur THEN the system SHALL rely on CDK's native database and accounting capabilities for audit trails and transaction records
6. WHEN the Mint operates THEN it SHALL support HASH unit for eHash token issuance and create PAID sats quotes using eHash tokens as payment
7. WHEN the Mint creates PAID quotes THEN it SHALL always include the per-share locking_pubkey for NUT-20 P2PK authentication (NOT optional)

### Requirement 2: Per-Share NUT-20 P2PK Locking Implementation

**User Story:** As a miner using a translator proxy, I want to specify my locking pubkey in my user identity so that each share I submit can be locked to my specific public key, enabling secure external wallet redemption with NUT-20 authentication.

#### Acceptance Criteria

1. WHEN a downstream miner connects to TProxy THEN the miner SHALL specify their locking pubkey in user_identity field using hpub format (bech32-encoded with 'hpub' HRP)
2. WHEN TProxy receives OpenExtendedMiningChannel with user_identity THEN it SHALL parse and validate the hpub format
3. WHEN hpub validation fails THEN TProxy SHALL disconnect the miner and SHALL NOT provide jobs
4. WHEN hpub validation succeeds THEN TProxy SHALL extract the secp256k1 public key and store it for the channel
5. WHEN TProxy submits shares upstream via SubmitSharesExtended THEN it SHALL include the locking pubkey in the direct `locking_pubkey: PubKey33` field (33-byte compressed secp256k1 pubkey)
6. WHEN the Pool receives SubmitSharesExtended THEN it SHALL extract the locking pubkey from the `locking_pubkey` field and include it in EHashMintData
7. WHEN multiple downstream miners connect THEN each miner SHALL have their own unique locking pubkey enabling multi-miner proxy support
8. WHEN miners submit multiple shares THEN each share MAY have a different locking pubkey enabling per-share key rotation

### Requirement 3: Share Hash Integration with Event-Driven Architecture

**User Story:** As a system architect, I want to leverage the refactored share validation system that now returns share hashes directly from validation results and emit events to dedicated processing threads.

#### Acceptance Criteria

1. WHEN share validation occurs in Pool/JDC THEN the system SHALL extract share hash from ShareValidationResult::Valid(share_hash) or ShareValidationResult::BlockFound(share_hash, template_id, coinbase)
2. WHEN share hash is extracted THEN the system SHALL create EHashMintData containing share hash and all channel context data
3. WHEN EHashMintData is created THEN the system SHALL send it via async channel to the dedicated Mint thread for processing
4. WHEN the TProxy receives SubmitSharesSuccess THEN it SHALL create correlation events and send them via async channel to the dedicated Wallet thread
5. WHEN share processing occurs THEN the system SHALL rely on existing Stratum v2 share validation and deduplication logic while maintaining thread separation

### Requirement 4: CDK Integration

**User Story:** As a developer, I want to integrate the CDK crate from the cashubtc/cdk repository to provide robust Cashu ecash functionality.

#### Acceptance Criteria

1. WHEN the system initializes THEN it SHALL use the CDK crate from https://github.com/cashubtc/cdk
2. WHEN CDK operations are performed THEN the system SHALL handle all CDK-specific error conditions using the existing Status system from the Stratum v2 roles
3. WHEN mint operations occur THEN the system SHALL comply with Cashu protocol specifications
4. WHEN wallet operations occur THEN the system SHALL maintain compatibility with standard Cashu wallets

### Requirement 5: Configuration and Deployment

**User Story:** As a system administrator, I want configurable deployment options that integrate CDK mint and wallet configurations with existing Stratum v2 TOML configuration structures.

#### Acceptance Criteria

1. WHEN deploying the Pool role THEN the system SHALL merge CDK mint configuration options with existing Stratum v2 TOML configuration
2. WHEN deploying the TProxy role THEN the system SHALL merge CDK wallet configuration options with existing Stratum v2 TOML configuration
3. WHEN deploying the JDC role as a mint THEN the system SHALL merge CDK mint configuration options with existing Stratum v2 TOML configuration
4. WHEN deploying the JDC role as a wallet THEN the system SHALL merge CDK wallet configuration options with existing Stratum v2 TOML configuration
5. WHEN configuring ecash operations THEN the system SHALL maintain compatibility with existing Stratum v2 configuration patterns
6. WHEN configuration changes occur THEN the system SHALL validate both Stratum v2 and CDK settings and provide clear error messages

### Requirement 6: Error Handling and Reliability

**User Story:** As a mining operation manager, I want robust error handling to ensure continuous mining operations even when ecash components experience issues.

#### Acceptance Criteria

1. WHEN Mint operations fail THEN the Pool role SHALL continue normal mining operations and attempt a graceful recovery
2. WHEN Wallet operations fail THEN the TProxy role SHALL continue normal translation operations
3. WHEN network connectivity to the mint is lost THEN the system SHALL queue operations for retry
4. WHEN CDK operations encounter errors THEN the system SHALL log detailed error information
5. WHEN recovery from failures occurs THEN the system SHALL resume operations without data loss

### Requirement 7: Unified Mint and EHash Handler Implementation

**User Story:** As a developer, I want shared MintHandler and EhashHandler implementations that can be used by Pool, JDC, and TProxy roles based on configuration.

#### Acceptance Criteria

1. WHEN implementing Mint functionality THEN the system SHALL create a single MintHandler that runs in a dedicated thread and processes EHashMintData events
2. WHEN implementing eHash accounting THEN the system SHALL create a single EhashHandler that runs in a dedicated thread and tracks eHash accounting statistics
3. WHEN the Pool role is configured THEN it SHALL use MintHandler to process share validation results
4. WHEN the JDC role is configured as a mint THEN it SHALL use MintHandler to process share validation results
5. WHEN the JDC role is configured in ehash mode THEN it SHALL use EhashHandler to track eHash accounting for downstream connections
6. WHEN the TProxy role is configured THEN it SHALL use EhashHandler to track eHash accounting for downstream miners
7. WHEN EhashHandler tracks statistics THEN it SHALL NOT perform token redemption (external wallets handle redemption)
8. WHEN handler implementations are updated THEN all roles using them SHALL automatically benefit from the changes

### Requirement 8: NUT-04 and NUT-20 Compliant External Wallet Integration

**User Story:** As a mining operation manager, I want external Cashu wallets to securely redeem eHash tokens using NUT-04 random quote IDs and NUT-20 authentication to prevent front-running attacks and unauthorized token minting.

#### Acceptance Criteria

1. WHEN the Mint creates PAID quotes THEN it SHALL generate random UUID v4 quote IDs that are NOT derivable from payment requests (NUT-04 compliance)
2. WHEN the Mint creates PAID quotes THEN it SHALL attach the per-share locking pubkey from EHashMintData.locking_pubkey to the quote (NUT-20 authorization)
3. WHEN external wallets query the mint THEN they SHALL be able to query quotes by their locking pubkey via authenticated API
4. WHEN external wallets receive quote information THEN they SHALL receive the secret quote ID from the mint
5. WHEN external wallets redeem tokens THEN they SHALL create blinded messages (MAY be normal secrets or P2PK-locked, wallet's choice)
6. WHEN external wallets submit MintRequest THEN they SHALL sign the request with their private key (NUT-20 authentication)
7. WHEN the Mint receives MintRequest THEN it SHALL verify the signature matches the quote's locking pubkey before signing blinded messages
8. WHEN the Mint verifies the signature THEN it SHALL sign the blinded messages and return blind signatures
9. WHEN wallets receive blind signatures THEN they SHALL unblind them to obtain eHash token proofs (normal bearer tokens or P2PK-locked based on wallet choice)
10. WHEN all eHash operations occur THEN the system SHALL maintain audit trails including share hashes as payment proof

### Requirement 9: eHash Keyset Lifecycle Management

**User Story:** As a mining pool operator, I want automated keyset lifecycle management that transitions eHash keysets through different states based on mining events.

#### Acceptance Criteria

1. WHEN the Mint receives a ShareAccountingEvent with block_found=true THEN it SHALL query Bitcoin Core via RPC for block reward details and transition the active keyset to QUANTIFYING state
   - **Implementation Note**: Use Bitcoin Core RPC `getblock` method with verbosity=2 to fetch full block data including coinbase transaction and fees
   - **Configuration**: Mint SHALL support configurable Bitcoin RPC endpoint, authentication credentials, and timeout settings
   - **Block Hash**: Use EHashMintData.share_hash directly as block hash (when block_found=true, share_hash IS the block hash)
   - **Block Data**: Call getblock(share_hash, 2), parse coinbase transaction output value, and calculate total transaction fees
2. WHEN the Mint detects a BOLT12 payment via LDK integration THEN it SHALL verify the payment is for mining rewards and transition the active keyset to QUANTIFYING state
3. WHEN a keyset transitions to QUANTIFYING state THEN the Mint SHALL create a new ACTIVE keyset first to ensure continuous eHash minting capability
4. WHEN quantification is complete THEN the Mint SHALL automatically transition the keyset to PAYOUT state with the calculated conversion rate
5. WHEN a keyset is in PAYOUT state THEN external wallets SHALL be able to swap eHash tokens for sats at the determined rate
6. WHEN all eHash tokens are redeemed or payout period expires THEN the Mint SHALL transition the keyset to EXPIRED state

### Requirement 10: Authenticated Quote Discovery by Pubkey

**User Story:** As a wallet operator, I want to query the mint for outstanding quotes associated with my pubkey using authenticated requests to prevent unauthorized snooping.

#### Acceptance Criteria

1. WHEN the Mint creates a PAID quote THEN it SHALL store a mapping from locking_pubkey to quote_id in CDK's KV store
2. WHEN the Mint stores pubkey-to-quote mappings THEN it SHALL use KV store namespace "ehash_pubkey_quotes" with pubkey hex as secondary namespace
3. WHEN external wallets or TProxy query for quotes by pubkey THEN they SHALL provide a NUT-20 signature proving ownership of the private key
4. WHEN the query request is sent THEN it SHALL include: pubkey, signature, and the signed message
5. WHEN the Mint receives a quote query request THEN it SHALL verify the signature matches the pubkey and signed message
6. WHEN signature verification fails THEN the Mint SHALL reject the request with HTTP 401 Unauthorized
7. WHEN signature verification succeeds THEN the Mint SHALL return all quote IDs associated with that pubkey including basic metadata (quote_id, amount, unit, state, created_time)
8. WHEN quotes are returned THEN wallets MAY fetch full quote details using standard CDK quote endpoints
9. WHEN a quote is fully redeemed (state=ISSUED) THEN the Mint MAY remove the pubkey-to-quote mapping during periodic cleanup

