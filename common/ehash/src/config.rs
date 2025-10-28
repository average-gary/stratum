//! Configuration structures for eHash mint and wallet operations
//!
//! This module defines the configuration types used to initialize:
//! - `MintConfig` - Configuration for the Mint handler (Pool and JDC mint mode)
//! - `WalletConfig` - Configuration for the Wallet handler (TProxy and JDC wallet mode)
//! - `JdcEHashConfig` - Configuration for JDC role eHash mode selection

use cdk::mint_url::MintUrl;
use cdk::nuts::CurrencyUnit;
use serde::{Deserialize, Serialize};

/// Configuration for the Mint handler
///
/// This configuration is used by:
/// - Pool role: To mint eHash tokens for validated shares
/// - JDC role in mint mode: To mint eHash tokens for JDC-validated shares
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MintConfig {
    /// URL where the mint is accessible
    pub mint_url: MintUrl,

    /// Optional mint private key for CDK Mint initialization
    /// If not provided, a new key will be generated
    pub mint_private_key: Option<String>,

    /// Supported currency units for minting
    /// Typically includes "HASH" for eHash tokens and "sat" for sats
    #[serde(default = "default_supported_units")]
    pub supported_units: Vec<CurrencyUnit>,

    /// Optional database URL for CDK persistence
    /// Supports sqlite, postgres, redb backends
    /// If not provided, uses in-memory database (not recommended for production)
    pub database_url: Option<String>,

    /// Minimum leading zero bits required to earn 1 unit of eHash
    /// Default: 32 (hashpool standard)
    #[serde(default = "default_min_leading_zeros")]
    pub min_leading_zeros: u32,

    /// Maximum retry attempts before disabling mint operations
    /// Default: 10
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Backoff multiplier in seconds for retry logic
    /// Default: 2
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: u64,

    /// Enable automatic recovery from failures
    /// Default: true
    #[serde(default = "default_recovery_enabled")]
    pub recovery_enabled: bool,

    /// Optional log level for mint operations
    pub log_level: Option<String>,

    /// Placeholder locking pubkey for Phase 5 (basic integration)
    /// This will be replaced with per-share pubkeys from TLV in Phase 8
    /// Format: hex-encoded 33-byte compressed secp256k1 public key
    /// TODO: Remove in Phase 8 when TLV 0x0004 extraction is implemented
    pub placeholder_locking_pubkey: Option<String>,
}

/// Configuration for the Wallet handler
///
/// This configuration is used by:
/// - TProxy role: To track eHash accounting for multiple downstream miners
/// - JDC role in wallet mode: To track share correlation for JDC operations
///
/// Note: TProxy tracks multiple downstream miners' pubkeys (from user_identity hpubs),
/// so it does not have a single locking_pubkey. The pubkeys come from correlation events.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WalletConfig {
    /// Optional mint URL for HASH unit wallet integration
    /// If provided, enables automatic wallet operations for correlation tracking
    pub mint_url: Option<MintUrl>,

    /// Maximum retry attempts before disabling wallet operations
    /// Default: 10
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Backoff multiplier in seconds for retry logic
    /// Default: 2
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: u64,

    /// Enable automatic recovery from failures
    /// Default: true
    #[serde(default = "default_recovery_enabled")]
    pub recovery_enabled: bool,

    /// Optional log level for wallet operations
    pub log_level: Option<String>,
}

/// Configuration for JDC role eHash operations
///
/// The JDC can operate in either mint or wallet mode:
/// - Mint mode: JDC mints eHash tokens for shares it validates
/// - Wallet mode: JDC tracks correlation data like TProxy does
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JdcEHashConfig {
    /// JDC eHash mode: "mint" or "wallet"
    pub mode: JdcEHashMode,

    /// Mint configuration (required when mode = Mint)
    pub mint: Option<MintConfig>,

    /// Wallet configuration (required when mode = Wallet)
    pub wallet: Option<WalletConfig>,
}

/// JDC eHash operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JdcEHashMode {
    /// JDC acts as a mint, processing share validation results
    Mint,
    /// JDC acts as a wallet, processing SubmitSharesSuccess correlation
    Wallet,
}

// Default value functions

fn default_supported_units() -> Vec<CurrencyUnit> {
    vec![
        CurrencyUnit::Custom("HASH".to_string()),
        CurrencyUnit::Sat,
    ]
}

fn default_min_leading_zeros() -> u32 {
    32
}

fn default_max_retries() -> u32 {
    10
}

fn default_backoff_multiplier() -> u64 {
    2
}

fn default_recovery_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mint_config_defaults() {
        let config_toml = r#"
            mint_url = "https://mint.example.com"
        "#;

        let config: MintConfig = toml::from_str(config_toml).unwrap();
        assert_eq!(config.min_leading_zeros, 32);
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.backoff_multiplier, 2);
        assert!(config.recovery_enabled);
        assert_eq!(config.supported_units.len(), 2);
    }

    #[test]
    fn test_mint_config_custom_values() {
        let config_toml = r#"
            mint_url = "https://mint.example.com"
            min_leading_zeros = 40
            max_retries = 5
            recovery_enabled = false
        "#;

        let config: MintConfig = toml::from_str(config_toml).unwrap();
        assert_eq!(config.min_leading_zeros, 40);
        assert_eq!(config.max_retries, 5);
        assert!(!config.recovery_enabled);
    }

    #[test]
    fn test_wallet_config() {
        let config_toml = r#"
            mint_url = "https://mint.example.com"
        "#;

        let config: WalletConfig = toml::from_str(config_toml).unwrap();
        assert!(config.mint_url.is_some());
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.backoff_multiplier, 2);
        assert!(config.recovery_enabled);
    }

    #[test]
    fn test_jdc_ehash_config_mint_mode() {
        let config_toml = r#"
            mode = "mint"

            [mint]
            mint_url = "https://mint.example.com"
            min_leading_zeros = 32
        "#;

        let config: JdcEHashConfig = toml::from_str(config_toml).unwrap();
        assert_eq!(config.mode, JdcEHashMode::Mint);
        assert!(config.mint.is_some());
        assert!(config.wallet.is_none());
    }

    #[test]
    fn test_jdc_ehash_config_wallet_mode() {
        let config_toml = r#"
            mode = "wallet"

            [wallet]
            mint_url = "https://mint.example.com"
        "#;

        let config: JdcEHashConfig = toml::from_str(config_toml).unwrap();
        assert_eq!(config.mode, JdcEHashMode::Wallet);
        assert!(config.wallet.is_some());
        assert!(config.mint.is_none());
    }
}
