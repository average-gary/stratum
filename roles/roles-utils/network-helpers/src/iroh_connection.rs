//! Iroh network connection handling with Noise protocol encryption.
//!
//! This module provides the `Connection::new_iroh()` method for creating secure connections
//! over Iroh's QUIC-based transport. It wraps Iroh's bidirectional streams with the Noise
//! protocol for application-level encryption and authentication, following the Stratum V2
//! specification.
//!
//! # Architecture
//!
//! ```text
//! Iroh QUIC Connection
//!     ↓
//! RecvStream / SendStream (bidirectional)
//!     ↓
//! NoiseStream (handshake + encryption)
//!     ↓
//! StandardEitherFrame<Message>
//!     ↓
//! async_channel (Receiver/Sender)
//!     ↓
//! Application (Pool / Translator)
//! ```
//!
//! # Security Model
//!
//! **Defense in Depth:**
//! - **Transport Layer (Iroh/QUIC):** TLS 1.3 encryption, protects against network-level attacks
//! - **Application Layer (Noise):** ChaCha20-Poly1305 encryption, validates Pool authority keys
//!
//! This double encryption ensures that even if an Iroh relay is compromised, the application
//! data remains secure and authenticated.

#![cfg(feature = "iroh")]

use crate::{noise_stream::NoiseStream, Error};
use async_channel::{unbounded, Receiver, Sender};
use codec_sv2::{
    binary_sv2::{Deserialize, GetSize, Serialize},
    HandshakeRole, StandardEitherFrame,
};
use iroh::endpoint::{Connection as IrohConnection, RecvStream, SendStream};
use std::sync::Arc;
use tokio::task;
use tracing::{debug, error};

/// Type alias for NoiseStream over Iroh's RecvStream and SendStream.
///
/// This combines Iroh's QUIC transport with Noise protocol encryption.
pub type NoiseIrohStream<Message> = NoiseStream<RecvStream, SendStream, Message>;

/// The reading half of a Noise-encrypted Iroh stream.
pub type NoiseIrohReadHalf<Message> = crate::noise_stream::NoiseReadHalf<RecvStream, Message>;

/// The writing half of a Noise-encrypted Iroh stream.
pub type NoiseIrohWriteHalf<Message> = crate::noise_stream::NoiseWriteHalf<SendStream, Message>;

/// Connection state shared between reader and writer tasks.
struct ConnectionState<Message> {
    sender_incoming: Sender<StandardEitherFrame<Message>>,
    receiver_incoming: Receiver<StandardEitherFrame<Message>>,
    sender_outgoing: Sender<StandardEitherFrame<Message>>,
    receiver_outgoing: Receiver<StandardEitherFrame<Message>>,
}

impl<Message> ConnectionState<Message> {
    /// Closes all channels, signaling shutdown to both reader and writer tasks.
    fn close_all(&self) {
        self.sender_incoming.close();
        self.receiver_incoming.close();
        self.sender_outgoing.close();
        self.receiver_outgoing.close();
    }
}

/// Extension trait for `crate::Connection` to add Iroh support.
pub trait ConnectionIrohExt {
    /// Creates a new connection over an Iroh QUIC connection with Noise protocol encryption.
    ///
    /// # Arguments
    ///
    /// * `connection` - An established Iroh connection (QUIC transport)
    /// * `role` - The Noise handshake role (Initiator or Responder)
    ///
    /// # Returns
    ///
    /// Returns a tuple of:
    /// - `Receiver<StandardEitherFrame<Message>>` - Receives incoming frames from the peer
    /// - `Sender<StandardEitherFrame<Message>>` - Sends outgoing frames to the peer
    ///
    /// # Security
    ///
    /// - The Iroh connection provides TLS 1.3 encryption at the transport layer
    /// - The Noise handshake adds application-level authentication and encryption
    /// - Together, they provide defense-in-depth security
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use codec_sv2::HandshakeRole;
    /// use roles_logic_sv2::utils::Mutex;
    /// use std::sync::Arc;
    ///
    /// // Translator (Initiator)
    /// let iroh_endpoint = iroh::Endpoint::builder().bind_port(0).build().await?;
    /// let connection = iroh_endpoint.connect(pool_node_id, b"sv2-m").await?;
    /// let initiator = codec_sv2::Initiator::from_raw_k(pool_authority_pubkey)?;
    /// let (receiver, sender) = Connection::new_iroh(
    ///     connection,
    ///     HandshakeRole::Initiator(initiator),
    /// ).await?;
    ///
    /// // Pool (Responder)
    /// let responder_keypair = /* ... */;
    /// let (receiver, sender) = Connection::new_iroh(
    ///     connection,
    ///     HandshakeRole::Responder(responder_keypair),
    /// ).await?;
    /// ```
    fn new_iroh<Message>(
        connection: IrohConnection,
        role: HandshakeRole,
    ) -> impl std::future::Future<
        Output = Result<
            (
                Receiver<StandardEitherFrame<Message>>,
                Sender<StandardEitherFrame<Message>>,
            ),
            Error,
        >,
    >
    where
        Message: Serialize + Deserialize<'static> + GetSize + Send + 'static;
}

impl ConnectionIrohExt for crate::noise_connection::Connection {
    async fn new_iroh<Message>(
        connection: IrohConnection,
        role: HandshakeRole,
    ) -> Result<
        (
            Receiver<StandardEitherFrame<Message>>,
            Sender<StandardEitherFrame<Message>>,
        ),
        Error,
    >
    where
        Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
    {
        // Open a bidirectional stream over the Iroh connection
        let (send_stream, recv_stream) = connection
            .open_bi()
            .await
            .map_err(|e| Error::IrohConnectionError(format!("Failed to open bi stream: {}", e)))?;

        debug!("Opened bidirectional stream over Iroh connection");

        // Create async channels for message passing
        let (sender_incoming, receiver_incoming) = unbounded();
        let (sender_outgoing, receiver_outgoing) = unbounded();

        let conn_state = Arc::new(ConnectionState {
            sender_incoming,
            receiver_incoming: receiver_incoming.clone(),
            sender_outgoing: sender_outgoing.clone(),
            receiver_outgoing,
        });

        // Perform Noise handshake over the Iroh stream
        let noise_stream = NoiseStream::<RecvStream, SendStream, Message>::new(
            recv_stream,
            send_stream,
            role.clone(),
        )
        .await
        .map_err(|e| {
            Error::IrohConnectionError(format!("Noise handshake failed over Iroh: {:?}", e))
        })?;

        debug!("Noise handshake completed successfully over Iroh");

        // Split into read and write halves
        let (read_half, write_half) = noise_stream.into_split();

        // Spawn reader and writer tasks
        spawn_reader(read_half, Arc::clone(&conn_state));
        spawn_writer(write_half, conn_state);

        Ok((receiver_incoming, sender_outgoing))
    }
}

/// Spawns a reader task that continuously reads frames from the Iroh stream
/// and forwards them to the incoming channel.
fn spawn_reader<Message>(
    mut read_half: NoiseIrohReadHalf<Message>,
    conn_state: Arc<ConnectionState<Message>>,
) -> task::JoinHandle<()>
where
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    let sender_incoming = conn_state.sender_incoming.clone();

    task::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    debug!("Iroh reader received shutdown signal.");
                    break;
                }
                res = read_half.read_frame() => match res {
                    Ok(frame) => {
                        if sender_incoming.send(frame).await.is_err() {
                            error!("Iroh reader: channel closed, shutting down.");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Iroh reader: error while reading frame: {e:?}");
                        break;
                    }
                }
            }
        }

        conn_state.close_all();
    })
}

/// Spawns a writer task that continuously reads frames from the outgoing channel
/// and writes them to the Iroh stream.
fn spawn_writer<Message>(
    mut write_half: NoiseIrohWriteHalf<Message>,
    conn_state: Arc<ConnectionState<Message>>,
) -> task::JoinHandle<()>
where
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    let receiver_outgoing = conn_state.receiver_outgoing.clone();

    task::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    debug!("Iroh writer received shutdown signal.");
                    break;
                }
                res = receiver_outgoing.recv() => match res {
                    Ok(frame) => {
                        if let Err(e) = write_half.write_frame(frame).await {
                            error!("Iroh writer: error while writing frame: {e:?}");
                            break;
                        }
                    }
                    Err(_) => {
                        debug!("Iroh writer: channel closed, shutting down.");
                        break;
                    }
                }
            }
        }

        if let Err(e) = write_half.shutdown().await {
            error!("Iroh writer: error during shutdown: {e:?}");
        }

        conn_state.close_all();
    })
}
