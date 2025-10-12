//! # Iroh Helper Functions
//!
//! Utilities for managing Iroh endpoints in the Pool, including secret key persistence
//! and endpoint initialization.

#[cfg(feature = "iroh")]
use crate::{config::IrohConfig, error::PoolError};
#[cfg(feature = "iroh")]
use tracing::{info, warn};

#[cfg(feature = "iroh")]
/// Load an existing Iroh secret key from a file, or generate a new one if the file doesn't exist.
///
/// This function ensures that the Pool's NodeId remains stable across restarts by persisting
/// the secret key to disk.
///
/// # Arguments
///
/// * `path` - Optional path to the secret key file. If None, a new key is generated but not saved.
///
/// # Returns
///
/// Returns the loaded or generated `iroh::SecretKey`.
///
/// # Errors
///
/// Returns `PoolError` if file I/O operations fail.
pub fn load_or_generate_secret_key(
    path: &Option<std::path::PathBuf>,
) -> Result<iroh::SecretKey, PoolError> {
    match path {
        Some(path) => {
            if path.exists() {
                // Load existing key
                info!("Loading Iroh secret key from: {}", path.display());
                let key_bytes = std::fs::read(path).map_err(|e| {
                    PoolError::Custom(format!("Failed to read Iroh secret key file: {}", e))
                })?;

                if key_bytes.len() != 32 {
                    return Err(PoolError::Custom(format!(
                        "Invalid secret key file: expected 32 bytes, got {}",
                        key_bytes.len()
                    )));
                }

                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(&key_bytes);
                let secret_key = iroh::SecretKey::from_bytes(&bytes);

                info!("Successfully loaded Iroh secret key");
                Ok(secret_key)
            } else {
                // Generate new key and save it
                info!(
                    "Iroh secret key file not found, generating new key: {}",
                    path.display()
                );
                // Generate random bytes for secret key
                let mut key_bytes = [0u8; 32];
                use rand::RngCore;
                rand::thread_rng().fill_bytes(&mut key_bytes);
                let secret_key = iroh::SecretKey::from_bytes(&key_bytes);

                // Ensure parent directory exists
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        PoolError::Custom(format!("Failed to create key directory: {}", e))
                    })?;
                }

                // Save key to file (as raw 32 bytes)
                let key_bytes = secret_key.to_bytes();

                std::fs::write(path, &key_bytes).map_err(|e| {
                    PoolError::Custom(format!("Failed to write Iroh secret key file: {}", e))
                })?;

                info!("Generated and saved new Iroh secret key to: {}", path.display());
                Ok(secret_key)
            }
        }
        None => {
            // Generate ephemeral key (not saved)
            warn!("No secret key path configured, generating ephemeral Iroh key (NodeId will change on restart)");
            let mut key_bytes = [0u8; 32];
            use rand::RngCore;
            rand::thread_rng().fill_bytes(&mut key_bytes);
            Ok(iroh::SecretKey::from_bytes(&key_bytes))
        }
    }
}

#[cfg(feature = "iroh")]
/// Initialize an Iroh endpoint for the Pool.
///
/// Creates and configures an Iroh endpoint based on the provided configuration.
/// The endpoint will listen for incoming connections and can be used to accept
/// connections from Translators using the Stratum V2 ALPN.
///
/// # Arguments
///
/// * `config` - The Iroh configuration containing secret key path, listen port, and relay settings.
///
/// # Returns
///
/// Returns the initialized `iroh::Endpoint`.
///
/// # Errors
///
/// Returns `PoolError` if endpoint initialization fails.
pub async fn init_iroh_endpoint(config: &IrohConfig) -> Result<iroh::Endpoint, PoolError> {
    info!("Initializing Iroh endpoint for Pool");

    // Load or generate secret key
    let secret_key = load_or_generate_secret_key(&config.secret_key_path)?;

    // Build endpoint
    let mut builder = iroh::Endpoint::builder().secret_key(secret_key);

    // Set relay mode
    // TODO: Support custom relay URL when config.relay_url is Some
    builder = builder.relay_mode(iroh::RelayMode::Default);
    if let Some(ref relay_url) = config.relay_url {
        info!("Custom relay URL configured: {} (Note: custom relay URLs not yet implemented, using default)", relay_url);
    } else {
        info!("Using default Iroh relay server");
    }

    // Set bind port if specified
    if let Some(port) = config.listen_port {
        info!("Configuring Iroh endpoint to use port: {}", port);
        builder = builder.bind_addr_v4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::UNSPECIFIED,
            port,
        ));
    } else {
        info!("Iroh endpoint will use random port");
    }

    // Bind the endpoint
    let endpoint = builder.bind().await.map_err(|e| {
        PoolError::Custom(format!("Failed to initialize Iroh endpoint: {}", e))
    })?;

    // Log the Pool's NodeId - this is what Translators will use to connect
    info!("========================================");
    info!("Pool Iroh NodeId: {}", endpoint.node_id());
    info!("Translators should use this NodeId to connect to this Pool");
    info!("========================================");

    Ok(endpoint)
}
