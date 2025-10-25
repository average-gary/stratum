//! Error types for eHash operations
//!
//! This module defines error types for:
//! - `MintError` - Errors that occur during mint operations
//! - `WalletError` - Errors that occur during wallet operations

use thiserror::Error;

/// Errors that can occur during Mint handler operations
#[derive(Debug, Error)]
pub enum MintError {
    /// CDK-related errors
    #[error("CDK error: {0}")]
    CdkError(#[from] cdk::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Channel communication errors
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Payment evaluation errors
    #[error("Payment evaluation error: {0}")]
    PaymentEvaluationError(String),

    /// Database errors
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Initialization errors
    #[error("Initialization error: {0}")]
    InitializationError(String),

    /// Keyset lifecycle errors
    #[error("Keyset lifecycle error: {0}")]
    KeysetLifecycleError(String),

    /// Token minting errors
    #[error("Token minting error: {0}")]
    TokenMintingError(String),

    /// Invalid pubkey errors
    #[error("Invalid pubkey: {0}")]
    InvalidPubkey(String),

    /// Retry queue full
    #[error("Retry queue full: {0}")]
    RetryQueueFull(String),

    /// Generic errors
    #[error("Mint error: {0}")]
    Other(String),
}

/// Errors that can occur during Wallet handler operations
#[derive(Debug, Error)]
pub enum WalletError {
    /// CDK-related errors
    #[error("CDK error: {0}")]
    CdkError(#[from] cdk::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Channel communication errors
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Redemption errors
    #[error("Redemption error: {0}")]
    RedemptionError(String),

    /// Network errors
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Database errors
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Initialization errors
    #[error("Initialization error: {0}")]
    InitializationError(String),

    /// P2PK token query errors
    #[error("P2PK token query error: {0}")]
    P2pkQueryError(String),

    /// Invalid pubkey errors
    #[error("Invalid pubkey: {0}")]
    InvalidPubkey(String),

    /// Retry queue full
    #[error("Retry queue full: {0}")]
    RetryQueueFull(String),

    /// Generic errors
    #[error("Wallet error: {0}")]
    Other(String),
}

// Conversion helpers for common error scenarios

impl From<async_channel::SendError<crate::types::EHashMintData>> for MintError {
    fn from(err: async_channel::SendError<crate::types::EHashMintData>) -> Self {
        MintError::ChannelError(format!("Failed to send mint data: {}", err))
    }
}

impl<T> From<async_channel::TrySendError<T>> for MintError {
    fn from(err: async_channel::TrySendError<T>) -> Self {
        MintError::ChannelError(format!("Failed to try_send: {}", err))
    }
}

impl From<async_channel::SendError<crate::types::WalletCorrelationData>> for WalletError {
    fn from(err: async_channel::SendError<crate::types::WalletCorrelationData>) -> Self {
        WalletError::ChannelError(format!("Failed to send correlation data: {}", err))
    }
}

impl<T> From<async_channel::TrySendError<T>> for WalletError {
    fn from(err: async_channel::TrySendError<T>) -> Self {
        WalletError::ChannelError(format!("Failed to try_send: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mint_error_display() {
        let err = MintError::ConfigError("Invalid config".to_string());
        assert_eq!(err.to_string(), "Configuration error: Invalid config");
    }

    #[test]
    fn test_wallet_error_display() {
        let err = WalletError::NetworkError("Connection failed".to_string());
        assert_eq!(err.to_string(), "Network error: Connection failed");
    }

    #[test]
    fn test_mint_error_types() {
        let _config_err = MintError::ConfigError("test".to_string());
        let _channel_err = MintError::ChannelError("test".to_string());
        let _payment_err = MintError::PaymentEvaluationError("test".to_string());
        let _db_err = MintError::DatabaseError("test".to_string());
        let _init_err = MintError::InitializationError("test".to_string());
        let _keyset_err = MintError::KeysetLifecycleError("test".to_string());
        let _token_err = MintError::TokenMintingError("test".to_string());
        let _pubkey_err = MintError::InvalidPubkey("test".to_string());
        let _queue_err = MintError::RetryQueueFull("test".to_string());
        let _other_err = MintError::Other("test".to_string());
    }

    #[test]
    fn test_wallet_error_types() {
        let _config_err = WalletError::ConfigError("test".to_string());
        let _channel_err = WalletError::ChannelError("test".to_string());
        let _redemption_err = WalletError::RedemptionError("test".to_string());
        let _network_err = WalletError::NetworkError("test".to_string());
        let _db_err = WalletError::DatabaseError("test".to_string());
        let _init_err = WalletError::InitializationError("test".to_string());
        let _p2pk_err = WalletError::P2pkQueryError("test".to_string());
        let _pubkey_err = WalletError::InvalidPubkey("test".to_string());
        let _queue_err = WalletError::RetryQueueFull("test".to_string());
        let _other_err = WalletError::Other("test".to_string());
    }
}
