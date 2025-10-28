//! # eHash - Hashpool Integration for Stratum V2
//!
//! This crate provides Cashu ecash minting and wallet functionality for the Stratum V2
//! reference implementation, implementing the hashpool.dev protocol.
//!
//! ## Architecture
//!
//! The crate is organized into the following modules:
//!
//! - `types` - Core data structures for eHash operations
//! - `config` - Configuration structures for mint and wallet
//! - `error` - Error types for eHash operations
//! - `mint` - Mint handler implementation (will be added in Phase 3)
//! - `wallet` - Wallet handler implementation (will be added in Phase 4)
//!
//! ## Usage
//!
//! This crate is intended to be used by:
//! - Pool role: Uses MintHandler to mint eHash tokens for valid shares
//! - JDC role: Can operate in either Mint or Wallet mode
//! - TProxy role: Uses WalletHandler to track share correlation

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// Re-export CDK types for convenience
pub use cdk;
pub use cdk_common;

/// Re-exported CDK types commonly used in eHash operations
pub mod cdk_types {
    pub use cdk::amount::Amount;
    pub use cdk::mint_url::MintUrl;
    pub use cdk::nuts::CurrencyUnit;
    pub use cdk::Mint;
    pub use cdk::Wallet;
}

// Core eHash calculation module
pub mod work;

// Re-export core functions
pub use work::{calculate_difficulty, calculate_ehash_amount};

// Module declarations - will be populated in subsequent phases
pub mod types; // Phase 2: Core Data Structures
pub mod config; // Phase 2: Core Data Structures
pub mod error; // Phase 2: Core Data Structures
pub mod mint; // Phase 3: MintHandler Implementation
pub mod hpub; // Phase 3: hpub encoding/decoding utilities
              // pub mod wallet;    // Phase 4: WalletHandler Implementation

// Re-export commonly used types
pub use types::{EHashMintData, WalletCorrelationData};
pub use config::{MintConfig, WalletConfig, JdcEHashConfig, JdcEHashMode};
pub use error::{MintError, WalletError};
pub use mint::MintHandler;
pub use hpub::{encode_hpub, parse_hpub};
