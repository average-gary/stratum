use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Generate default configurations for eHash testing scenarios
pub mod defaults {
    use super::*;

    /// Default Pool configuration for eHash minting
    pub fn pool_config(mint_enabled: bool) -> PoolConfig {
        PoolConfig {
            authority_public_key: "9auqWEzQDVyd2oe1JVGFLMLHZtCo2FFqZwtKA5gd9xbuEu7PH72".to_string(),
            authority_secret_key: "mkDLTBBRxdBv998612qipDYoTK3YUrqLe8uWw7gu3iXbSrn2n".to_string(),
            cert_validity_sec: 3600,
            listen_address: "127.0.0.1:34254".to_string(),
            coinbase_reward_script: "76a914dc67b90aa0d3e76a7f69c2d5a1eb4ba0e52f474f88ac".to_string(),
            server_id: 0,
            pool_signature: "Tutorial Pool".to_string(),
            tp_address: Some("127.0.0.1:8442".to_string()),
            shares_per_minute: 60.0,
            share_batch_size: 10,
            ehash_mint: if mint_enabled {
                Some(EHashMintConfig {
                    mint_url: "http://127.0.0.1:3338".to_string(),
                    database_url: None,
                    min_leading_zeros: 32,
                })
            } else {
                None
            },
        }
    }

    /// Default TProxy configuration
    pub fn tproxy_config() -> TProxyConfig {
        TProxyConfig {
            upstream_address: "127.0.0.1:34254".to_string(),
            upstream_port: 34254,
            listening_address: "127.0.0.1:34255".to_string(),
            listening_port: 34255,
            ehash_wallet: Some(EHashWalletConfig {
                enabled: true,
                default_locking_pubkey:
                    "hpub1qyq2fw8qdwmhzgfzecvl5a3jyy8v8lf7wj8rfxp8sxvh7vxqzqfxl6yw".to_string(),
            }),
        }
    }

    /// Default JDC configuration for mint mode
    pub fn jdc_mint_config() -> JdcConfig {
        JdcConfig {
            listen_mining_address: "127.0.0.1:34260".to_string(),
            listen_mining_port: 34260,
            jds_address: "127.0.0.1:34264".to_string(),
            upstream_address: Some("127.0.0.1:34254".to_string()),
            upstream_port: Some(34254),
            ehash_mint: Some(EHashMintConfig {
                mint_url: "http://127.0.0.1:3339".to_string(),
                database_url: None,
                min_leading_zeros: 32,
            }),
        }
    }

    /// Default JDC configuration for wallet mode
    pub fn jdc_wallet_config() -> JdcConfig {
        JdcConfig {
            listen_mining_address: "127.0.0.1:34260".to_string(),
            listen_mining_port: 34260,
            jds_address: "127.0.0.1:34264".to_string(),
            upstream_address: Some("127.0.0.1:34254".to_string()),
            upstream_port: Some(34254),
            ehash_mint: None, // Wallet mode - no minting
        }
    }

    /// Default JDS configuration
    pub fn jds_config() -> JdsConfig {
        JdsConfig {
            listen_address: "127.0.0.1:34264".to_string(),
            listen_port: 34264,
            tp_address: Some("127.0.0.1:8442".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub authority_public_key: String,
    pub authority_secret_key: String,
    pub cert_validity_sec: u64,
    pub listen_address: String,
    pub coinbase_reward_script: String,
    pub server_id: u32,
    pub pool_signature: String,
    pub tp_address: Option<String>,
    pub shares_per_minute: f64,
    pub share_batch_size: u32,
    pub ehash_mint: Option<EHashMintConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TProxyConfig {
    pub upstream_address: String,
    pub upstream_port: u16,
    pub listening_address: String,
    pub listening_port: u16,
    pub ehash_wallet: Option<EHashWalletConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdcConfig {
    pub listen_mining_address: String,
    pub listen_mining_port: u16,
    pub jds_address: String,
    pub upstream_address: Option<String>,
    pub upstream_port: Option<u16>,
    pub ehash_mint: Option<EHashMintConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JdsConfig {
    pub listen_address: String,
    pub listen_port: u16,
    pub tp_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EHashMintConfig {
    pub mint_url: String,
    pub database_url: Option<String>,
    pub min_leading_zeros: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EHashWalletConfig {
    pub enabled: bool,
    pub default_locking_pubkey: String,
}

/// Write configuration to TOML file
pub async fn write_config<T: Serialize>(config: &T, path: &Path) -> Result<()> {
    let content = toml::to_string_pretty(config)
        .context("Failed to serialize configuration to TOML")?;

    tokio::fs::write(path, content)
        .await
        .with_context(|| format!("Failed to write config to {}", path.display()))?;

    Ok(())
}

/// Read configuration from TOML file
pub async fn read_config<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read config from {}", path.display()))?;

    let config: T =
        toml::from_str(&content).context("Failed to deserialize TOML configuration")?;

    Ok(config)
}
