# Requirements Document - eHash HTTP API

## Introduction

This specification defines the HTTP API requirements for Phase 9 of the eHash (ecash hashrate) token system. The system currently has a complete internal CDK Mint implementation but lacks HTTP endpoints for external wallet access. 

The core requirement is to extend NUT-20 (Signature on Mint Quote) to support querying quotes by public key with signature authentication, preventing unauthorized access to other users' quotes. This extension enables wallets to discover their PAID quotes using the same public key used for NUT-20 minting authentication.

## Glossary

- **CDK**: Cashu Development Kit - the Rust library used for ecash operations
- **cdk-axum**: HTTP server implementation for CDK using the Axum web framework
- **NUT-20**: Cashu specification for signature-based authentication on mint quotes using secp256k1 public keys
- **NUT-20 Extension**: Proposed extension to NUT-20 for querying quotes by public key with signature authentication
- **PAID Quote**: A mint quote in PAID state, ready for token redemption
- **Locking Pubkey**: The secp256k1 public key that controls access to P2PK-locked tokens (same as NUT-20 pubkey)
- **hpub**: Bech32-encoded public key format with 'hpub' prefix, the preferred encoding for eHash public keys
- **Quote Discovery**: The process of finding quote IDs associated with a public key

## Requirements

### Requirement 1: NUT-20 Extension - Authenticated Quote Discovery by Public Key

**User Story:** As an external wallet with a private key, I want to securely discover all PAID quotes associated with my NUT-20 public key, so that I can redeem eHash tokens without revealing information about other users' quotes.

#### Acceptance Criteria

1. **THE** HTTP API **SHALL** extend NUT-20 to support querying quotes by public key with signature authentication
2. **WHEN** a wallet requests quotes by public key, **THE** HTTP API **SHALL** require a valid BIP340 Schnorr signature proving ownership of the private key (same signature scheme as NUT-20)
3. **THE** HTTP API **SHALL** use the message format "get_quotes:{pubkey_hex}" for signature verification
4. **IF** the signature is missing or invalid, **THEN** **THE** HTTP API **SHALL** return HTTP 401 Unauthorized
5. **WHEN** the signature is valid, **THE** HTTP API **SHALL** return only PAID quotes where the NUT-20 pubkey matches the authenticated public key
6. **THE** HTTP API **SHALL** support hpub format (bech32-encoded with 'hpub' prefix) as the preferred encoding for public keys in requests
7. **THE** HTTP API **SHALL** prevent unauthorized enumeration of other users' quotes

### Requirement 2: HTTP Server Integration

**User Story:** As a Pool or JDC operator, I want to enable HTTP API access to my mint using cdk-axum, so that external wallets can access tokens while mining continues normally.

#### Acceptance Criteria

1. **THE** Pool role **SHALL** optionally spawn a cdk-axum HTTP server sharing the CDK Mint instance
2. **THE** JDC role in Mint mode **SHALL** optionally spawn a cdk-axum HTTP server for external wallet access
3. **THE** HTTP server **SHALL** use a configurable port separate from existing Stratum v2 ports
4. **THE** system **SHALL** ensure that internal mint operations and HTTP operations do not conflict
5. **IF** the HTTP server fails, **THEN** mining operations **SHALL** continue without interruption