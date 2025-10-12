//! # Iroh Protocol Handler for SV2 Mining
//!
//! This module implements the Iroh protocol handler for accepting incoming connections
//! from Translators using the Stratum V2 mining ALPN (`sv2-m`).
//!
//! ## Architecture
//!
//! ```text
//! Translator                                Pool
//!     |                                      |
//!     |  1. Iroh Connection (QUIC/TLS)      |
//!     |------------------------------------>|
//!     |                                      |
//!     |  2. Noise Handshake                 |
//!     |<----------------------------------->|
//!     |     (Pool as Responder)             |
//!     |                                      |
//!     |  3. SV2 SetupConnection             |
//!     |<----------------------------------->|
//!     |                                      |
//!     |  4. Mining Messages                 |
//!     |<----------------------------------->|
//! ```
//!
//! The handler performs the following steps:
//! 1. Accept incoming Iroh connection with `sv2-m` ALPN
//! 2. Open bidirectional stream
//! 3. Perform Noise handshake (Pool as responder)
//! 4. Complete SV2 SetupConnection handshake
//! 5. Create Downstream instance and add to Pool
//!
//! ## Security
//!
//! - **Transport Layer (Iroh/QUIC):** TLS 1.3 encryption, NodeId authentication
//! - **Application Layer (Noise):** Authority key verification, ChaCha20-Poly1305 encryption
//!
//! This provides defense-in-depth: even if the Iroh relay is compromised, the Noise protocol
//! ensures that only authorized Translators (with correct Pool authority public key) can connect.

#[cfg(feature = "iroh")]
use crate::{
    error::{PoolError, PoolResult},
    mining_pool::Downstream,
    status,
};
#[cfg(feature = "iroh")]
use config_helpers_sv2::CoinbaseRewardScript;
#[cfg(feature = "iroh")]
use key_utils::Secp256k1SecretKey;
#[cfg(feature = "iroh")]
use std::sync::Arc;
#[cfg(feature = "iroh")]
use stratum_common::{
    network_helpers_sv2::iroh_connection::ConnectionIrohExt,
    roles_logic_sv2::{
        codec_sv2::{HandshakeRole, Responder},
        utils::Mutex,
    },
};
#[cfg(feature = "iroh")]
use tracing::{debug, error, info};

#[cfg(feature = "iroh")]
use super::Pool;

#[cfg(feature = "iroh")]
/// Protocol handler for accepting SV2 mining connections over Iroh.
///
/// This handler is registered with the Iroh Router for the `sv2-m` ALPN.
/// It manages the full connection lifecycle from Iroh connection acceptance
/// through Noise handshake and SV2 setup.
#[derive(Clone)]
pub struct Sv2MiningProtocolHandler {
    /// Pool's authority secret key for Noise handshake (Pool is responder)
    pub responder_keypair: Secp256k1SecretKey,
    /// Certificate validity duration for Noise handshake
    pub cert_validity_sec: u64,
    /// Reference to the main Pool state for adding new downstreams
    pub pool: Arc<Mutex<Pool>>,
    /// Status channel for reporting connection events
    pub status_tx: status::Sender,
    /// Target shares per minute for connected miners
    pub shares_per_minute: f32,
    /// Pool's coinbase reward script
    pub coinbase_reward_script: CoinbaseRewardScript,
}

#[cfg(feature = "iroh")]
impl std::fmt::Debug for Sv2MiningProtocolHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sv2MiningProtocolHandler")
            .field("responder_keypair", &"<secret>")
            .field("cert_validity_sec", &self.cert_validity_sec)
            .field("pool", &"<pool>")
            .field("status_tx", &"<status_tx>")
            .field("shares_per_minute", &self.shares_per_minute)
            .field("coinbase_reward_script", &self.coinbase_reward_script)
            .finish()
    }
}

#[cfg(feature = "iroh")]
impl Sv2MiningProtocolHandler {
    /// Create a new protocol handler instance.
    pub fn new(
        responder_keypair: Secp256k1SecretKey,
        cert_validity_sec: u64,
        pool: Arc<Mutex<Pool>>,
        status_tx: status::Sender,
        shares_per_minute: f32,
        coinbase_reward_script: CoinbaseRewardScript,
    ) -> Self {
        Self {
            responder_keypair,
            cert_validity_sec,
            pool,
            status_tx,
            shares_per_minute,
            coinbase_reward_script,
        }
    }
}

#[cfg(feature = "iroh")]
impl iroh::protocol::ProtocolHandler for Sv2MiningProtocolHandler {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        let remote_node_id = connection.remote_node_id()?;

        info!("Accepted Iroh connection from NodeId: {}", remote_node_id);

        // Handle the connection in the same task
        // The ProtocolHandler::accept future runs on its own task, so we don't need to spawn
        if let Err(e) = self.handle_connection(connection).await {
            error!("Error handling Iroh connection from {:?}: {:?}", remote_node_id, e);
            // Convert PoolError to AcceptError using the from_err method
            return Err(iroh::protocol::AcceptError::from_err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Connection handling failed: {:?}", e),
            )));
        }

        Ok(())
    }
}

#[cfg(feature = "iroh")]
impl Sv2MiningProtocolHandler {
    /// Handle a single Iroh connection through its full lifecycle.
    ///
    /// This method performs:
    /// 1. Opening a bidirectional stream
    /// 2. Noise handshake (Pool as responder)
    /// 3. SV2 SetupConnection handshake
    /// 4. Creating and registering a Downstream instance
    async fn handle_connection(&self, connection: iroh::endpoint::Connection) -> PoolResult<()> {
        let remote_node_id = connection.remote_node_id()
            .map_err(|e| PoolError::Custom(format!("Failed to get remote node ID: {:?}", e)))?;

        debug!("Starting Noise handshake with {:?}", remote_node_id);

        // Get the public key bytes from the authority secret key
        // We need to construct the x-only public key (32 bytes) from the secret key
        let secret_key_bytes = self.responder_keypair.into_bytes();
        let secret_key = secp256k1::SecretKey::from_slice(&secret_key_bytes)
            .map_err(|e| PoolError::Custom(format!("Invalid secret key: {:?}", e)))?;
        let secp = secp256k1::Secp256k1::new();
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &secret_key);
        let (x_only_public_key, _parity) = keypair.x_only_public_key();
        let public_key_bytes = x_only_public_key.serialize();

        // Create Noise responder with Pool's authority keys
        let responder_result = Responder::from_authority_kp(
            &public_key_bytes,
            &secret_key_bytes,
            std::time::Duration::from_secs(self.cert_validity_sec),
        );

        let responder = match responder_result {
            Ok(r) => r,
            Err(e) => {
                error!(
                    "Failed to create Noise responder for {:?}: {:?}",
                    remote_node_id, e
                );
                return Err(PoolError::Custom(format!(
                    "Failed to create Noise responder: {:?}",
                    e
                )));
            }
        };

        // Perform Noise handshake over Iroh connection
        // This uses the ConnectionIrohExt trait to create a Noise connection
        use stratum_common::network_helpers_sv2::noise_connection::Connection as NoiseConnection;
        let (receiver, sender) =
            match NoiseConnection::new_iroh::<crate::mining_pool::Message>(connection, HandshakeRole::Responder(responder))
                .await
            {
                Ok(channels) => channels,
                Err(e) => {
                    error!(
                        "Noise handshake failed with {:?}: {:?}",
                        remote_node_id, e
                    );
                    return Err(PoolError::Custom(format!(
                        "Noise handshake failed: {:?}",
                        e
                    )));
                }
            };

        info!("Noise handshake completed with {:?}", remote_node_id);

        // For Iroh connections, we use a synthetic SocketAddr since Iroh uses NodeId-based addressing
        // We'll use a placeholder address (127.0.0.1:0) since the real identifier is the NodeId
        let address = "127.0.0.1:0".parse().unwrap();

        // Get solution sender from pool
        let solution_sender = self
            .pool
            .safe_lock(|p| p.solution_sender.clone())
            .map_err(|e| PoolError::PoisonLock(e.to_string()))?;

        // Create Downstream instance (this will spawn the message handler task)
        let downstream = Downstream::new(
            receiver,
            sender,
            solution_sender,
            self.pool.clone(),
            self.status_tx.clone().listener_to_connection(),
            address,
            self.shares_per_minute,
            self.coinbase_reward_script.clone(),
        )
        .await?;

        // Get the assigned downstream ID
        let downstream_id = downstream
            .safe_lock(|d| d.id)
            .map_err(|e| PoolError::PoisonLock(e.to_string()))?;

        // Add downstream to pool
        self.pool
            .safe_lock(|p| {
                p.downstreams.insert(downstream_id, downstream);
            })
            .map_err(|e| PoolError::PoisonLock(e.to_string()))?;

        info!(
            "Successfully added Iroh downstream {} from NodeId {} to pool",
            downstream_id, remote_node_id
        );

        Ok(())
    }
}
