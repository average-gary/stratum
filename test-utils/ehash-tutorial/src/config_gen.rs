//! Configuration file generation for the tutorial

use anyhow::Result;
use ehashimint::config::{defaults, write_config, PoolConfig};
use std::path::Path;

/// Generate the Pool configuration file for the tutorial
pub async fn generate_pool_config(output_path: &Path) -> Result<()> {
    // Use ehashimint's default pool config with minting enabled
    let config = defaults::pool_config(true);

    // Write to file
    write_config(&config, output_path).await?;

    Ok(())
}

/// Get the pool configuration (either generate or load existing)
pub async fn get_pool_config(config_path: &Path) -> Result<PoolConfig> {
    if config_path.exists() {
        // Load existing config
        ehashimint::config::read_config(config_path).await
    } else {
        // Generate new config
        generate_pool_config(config_path).await?;
        ehashimint::config::read_config(config_path).await
    }
}
