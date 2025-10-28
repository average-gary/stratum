//! ## Translator Configuration Module
//!
//! Defines [`TranslatorConfig`], the primary configuration structure for the Translator.
//!
//! This module provides the necessary structures to configure the Translator,
//! managing connections and settings for both upstream and downstream interfaces.
//!
//! This module handles:
//! - Upstream server address, port, and authentication key ([`UpstreamConfig`])
//! - Downstream interface address and port ([`DownstreamConfig`])
//! - Supported protocol versions
//! - Downstream difficulty adjustment parameters ([`DownstreamDifficultyConfig`])
use std::path::{Path, PathBuf};

use stratum_apps::stratum_core::bitcoin::secp256k1::PublicKey;
use ehash_integration::config::WalletConfig;
use ehash_integration::hpub::parse_hpub;
use serde::Deserialize;
use stratum_apps::key_utils::Secp256k1PublicKey;

/// Configuration for the Translator.
#[derive(Debug, Deserialize, Clone)]
pub struct TranslatorConfig {
    pub upstreams: Vec<Upstream>,
    /// The address for the downstream interface.
    pub downstream_address: String,
    /// The port for the downstream interface.
    pub downstream_port: u16,
    /// The maximum supported protocol version for communication.
    pub max_supported_version: u16,
    /// The minimum supported protocol version for communication.
    pub min_supported_version: u16,
    /// The size of the extranonce2 field for downstream mining connections.
    pub downstream_extranonce2_size: u16,
    /// The user identity/username to use when connecting to the pool.
    /// This will be appended with a counter for each mining channel (e.g., username.miner1,
    /// username.miner2).
    pub user_identity: String,
    /// Configuration settings for managing difficulty on the downstream connection.
    pub downstream_difficulty_config: DownstreamDifficultyConfig,
    /// Whether to aggregate all downstream connections into a single upstream channel.
    /// If true, all miners share one channel. If false, each miner gets its own channel.
    pub aggregate_channels: bool,
    /// Optional eHash wallet configuration for tracking downstream miner eHash accounting
    /// When provided, the TProxy will track eHash balances and statistics for multiple
    /// downstream miners. External wallets handle token redemption via authenticated API.
    pub ehash_wallet: Option<WalletConfig>,
    /// Optional default locking pubkey for eHash minting (hex-encoded 33-byte compressed secp256k1)
    /// Used as fallback when downstream SV1 miners don't provide their own locking pubkey
    /// in their username. This enables eHash support for legacy miners that don't know
    /// about eHash protocol extensions. Format: 66 hex characters (e.g., "02a1b2c3...")
    pub default_locking_pubkey: Option<String>,
    /// The path to the log file for the Translator.
    log_file: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Upstream {
    /// The address of the upstream server.
    pub address: String,
    /// The port of the upstream server.
    pub port: u16,
    /// The Secp256k1 public key used to authenticate the upstream authority.
    pub authority_pubkey: Secp256k1PublicKey,
}

impl Upstream {
    /// Creates a new `UpstreamConfig` instance.
    pub fn new(address: String, port: u16, authority_pubkey: Secp256k1PublicKey) -> Self {
        Self {
            address,
            port,
            authority_pubkey,
        }
    }
}

impl TranslatorConfig {
    /// Creates a new `TranslatorConfig` instance with the specified upstream and downstream
    /// configurations and version constraints.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        upstreams: Vec<Upstream>,
        downstream_address: String,
        downstream_port: u16,
        downstream_difficulty_config: DownstreamDifficultyConfig,
        max_supported_version: u16,
        min_supported_version: u16,
        downstream_extranonce2_size: u16,
        user_identity: String,
        aggregate_channels: bool,
    ) -> Self {
        Self {
            upstreams,
            downstream_address,
            downstream_port,
            max_supported_version,
            min_supported_version,
            downstream_extranonce2_size,
            user_identity,
            downstream_difficulty_config,
            aggregate_channels,
            ehash_wallet: None,
            default_locking_pubkey: None,
            log_file: None,
        }
    }

    pub fn set_log_dir(&mut self, log_dir: Option<PathBuf>) {
        if let Some(dir) = log_dir {
            self.log_file = Some(dir);
        }
    }
    pub fn log_dir(&self) -> Option<&Path> {
        self.log_file.as_deref()
    }

    /// Returns the optional eHash wallet configuration.
    pub fn ehash_wallet(&self) -> Option<&WalletConfig> {
        self.ehash_wallet.as_ref()
    }

    /// Returns the optional default locking pubkey for eHash minting.
    /// This pubkey is used as a fallback when downstream miners don't provide their own.
    pub fn default_locking_pubkey(&self) -> Option<&str> {
        self.default_locking_pubkey.as_deref()
    }

    /// Validates the configuration to ensure eHash settings are consistent.
    ///
    /// # Validation Rules
    /// - If `ehash_wallet` is configured, `default_locking_pubkey` MUST also be configured
    /// - If `default_locking_pubkey` is configured, it must be valid hpub format
    ///
    /// # Errors
    /// Returns an error string describing the validation failure.
    pub fn validate_ehash_config(&self) -> Result<(), String> {
        // If ehash_wallet is configured, default_locking_pubkey is required
        if self.ehash_wallet.is_some() && self.default_locking_pubkey.is_none() {
            return Err(
                "Configuration error: 'default_locking_pubkey' is required when 'ehash_wallet' is configured. \
                 This pubkey is used as fallback for miners that don't provide their own. \
                 Format: hpub1... (bech32-encoded secp256k1 public key)".to_string()
            );
        }

        // Validate hpub format if default_locking_pubkey is provided
        if let Some(ref hpub_str) = self.default_locking_pubkey {
            // Check if it starts with hpub1 (hpub HRP + bech32 separator)
            if !hpub_str.starts_with("hpub1") {
                return Err(
                    "Configuration error: 'default_locking_pubkey' must be in hpub format (starting with 'hpub1')".to_string()
                );
            }

            // Try to parse to validate format
            parse_hpub(hpub_str)
                .map_err(|e| format!("Configuration error: invalid 'default_locking_pubkey' hpub format: {}", e))?;
        }

        Ok(())
    }

    /// Decodes the default locking pubkey from hpub format to a secp256k1 PublicKey.
    ///
    /// # Returns
    /// - `Ok(PublicKey)` if configured and valid
    /// - `Err(String)` if not configured or decoding fails
    pub fn decode_default_locking_pubkey(&self) -> Result<PublicKey, String> {
        match &self.default_locking_pubkey {
            Some(hpub_str) => {
                parse_hpub(hpub_str)
                    .map_err(|e| format!("Failed to parse default_locking_pubkey from hpub format: {}", e))
            }
            None => Err("default_locking_pubkey is not configured".to_string()),
        }
    }

    /// Returns a sentinel public key for initialization (indicates no eHash support)
    ///
    /// This uses a well-known "nothing up my sleeve" pubkey (generator point G)
    /// that can be used as a placeholder. The Pool will reject shares with this
    /// sentinel pubkey for eHash minting since no one controls its private key.
    pub fn null_locking_pubkey() -> PublicKey {
        // Use the generator point G as sentinel (well-known, no one has private key)
        // 0279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798
        let g_bytes = hex::decode("0279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798")
            .expect("hardcoded generator point should decode");
        PublicKey::from_slice(&g_bytes)
            .expect("generator point G should always be valid")
    }
}

/// Configuration settings for managing difficulty adjustments on the downstream connection.
#[derive(Debug, Deserialize, Clone)]
pub struct DownstreamDifficultyConfig {
    /// The minimum hashrate expected from an individual miner on the downstream connection.
    pub min_individual_miner_hashrate: f32,
    /// The target number of shares per minute for difficulty adjustment.
    pub shares_per_minute: f32,
    /// Whether to enable variable difficulty adjustment mechanism.
    /// If false, difficulty will be managed by upstream (useful with JDC).
    pub enable_vardiff: bool,
}

impl DownstreamDifficultyConfig {
    /// Creates a new `DownstreamDifficultyConfig` instance.
    pub fn new(
        min_individual_miner_hashrate: f32,
        shares_per_minute: f32,
        enable_vardiff: bool,
    ) -> Self {
        Self {
            min_individual_miner_hashrate,
            shares_per_minute,
            enable_vardiff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use stratum_apps::stratum_core::bitcoin::secp256k1::{Secp256k1, SecretKey};

    fn create_test_upstream() -> Upstream {
        // Use a valid base58-encoded public key from the key-utils test cases
        let pubkey_str = "9bDuixKmZqAJnrmP746n8zU1wyAQRrus7th9dxnkPg6RzQvCnan";
        let pubkey = Secp256k1PublicKey::from_str(pubkey_str).unwrap();
        Upstream::new("127.0.0.1".to_string(), 4444, pubkey)
    }

    fn create_test_difficulty_config() -> DownstreamDifficultyConfig {
        DownstreamDifficultyConfig::new(100.0, 5.0, true)
    }

    fn create_test_hpub() -> String {
        // Create a test pubkey and encode to hpub
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let pubkey = PublicKey::from_secret_key(&secp, &secret);
        ehash_integration::hpub::encode_hpub(&pubkey).unwrap()
    }

    #[test]
    fn test_upstream_creation() {
        let upstream = create_test_upstream();
        assert_eq!(upstream.address, "127.0.0.1");
        assert_eq!(upstream.port, 4444);
    }

    #[test]
    fn test_downstream_difficulty_config_creation() {
        let config = create_test_difficulty_config();
        assert_eq!(config.min_individual_miner_hashrate, 100.0);
        assert_eq!(config.shares_per_minute, 5.0);
        assert!(config.enable_vardiff);
    }

    #[test]
    fn test_translator_config_creation() {
        let upstreams = vec![create_test_upstream()];
        let difficulty_config = create_test_difficulty_config();

        let config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            true,
        );

        assert_eq!(config.upstreams.len(), 1);
        assert_eq!(config.downstream_address, "0.0.0.0");
        assert_eq!(config.downstream_port, 3333);
        assert_eq!(config.max_supported_version, 2);
        assert_eq!(config.min_supported_version, 1);
        assert_eq!(config.downstream_extranonce2_size, 4);
        assert_eq!(config.user_identity, "test_user");
        assert!(config.aggregate_channels);
        assert!(config.log_file.is_none());
    }

    #[test]
    fn test_translator_config_log_dir() {
        let upstreams = vec![create_test_upstream()];
        let difficulty_config = create_test_difficulty_config();

        let mut config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            false,
        );

        assert!(config.log_dir().is_none());

        let log_path = PathBuf::from("/tmp/logs");
        config.set_log_dir(Some(log_path.clone()));
        assert_eq!(config.log_dir(), Some(log_path.as_path()));

        config.set_log_dir(None);
        assert_eq!(config.log_dir(), Some(log_path.as_path())); // Should remain unchanged
    }

    #[test]
    fn test_multiple_upstreams() {
        let upstream1 = create_test_upstream();
        let mut upstream2 = create_test_upstream();
        upstream2.address = "192.168.1.1".to_string();
        upstream2.port = 5555;

        let upstreams = vec![upstream1, upstream2];
        let difficulty_config = create_test_difficulty_config();

        let config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            true,
        );

        assert_eq!(config.upstreams.len(), 2);
        assert_eq!(config.upstreams[0].address, "127.0.0.1");
        assert_eq!(config.upstreams[0].port, 4444);
        assert_eq!(config.upstreams[1].address, "192.168.1.1");
        assert_eq!(config.upstreams[1].port, 5555);
    }

    #[test]
    fn test_vardiff_disabled_config() {
        let mut difficulty_config = create_test_difficulty_config();
        difficulty_config.enable_vardiff = false;

        let upstreams = vec![create_test_upstream()];
        let config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            false,
        );

        assert!(!config.downstream_difficulty_config.enable_vardiff);
        assert!(!config.aggregate_channels);
    }

    #[test]
    fn test_ehash_config_validation_requires_default_pubkey() {
        use ehash_integration::config::WalletConfig;

        let upstreams = vec![create_test_upstream()];
        let difficulty_config = create_test_difficulty_config();

        let mut config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            false,
        );

        // Add ehash_wallet without default_locking_pubkey - should fail validation
        config.ehash_wallet = Some(WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        });

        let result = config.validate_ehash_config();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("default_locking_pubkey' is required"));
    }

    #[test]
    fn test_ehash_config_validation_valid_pubkey() {
        use ehash_integration::config::WalletConfig;

        let upstreams = vec![create_test_upstream()];
        let difficulty_config = create_test_difficulty_config();

        let mut config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            false,
        );

        // Add ehash_wallet WITH valid default_locking_pubkey (hpub format) - should pass validation
        config.ehash_wallet = Some(WalletConfig {
            mint_url: None,
            max_retries: 10,
            backoff_multiplier: 2,
            recovery_enabled: true,
            log_level: None,
        });
        config.default_locking_pubkey = Some(create_test_hpub());

        let result = config.validate_ehash_config();
        assert!(result.is_ok());
    }

    #[test]
    fn test_ehash_config_validation_invalid_pubkey_format() {
        let upstreams = vec![create_test_upstream()];
        let difficulty_config = create_test_difficulty_config();

        let mut config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            false,
        );

        config.default_locking_pubkey = Some("not_an_hpub".to_string()); // Invalid format

        let result = config.validate_ehash_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("hpub format"));
    }

    #[test]
    fn test_ehash_config_validation_invalid_hpub_encoding() {
        let upstreams = vec![create_test_upstream()];
        let difficulty_config = create_test_difficulty_config();

        let mut config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            false,
        );

        // Invalid hpub (starts with hpub1 but has invalid encoding)
        config.default_locking_pubkey = Some("hpub1invalid".to_string());

        let result = config.validate_ehash_config();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid 'default_locking_pubkey' hpub format"));
    }

    #[test]
    fn test_decode_default_locking_pubkey() {
        let upstreams = vec![create_test_upstream()];
        let difficulty_config = create_test_difficulty_config();

        let mut config = TranslatorConfig::new(
            upstreams,
            "0.0.0.0".to_string(),
            3333,
            difficulty_config,
            2,
            1,
            4,
            "test_user".to_string(),
            false,
        );

        config.default_locking_pubkey = Some(create_test_hpub());

        let decoded = config.decode_default_locking_pubkey();
        assert!(decoded.is_ok());
        let pubkey = decoded.unwrap();
        let bytes = pubkey.serialize();
        assert_eq!(bytes.len(), 33);
        // Should be compressed pubkey (02 or 03 prefix)
        assert!(bytes[0] == 0x02 || bytes[0] == 0x03);
    }

    #[test]
    fn test_null_locking_pubkey() {
        let null_pubkey = TranslatorConfig::null_locking_pubkey();
        let bytes = null_pubkey.serialize();
        assert_eq!(bytes.len(), 33);
        assert_eq!(bytes[0], 0x02); // Compressed pubkey prefix
        // Should be the generator point G (well-known point, no one has private key)
        let expected_g = hex::decode("0279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798")
            .expect("valid hex");
        assert_eq!(bytes, expected_g.as_slice());
    }
}
