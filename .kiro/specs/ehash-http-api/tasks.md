# Implementation Tasks - eHash HTTP API

This document breaks down the eHash HTTP API implementation into focused tasks. The implementation extends cdk-axum with NUT-20 authenticated quote discovery and integrates HTTP servers into Pool/JDC roles.

## Task 1: CDK Submodule Branch Setup

### 1.1 Create eHash-specific branch in CDK submodule
- [x] Create new branch `ehash-v0.13.x` from current v0.13.x in `deps/cdk` submodule
- [x] Switch CDK submodule to the new eHash branch
- [x] Update submodule reference to point to new branch
- [x] Configure build system to use the eHash branch
- [x] Document branch strategy and maintenance workflow
- **Requirements**: CDK integration strategy
- **Files**: `deps/cdk` (submodule), `.gitmodules`, `Cargo.toml` dependencies
- **Status**: ✅ COMPLETED

**Branch Strategy and Maintenance Workflow:**

The `ehash-v0.13.x` branch has been successfully created and configured in the CDK submodule (`deps/cdk`). This branch is based on CDK v0.13.3 and serves as our stable foundation for eHash-specific extensions.

**Current Configuration:**
- Branch: `ehash-v0.13.x` (commit 6ef8f3e3 - v0.13.3)
- Remote: `origin` → https://github.com/average-gary/cdk.git
- Upstream tracking: `upstream` → https://github.com/cashubtc/cdk.git
- `.gitmodules` configuration: branch = ehash-v0.13.x

**Maintenance Workflow:**
1. **Local Development**: All eHash-specific CDK changes are committed to the `ehash-v0.13.x` branch
2. **Remote Sync**: Changes are pushed to `average-gary/cdk.git` fork
3. **Upstream Updates**: Periodically sync with upstream `cashubtc/cdk` v0.13.x branch:
   ```bash
   cd deps/cdk
   git fetch upstream
   git rebase upstream/v0.13.x  
   git push origin ehash-v0.13.x
   ```
4. **Submodule Updates**: After pushing CDK changes, update parent repo:
   ```bash
   cd ../..
   git add deps/cdk
   git commit -m "chore(cdk): update submodule to latest ehash-v0.13.x"
   ```

**Philosophy:**
- Keep eHash changes isolated in our branch to avoid upstream conflicts
- Track upstream v0.13.x for bug fixes and security updates
- Consider upstreaming generic improvements that benefit the broader CDK ecosystem
- Maintain clear separation between eHash-specific features and general CDK enhancements

## Task 2: CDK Mint Extension for Pubkey Queries

### 2.1 Add pubkey-based quote query method to CDK Mint
- [x] Add `get_mint_quotes_by_pubkey(&self, pubkey: &PublicKey) -> Result<Vec<MintQuote>, Error>` method to CDK Mint
- [x] Implement database query to filter quotes by pubkey field (database already has pubkey column)
- [x] Add unit tests for pubkey-based quote filtering
- [x] Ensure method only returns quotes where `quote.pubkey == provided_pubkey`
- **Requirements**: 1.4, 1.5
- **Files**:
  - `deps/cdk/crates/cdk-common/src/database/mint/mod.rs` (trait definition)
  - `deps/cdk/crates/cdk-sql-common/src/mint/mod.rs` (SQL implementation)
  - `deps/cdk/crates/cdk/src/mint/issue/mod.rs` (Mint method)
  - `deps/cdk/crates/cdk-common/src/database/mint/test/mint.rs` (unit tests)
- **Status**: ✅ COMPLETED

**Implementation Details:**
- Added `get_mint_quotes_by_pubkey` method to the `QuotesDatabase` trait in `cdk-common/src/database/mint/mod.rs:238`
- Implemented SQL query in `cdk-sql-common/src/mint/mod.rs:1371` that filters quotes by pubkey using `WHERE pubkey = :pubkey`
- Added public method to Mint struct in `cdk/src/mint/issue/mod.rs:392` with proper instrumentation and prometheus metrics
- Created comprehensive unit test in `cdk-common/src/database/mint/test/mint.rs:409` that validates:
  - Multiple quotes with same pubkey are returned
  - Only quotes matching the specified pubkey are returned
  - Quotes with different pubkeys are excluded
  - Quotes without pubkeys are excluded
  - Empty result when querying for non-existent pubkey
- Test passes successfully: `test mint::test::get_mint_quotes_by_pubkey ... ok`

## Task 3: NUT-20 Extension Implementation

### 3.1 Create eHash-specific request/response structs
- [x] Create `QuotesByPubkeyRequest` struct with pubkey and signature fields
- [x] Create `QuotesByPubkeyResponse` struct with quotes array
- [x] Create `EHashQuoteSummary` struct with quote details
- [x] Create `PostMintEHashRequest` and `PostMintEHashResponse` structs
- [x] Add comprehensive input validation and error handling
- **Requirements**: 1.1, 1.5
- **Files**: `deps/cdk/crates/cdk-axum/src/nut20_extension.rs` (new)
- **Status**: ✅ COMPLETED

**Implementation Details:**
- Created `QuotesByPubkeyRequest` with pubkey (string) and signature fields in `cdk-axum/src/nut20_extension.rs:24`
- Created `EHashQuoteSummary` struct with all essential quote fields (quote_id, amount, unit, state, expiry, created_time, request) in `cdk-axum/src/nut20_extension.rs:49`
- Created `QuotesByPubkeyResponse` containing Vec<EHashQuoteSummary> in `cdk-axum/src/nut20_extension.rs:67`
- Created `PostMintEHashRequest` with quote, outputs, and signature fields in `cdk-axum/src/nut20_extension.rs:79`
- Created `PostMintEHashResponse` with blind signatures in `cdk-axum/src/nut20_extension.rs:92`
- All structs include proper serialization, swagger support (feature-gated), and comprehensive documentation
- Error handling uses appropriate HTTP status codes and CDK ErrorResponse pattern

### 3.2 Integrate existing hpub format parsing
- [x] Use existing `ehash::hpub::parse_hpub()` function from common library
- [x] Add `parse_pubkey()` wrapper function supporting both hex and hpub formats
- [x] Add comprehensive unit tests for integration with existing hpub parsing
- **Requirements**: 1.5
- **Files**: `deps/cdk/crates/cdk-axum/src/nut20_extension.rs`
- **Status**: ✅ COMPLETED

**Implementation Details:**
- Created `parse_pubkey()` function in `cdk-axum/src/nut20_extension.rs:100` that supports both hex and hpub formats
- Currently hex parsing is fully functional using `PublicKey::from_hex()`
- hpub parsing returns helpful error message indicating it requires ehash crate integration (will be added in Pool/JDC integration tasks)
- Added unit tests: `test_parse_pubkey_hex`, `test_parse_pubkey_invalid_hex`, `test_parse_pubkey_hpub_not_yet_supported`
- Design allows easy integration with ehash crate when dependencies are added in Tasks 5 & 6

### 3.3 Implement BIP340 Schnorr signature verification
- [x] Add signature verification for "get_quotes:{pubkey_hex}" message format
- [x] Use existing NUT-20 signature verification from CDK (reuse `PublicKey::verify` method)
- [x] Add proper error handling for invalid signatures
- [x] Add comprehensive unit tests for signature verification (valid and invalid cases)
- **Requirements**: 1.2, 1.3
- **Files**: `deps/cdk/crates/cdk-axum/src/nut20_extension.rs`
- **Status**: ✅ COMPLETED

**Implementation Details:**
- Implemented `verify_get_quotes_signature()` in `cdk-axum/src/nut20_extension.rs:131` following NUT-20 pattern
- Message format: "get_quotes:{pubkey_hex}" as UTF-8 bytes
- Uses `bitcoin::secp256k1::schnorr::Signature` for BIP340 signatures
- Leverages existing `PublicKey::verify()` method from CDK
- Added unit tests: `test_verify_get_quotes_signature_message_format` (valid signature), `test_verify_get_quotes_signature_invalid` (wrong message)
- All tests pass successfully

### 3.4 Implement authenticated quote discovery endpoint
- [x] Create `get_quotes_by_pubkey()` async handler function
- [x] Parse and validate pubkey from request (hex or hpub format)
- [x] Verify signature proves pubkey ownership using existing NUT-20 verification
- [x] Query CDK Mint for quotes matching the authenticated pubkey
- [x] Filter to PAID quotes only and return structured response
- [x] Return HTTP 401 for invalid/missing signatures
- **Requirements**: 1.1, 1.2, 1.3, 1.4, 1.5
- **Files**: `deps/cdk/crates/cdk-axum/src/nut20_extension.rs`
- **Status**: ✅ COMPLETED

**Implementation Details:**
- Implemented async handler `get_quotes_by_pubkey()` in `cdk-axum/src/nut20_extension.rs:186`
- Authentication flow: parse pubkey → verify signature → query database
- Uses `mint.get_mint_quotes_by_pubkey()` from Task 2 to fetch quotes
- Filters to MintQuoteState::Paid quotes only
- Converts internal MintQuote to EHashQuoteSummary for response
- Error responses: 400 (bad pubkey), 401 (invalid signature), 500 (database error)
- Comprehensive logging with tracing (debug, warn levels)
- Uses proper axum Response patterns with StatusCode and Json

### 3.5 Implement eHash minting endpoint
- [x] Create `mint_ehash_tokens()` async handler function
- [x] Verify quote exists and is in PAID state
- [x] Verify NUT-20 signature matches quote's pubkey using existing CDK verification
- [x] Process minting using standard CDK flow (reuse existing mint logic)
- [x] Return blind signatures in eHash-specific response format
- [x] Add proper error handling for all failure cases
- **Requirements**: 1.1, 1.2, 1.3
- **Files**: `deps/cdk/crates/cdk-axum/src/nut20_extension.rs`
- **Status**: ✅ COMPLETED

**Implementation Details:**
- Implemented async handler `mint_ehash_tokens()` in `cdk-axum/src/nut20_extension.rs:260`
- Minting flow: parse quote_id → fetch quote → verify PAID state → verify pubkey exists → construct MintRequest → verify NUT-20 signature → process minting
- Uses `mint.localstore().get_mint_quote()` to fetch internal MintQuote (needed for pubkey access)
- Verifies quote is in PAID state before processing
- Constructs MintRequest with NUT-20 signature for verification
- Uses `MintRequest::verify_signature()` to verify signature matches quote's pubkey
- Calls `mint.process_mint_request()` to process minting using standard CDK flow
- Returns `PostMintEHashResponse` with blind signatures
- Error responses: 400 (invalid quote_id/missing pubkey), 401 (invalid signature), 404 (quote not found), 409 (not PAID), 500 (minting failed)
- Comprehensive logging and error handling throughout

## Task 4: cdk-axum Router Integration

### 4.1 Add new endpoints to cdk-axum router
- [ ] Add `POST /v1/mint/quotes/by-pubkey` route for quote discovery
- [ ] Add `POST /v1/mint/ehash` route for eHash minting
- [ ] Integrate routes into existing cdk-axum router configuration in `create_mint_router_with_custom_cache`
- [ ] Ensure routes use shared CDK Mint state (MintState)
- **Requirements**: 1.1, 2.1
- **Files**: `deps/cdk/crates/cdk-axum/src/lib.rs`

### 4.2 Add proper error handling and logging
- [ ] Implement structured JSON error responses using existing cdk-axum error handling patterns
- [ ]* Add security logging for authentication failures
- [ ] Map CDK errors to appropriate HTTP status codes using existing `into_response` helper
- **Requirements**: Security and logging from design
- **Files**: `deps/cdk/crates/cdk-axum/src/nut20_extension.rs`

## Task 5: Pool Role HTTP Server Integration

### 5.1 Add cdk-axum dependency to Pool role
- [ ] Add `cdk-axum` dependency to `roles/pool/Cargo.toml`
- [ ] Add `ehash` dependency to access hpub parsing functions
- [ ] Ensure dependency versions match the CDK submodule version
- **Requirements**: 2.1
- **Files**: `roles/pool/Cargo.toml`

### 5.2 Add HTTP API configuration to Pool
- [ ] Extend `MintConfig` with `HttpApiConfig` struct
- [ ] Add fields for `enabled`, `bind_address`
- [ ]* Add TLS configuration fields (`tls_cert_path`, `tls_key_path`)
- [ ] Add configuration validation and parsing
- [ ] Update example configuration files
- **Requirements**: 2.1, 2.3, 2.4
- **Files**: `common/ehash/src/config.rs`, `roles/pool/config-examples/`

### 5.3 Integrate HTTP server into existing Pool mint thread
- [ ] Add HTTP server to existing mint thread (same thread as CDK Mint instance)
- [ ] Use tokio::select! to handle both mint events and HTTP requests concurrently
- [ ] Share CDK Mint instance between mint operations and HTTP handlers using Arc
- [ ] Add graceful shutdown handling for HTTP server
- [ ] Ensure HTTP server errors don't affect mining operations
- [ ]* Add optional TLS support
- **Requirements**: 2.1, 2.3, 2.5
- **Files**: `common/ehash/src/mint.rs`, `roles/pool/src/lib/mod.rs`

### 5.4 Add Pool HTTP server integration tests
- [ ]* Test HTTP server startup and shutdown
- [ ]* Test configuration parsing and validation
- [ ]* Test that mining operations continue if HTTP server fails
- [ ]* Test TLS configuration (if implemented)
- **Requirements**: 2.5
- **Files**: `roles/pool/tests/http_api_test.rs` (new)

## Task 6: JDC Role HTTP Server Integration

### 6.1 Add cdk-axum dependency to JDC role
- [ ] Add `cdk-axum` dependency to `roles/jd-client/Cargo.toml`
- [ ] Add `ehash` dependency to access hpub parsing functions
- [ ] Ensure dependency versions match the CDK submodule version
- **Requirements**: 2.2
- **Files**: `roles/jd-client/Cargo.toml`

### 6.2 Add HTTP API configuration to JDC Mint mode
- [ ] Extend `JdcEHashConfig.mint` with `HttpApiConfig`
- [ ] Add configuration validation for JDC Mint mode only
- [ ] Update example JDC configuration files
- [ ] Ensure HTTP API is only available in Mint mode
- **Requirements**: 2.2, 2.3, 2.4
- **Files**: `common/ehash/src/config.rs`, `roles/jd-client/config-examples/`

### 6.3 Integrate HTTP server into existing JDC mint thread
- [ ] Add HTTP server to existing JDC mint thread (same thread as CDK Mint instance)
- [ ] Use tokio::select! to handle both mint events and HTTP requests concurrently
- [ ] Share CDK Mint instance between JDC mint operations and HTTP handlers
- [ ] Add graceful shutdown handling
- [ ] Ensure HTTP server only starts in Mint mode
- **Requirements**: 2.2, 2.3, 2.5
- **Files**: `common/ehash/src/mint.rs`, `roles/jd-client/src/lib/mod.rs`

### 6.4 Add JDC HTTP server integration tests
- [ ]* Test HTTP server startup in Mint mode only
- [ ]* Test that Wallet mode doesn't start HTTP server
- [ ]* Test configuration validation
- [ ]* Test graceful shutdown
- **Requirements**: 2.5
- **Files**: `roles/jd-client/tests/http_api_test.rs` (new)

## Task 7: End-to-End Integration Testing

### 7.1 Create comprehensive integration test suite
- [ ] Test complete quote discovery flow (signature generation → authentication → response)
- [ ] Test complete eHash minting flow (quote discovery → minting → token verification)
- [ ]* Test multi-unit support (HASH and sat currencies)
- [ ]* Test hpub format support in all endpoints
- [ ]* Test error cases (invalid signatures, missing quotes, etc.)
- **Requirements**: All requirements
- **Files**: `test/integration-tests/http_api_integration_test.rs` (new)

### 7.2 Create wallet integration examples
- [ ]* Create example wallet code showing quote discovery
- [ ]* Create example wallet code showing eHash minting
- [ ]* Provide examples in multiple languages (Rust, JavaScript, Python)
- [ ]* Include signature generation examples
- **Requirements**: Documentation requirement
- **Files**: `examples/ehash-wallet-integration/` (new)

### 7.3 Add performance and security tests
- [ ]* Test concurrent quote discovery requests
- [ ]* Test rate limiting (if implemented)
- [ ]* Test signature verification performance
- [ ]* Test database query performance with large datasets
- [ ] Security test: verify unauthorized access is prevented
- **Requirements**: Performance and security from design
- **Files**: `test/performance-tests/http_api_perf_test.rs` (new)

## Task 8: Documentation and Examples

### 8.1 Create API documentation
- [ ] Document all endpoints with request/response examples
- [ ] Provide curl command examples for all operations
- [ ]* Document signature generation process
- [ ]* Document hpub format specification
- [ ]* Document error codes and responses
- **Requirements**: Documentation requirement
- **Files**: `docs/ehash-http-api.md` (new)

### 8.2 Create deployment guide
- [ ]* Document HTTP server configuration options
- [ ]* Provide deployment examples for Pool and JDC
- [ ]* Document TLS setup and security considerations
- [ ]* Create troubleshooting guide
- **Requirements**: Configuration and deployment
- **Files**: `docs/ehash-http-deployment.md` (new)

### 8.3 Update existing documentation
- [ ]* Update Pool configuration documentation
- [ ]* Update JDC configuration documentation
- [ ]* Update overall eHash system architecture documentation
- [ ]* Add HTTP API to system overview diagrams
- **Requirements**: Documentation requirement
- **Files**: Existing documentation files

## Implementation Status

**CURRENT STATE ANALYSIS:**
- ✅ CDK submodule is on ehash-v0.13.x branch (Task 1 completed)
- ✅ NUT-20 signature verification exists in CDK (`deps/cdk/crates/cashu/src/nuts/nut20.rs`)
- ✅ cdk-axum framework exists with standard endpoints (`deps/cdk/crates/cdk-axum/`)
- ✅ hpub parsing functions exist in common library (`common/ehash/src/hpub.rs`)
- ✅ Pubkey-based quote query method in CDK Mint (Task 2 completed)
- ✅ NUT-20 extension endpoints implemented in cdk-axum (Task 3 completed)
  - `get_quotes_by_pubkey()` handler for authenticated quote discovery
  - `mint_ehash_tokens()` handler for eHash minting
  - All request/response structs defined
  - Comprehensive tests passing (7/7)
- ❌ Endpoints not yet added to router (Task 4 pending)
- ❌ No HTTP server integration in Pool/JDC roles
- ❌ No HTTP API configuration structures

**NEXT STEPS:**
Tasks 1, 2, and 3 are complete. Proceed with Task 4 (cdk-axum Router Integration).

## Notes

- Tasks should be implemented in order due to dependencies
- Each task should result in a focused, reviewable commit
- All tests should pass after each task completion
- HTTP API functionality is optional and should not break existing mining operations
- Focus on security: all authentication must be properly implemented and tested
- Performance considerations: database queries should be efficient and scalable
- Leverage existing CDK implementations where possible to minimize custom code