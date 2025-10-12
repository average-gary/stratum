//! A Noise-encrypted wrapper around any async read/write stream, providing framed read/write I/O
//! using the SV2 protocol and a stateful Noise handshake.
//!
//! This module provides `NoiseStream`, a generic stream that works with any `AsyncRead + AsyncWrite`
//! transport (e.g., TCP, QUIC, in-memory streams). It performs a Noise-based authenticated key
//! exchange based on the provided [`HandshakeRole`].
//!
//! After a successful handshake, the stream can be split into a `NoiseReadHalf` and
//! `NoiseWriteHalf`, which support frame-based encoding/decoding of SV2 messages with optional
//! non-blocking behavior.
//!
//! For backward compatibility, type aliases are provided:
//! - `NoiseTcpStream` = `NoiseStream<OwnedReadHalf, OwnedWriteHalf, Message>`
//! - `NoiseTcpReadHalf` = `NoiseReadHalf<OwnedReadHalf, Message>`
//! - `NoiseTcpWriteHalf` = `NoiseWriteHalf<OwnedWriteHalf, Message>`

use crate::Error;
use codec_sv2::{
    binary_sv2::{Deserialize, GetSize, Serialize},
    noise_sv2::INITIATOR_EXPECTED_HANDSHAKE_MESSAGE_SIZE,
    HandshakeRole, NoiseEncoder, StandardNoiseDecoder, State,
};
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream,
};

use codec_sv2::{noise_sv2::ELLSWIFT_ENCODING_SIZE, HandShakeFrame, StandardEitherFrame};
use std::convert::TryInto;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, error};

/// A Noise-secured duplex stream that wraps any `AsyncRead + AsyncWrite` transport
/// and provides secure read/write capabilities using the Noise protocol.
///
/// This stream performs the full Noise handshake during construction
/// and returns a bidirectional encrypted stream split into read and write halves.
///
/// **Note:** This struct is **not cancellation-safe**.
/// If `read_frame()` or `write_frame()` is canceled mid-way,
/// internal state may be left in an inconsistent state, which can lead to
/// protocol errors or dropped frames.
pub struct NoiseStream<R, W, Message>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    reader: NoiseReadHalf<R, Message>,
    writer: NoiseWriteHalf<W, Message>,
}

/// The reading half of a `NoiseStream`.
///
/// It buffers incoming encrypted bytes, attempts to decode full Noise frames,
/// and exposes a method to retrieve structured messages of type `Message`.
pub struct NoiseReadHalf<R, Message>
where
    R: AsyncRead + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    reader: R,
    decoder: StandardNoiseDecoder<Message>,
    state: State,
    current_frame_buf: Vec<u8>,
    bytes_read: usize,
}

/// The writing half of a `NoiseStream`.
///
/// It accepts structured messages, encodes them via the Noise protocol,
/// and writes the result to the underlying transport.
pub struct NoiseWriteHalf<W, Message>
where
    W: AsyncWrite + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    writer: W,
    encoder: NoiseEncoder<Message>,
    state: State,
}

// Type aliases for backward compatibility with existing TCP usage
pub type NoiseTcpStream<Message> = NoiseStream<OwnedReadHalf, OwnedWriteHalf, Message>;
pub type NoiseTcpReadHalf<Message> = NoiseReadHalf<OwnedReadHalf, Message>;
pub type NoiseTcpWriteHalf<Message> = NoiseWriteHalf<OwnedWriteHalf, Message>;

impl<R, W, Message> NoiseStream<R, W, Message>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    /// Constructs a new `NoiseStream` over the given reader and writer,
    /// performing the Noise handshake in the given `role`.
    ///
    /// On success, returns a stream with encrypted communication channels.
    pub async fn new(mut reader: R, mut writer: W, role: HandshakeRole) -> Result<Self, Error> {

        let mut decoder = StandardNoiseDecoder::<Message>::new();
        let mut encoder = NoiseEncoder::<Message>::new();
        let mut state = State::initialized(role.clone());

        match role {
            HandshakeRole::Initiator(_) => {
                let mut responder_state = codec_sv2::State::not_initialized(&role);
                let first_msg = state.step_0()?;
                send_message(&mut writer, first_msg.into(), &mut state, &mut encoder).await?;
                debug!("First handshake message sent");

                loop {
                    match receive_message(&mut reader, &mut responder_state, &mut decoder).await {
                        Ok(second_msg) => {
                            debug!("Second handshake message received");
                            let handshake_frame: HandShakeFrame = second_msg
                                .try_into()
                                .map_err(|_| Error::HandshakeRemoteInvalidMessage)?;
                            let payload: [u8; INITIATOR_EXPECTED_HANDSHAKE_MESSAGE_SIZE] =
                                handshake_frame
                                    .get_payload_when_handshaking()
                                    .try_into()
                                    .map_err(|_| Error::HandshakeRemoteInvalidMessage)?;
                            let transport_state = state.step_2(payload)?;
                            state = transport_state;
                            break;
                        }
                        Err(Error::CodecError(codec_sv2::Error::MissingBytes(_))) => {
                            debug!("Waiting for more bytes during handshake");
                        }
                        Err(e) => {
                            error!("Handshake failed with upstream: {:?}", e);
                            return Err(e);
                        }
                    }
                }
            }
            HandshakeRole::Responder(_) => {
                let mut initiator_state = codec_sv2::State::not_initialized(&role);

                loop {
                    match receive_message(&mut reader, &mut initiator_state, &mut decoder).await {
                        Ok(first_msg) => {
                            debug!("First handshake message received");
                            let handshake_frame: HandShakeFrame = first_msg
                                .try_into()
                                .map_err(|_| Error::HandshakeRemoteInvalidMessage)?;
                            let payload: [u8; ELLSWIFT_ENCODING_SIZE] = handshake_frame
                                .get_payload_when_handshaking()
                                .try_into()
                                .map_err(|_| Error::HandshakeRemoteInvalidMessage)?;
                            let (second_msg, transport_state) = state.step_1(payload)?;
                            send_message(&mut writer, second_msg.into(), &mut state, &mut encoder)
                                .await?;
                            debug!("Second handshake message sent");
                            state = transport_state;
                            break;
                        }
                        Err(Error::CodecError(codec_sv2::Error::MissingBytes(_))) => {
                            debug!("Waiting for more bytes during handshake");
                        }
                        Err(e) => {
                            error!("Handshake failed with downstream: {:?}", e);
                            return Err(e);
                        }
                    }
                }
            }
        };
        Ok(Self {
            reader: NoiseReadHalf {
                reader,
                decoder,
                state: state.clone(),
                current_frame_buf: vec![],
                bytes_read: 0,
            },
            writer: NoiseWriteHalf {
                writer,
                encoder,
                state,
            },
        })
    }

    /// Consumes the stream and returns its reader and writer halves.
    pub fn into_split(self) -> (NoiseReadHalf<R, Message>, NoiseWriteHalf<W, Message>) {
        (self.reader, self.writer)
    }
}

// Convenience constructor for TCP streams
impl<Message> NoiseStream<OwnedReadHalf, OwnedWriteHalf, Message>
where
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    /// Constructs a new `NoiseStream` over a TCP stream,
    /// performing the Noise handshake in the given `role`.
    ///
    /// This is a convenience method that splits the TCP stream and calls `new()`.
    pub async fn from_tcp_stream(stream: TcpStream, role: HandshakeRole) -> Result<Self, Error> {
        let (reader, writer) = stream.into_split();
        Self::new(reader, writer, role).await
    }
}

impl<W, Message> NoiseWriteHalf<W, Message>
where
    W: AsyncWrite + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    /// Encrypts and writes a full message frame to the socket.
    ///
    /// Returns an error if the socket is closed or the message cannot be encoded.
    ///
    /// Not cancellation-safe: A canceled write may cause partial writes or state corruption.
    pub async fn write_frame(&mut self, frame: StandardEitherFrame<Message>) -> Result<(), Error> {
        let buf = self.encoder.encode(frame, &mut self.state)?;
        self.writer
            .write_all(buf.as_ref())
            .await
            .map_err(|_| Error::SocketClosed)?;
        Ok(())
    }


    /// Gracefully shuts down the writing half of the stream.
    ///
    /// Returns an error if the shutdown fails.
    pub async fn shutdown(&mut self) -> Result<(), Error> {
        self.writer
            .shutdown()
            .await
            .map_err(|_| Error::SocketClosed)
    }
}

impl<R, Message> NoiseReadHalf<R, Message>
where
    R: AsyncRead + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    /// Reads and decodes a complete frame from the socket.
    ///
    /// This method blocks until a full frame is read and decoded,
    /// handling `MissingBytes` errors from the codec automatically.
    ///
    /// Not cancellation-safe: Cancellation may leave partially-read state behind.
    pub async fn read_frame(&mut self) -> Result<StandardEitherFrame<Message>, Error> {
        loop {
            let expected = self.decoder.writable_len();

            if self.current_frame_buf.len() != expected {
                self.current_frame_buf.resize(expected, 0);
                self.bytes_read = 0;
            }

            while self.bytes_read < expected {
                let n = self
                    .reader
                    .read(&mut self.current_frame_buf[self.bytes_read..])
                    .await
                    .map_err(|_| Error::SocketClosed)?;

                if n == 0 {
                    return Err(Error::SocketClosed);
                }

                self.bytes_read += n;
            }

            self.decoder
                .writable()
                .copy_from_slice(&self.current_frame_buf[..]);

            self.bytes_read = 0;

            match self.decoder.next_frame(&mut self.state) {
                Ok(frame) => return Ok(frame),
                Err(codec_sv2::Error::MissingBytes(_)) => {
                    tokio::task::yield_now().await;
                    continue;
                }
                Err(e) => return Err(Error::CodecError(e)),
            }
        }
    }

}

async fn send_message<W, Message>(
    writer: &mut W,
    msg: StandardEitherFrame<Message>,
    state: &mut State,
    encoder: &mut NoiseEncoder<Message>,
) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    let buffer = encoder.encode(msg, state)?;
    writer
        .write_all(buffer.as_ref())
        .await
        .map_err(|_| Error::SocketClosed)?;
    Ok(())
}

async fn receive_message<R, Message>(
    reader: &mut R,
    state: &mut State,
    decoder: &mut StandardNoiseDecoder<Message>,
) -> Result<StandardEitherFrame<Message>, Error>
where
    R: AsyncRead + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    let mut buffer = vec![0u8; decoder.writable_len()];
    reader
        .read_exact(&mut buffer)
        .await
        .map_err(|_| Error::SocketClosed)?;
    decoder.writable().copy_from_slice(&buffer);
    decoder.next_frame(state).map_err(Error::CodecError)
}

// TCP-specific implementations with try_read/try_write support
impl<Message> NoiseTcpWriteHalf<Message>
where
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    /// Attempts to write a message without blocking.
    ///
    /// Returns:
    /// - `Ok(true)` if the entire frame was written successfully.
    /// - `Ok(false)` if the socket is not ready (would block).
    /// - `Err(_)` on socket or encoding errors.
    pub fn try_write_frame(&mut self, frame: StandardEitherFrame<Message>) -> Result<bool, Error> {
        let buf = self.encoder.encode(frame, &mut self.state)?;

        match self.writer.try_write(buf.as_ref()) {
            Ok(n) if n == buf.len() => Ok(true),
            Ok(_) => Err(Error::SocketClosed),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(_) => Err(Error::SocketClosed),
        }
    }
}

impl<Message> NoiseTcpReadHalf<Message>
where
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    /// Attempts to read and decode a frame without blocking.
    ///
    /// Returns:
    /// - `Ok(Some(frame))` if a full frame is successfully decoded.
    /// - `Ok(None)` if not enough data is available yet.
    /// - `Err(_)` on socket or decoding errors.
    pub fn try_read_frame(&mut self) -> Result<Option<StandardEitherFrame<Message>>, Error> {
        let expected = self.decoder.writable_len();

        if self.current_frame_buf.len() != expected {
            self.current_frame_buf.resize(expected, 0);
            self.bytes_read = 0;
        }

        match self
            .reader
            .try_read(&mut self.current_frame_buf[self.bytes_read..])
        {
            Ok(0) => return Err(Error::SocketClosed),
            Ok(n) => self.bytes_read += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
            Err(_) => return Err(Error::SocketClosed),
        }

        if self.bytes_read < expected {
            return Ok(None);
        }

        self.decoder
            .writable()
            .copy_from_slice(&self.current_frame_buf[..]);

        self.bytes_read = 0;

        match self.decoder.next_frame(&mut self.state) {
            Ok(frame) => Ok(Some(frame)),
            Err(codec_sv2::Error::MissingBytes(_)) => Ok(None),
            Err(e) => Err(Error::CodecError(e)),
        }
    }
}
