pub mod noise_connection;
pub mod noise_stream;
pub mod plain_connection;
#[cfg(feature = "sv1")]
pub mod sv1_connection;
// TODO(Phase 3): Add iroh_connection module
// #[cfg(feature = "iroh")]
// pub mod iroh_connection;

use async_channel::{RecvError, SendError};
use codec_sv2::Error as CodecError;

pub use codec_sv2;

#[cfg(feature = "iroh")]
/// ALPN protocol identifier for Stratum V2 mining protocol over Iroh.
///
/// This constant is used to identify the SV2 mining protocol when establishing
/// connections over Iroh's QUIC transport. It allows multiple protocols to coexist
/// on the same Iroh endpoint.
///
/// Future ALPNs might include:
/// - `b"sv2-jd"` - Job Declaration protocol
/// - `b"sv2-tp"` - Template Provider protocol
pub const ALPN_SV2_MINING: &[u8] = b"sv2-m";

#[derive(Debug)]
pub enum Error {
    HandshakeRemoteInvalidMessage,
    CodecError(CodecError),
    RecvError,
    SendError,
    // This means that a socket that was supposed to be opened have been closed, likley by the
    // peer
    SocketClosed,
    #[cfg(feature = "iroh")]
    /// Error connecting to Iroh endpoint
    IrohConnectionError(String),
    #[cfg(feature = "iroh")]
    /// Error with Iroh endpoint operations
    IrohEndpointError(String),
}

impl From<CodecError> for Error {
    fn from(e: CodecError) -> Self {
        Error::CodecError(e)
    }
}
impl From<RecvError> for Error {
    fn from(_: RecvError) -> Self {
        Error::RecvError
    }
}
impl<T> From<SendError<T>> for Error {
    fn from(_: SendError<T>) -> Self {
        Error::SendError
    }
}
