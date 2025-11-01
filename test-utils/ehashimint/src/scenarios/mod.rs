pub mod tproxy_pool;
pub mod tproxy_jdc;
pub mod jdc_pool;

use std::path::PathBuf;
use anyhow::Result;

/// Common functionality for all scenarios
pub struct ScenarioContext {
    pub test_dir: PathBuf,
    pub config_dir: PathBuf,
    pub log_dir: PathBuf,
    pub db_dir: PathBuf,
    pub with_miner: bool,
}

impl ScenarioContext {
    pub async fn new(test_dir: PathBuf, with_miner: bool) -> Result<Self> {
        let config_dir = test_dir.join("configs");
        let log_dir = test_dir.join("logs");
        let db_dir = test_dir.join("dbs");

        // Create directories
        tokio::fs::create_dir_all(&config_dir).await?;
        tokio::fs::create_dir_all(&log_dir).await?;
        tokio::fs::create_dir_all(&db_dir).await?;

        Ok(Self {
            test_dir,
            config_dir,
            log_dir,
            db_dir,
            with_miner,
        })
    }
}

/// Find binary in the workspace
pub fn find_binary(name: &str) -> Result<String> {
    use anyhow::{Context, bail};

    // Try to find workspace root by walking up from current directory
    let current_dir = std::env::current_dir()
        .context("Failed to get current directory")?;

    let workspace_root = find_workspace_root(&current_dir)
        .unwrap_or_else(|| current_dir.clone());

    // Map binary names to their cargo package names
    let package_name = match name {
        "pool_sv2" => "pool_sv2",
        "translator_sv2" => "translator_sv2",
        "jd_client_sv2" => "jd_client_sv2",
        "jd_server" => "jd_server",
        "mining_device" => "mining_device",
        _ => name,
    };

    // Check multiple locations
    let search_paths = vec![
        // Main workspace target (for older structure)
        workspace_root.join("target").join("release").join(name),
        workspace_root.join("target").join("debug").join(name),
        // Roles workspace target (current structure)
        workspace_root.join("roles").join("target").join("release").join(name),
        workspace_root.join("roles").join("target").join("debug").join(name),
        // Test utils for mining_device
        workspace_root.join("roles").join("test-utils").join("target").join("release").join(name),
        workspace_root.join("roles").join("test-utils").join("target").join("debug").join(name),
    ];

    for path in &search_paths {
        if path.exists() {
            return Ok(path.to_string_lossy().to_string());
        }
    }

    // Check if it's in PATH
    if which::which(name).is_ok() {
        return Ok(name.to_string());
    }

    // Not found - provide helpful error message
    bail!(
        "Binary '{}' not found. Please build it first:\n  \
         cargo build --release -p {}\n\
         Searched locations:\n  {}",
        name,
        package_name,
        search_paths.iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// Find workspace root by walking up from current directory
fn find_workspace_root(start: &std::path::Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        // Check for markers that indicate workspace root
        // We want the root with roles/ directory (the main workspace)
        if current.join("roles").exists() && current.join("roles").join("Cargo.toml").exists() {
            return Some(current);
        }

        // Move up one directory
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    // Fallback: look for .git directory
    let mut current = start.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }

        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    None
}
