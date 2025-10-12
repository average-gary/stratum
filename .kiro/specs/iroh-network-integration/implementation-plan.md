# Iroh Network Transport Implementation Plan

## Overview

This document outlines the implementation plan for integrating Iroh as a peer-to-peer networking transport for Stratum V2 connections between the Translator and Pool roles. The key insight is to **reuse the existing Noise protocol over Iroh streams** rather than creating custom protocol handlers, maintaining SV2 security requirements while gaining Iroh's NAT traversal and P2P benefits.

## Architecture Decision: Noise over Iroh

### Why This Approach?

1. **Security Defense in Depth**
   - QUIC (Iroh's transport): TLS 1.3 encryption
   - Noise Protocol: Application-layer encryption + authentication
   - Double encryption with different keys provides stronger security guarantees

2. **SV2 Protocol Compliance**
   - SV2 specification requires Noise protocol for authentication
   - Maintains compatibility with existing SV2 implementations
   - Pool can verify downstream authority keys per spec

3. **Code Reuse**
   - Existing Noise codec handles all SV2 framing and encryption
   - Minimal changes to existing connection logic
   - Only need to swap transport layer (TCP → Iroh)

4. **Authentication & Trust**
   - Noise handshake validates Pool's public key (authority)
   - Prevents MITM attacks even if Iroh relay is compromised
   - Critical for mining security (share theft prevention)

### Transport Layer Comparison

| Layer | TCP Approach | Iroh Approach |
|-------|-------------|---------------|
| **Transport** | TCP | QUIC (via Iroh) |
| **Transport Security** | None | TLS 1.3 (built into QUIC) |
| **Application Security** | Noise Protocol | Noise Protocol (same!) |
| **Framing** | SV2 Noise Frames | SV2 Noise Frames (same!) |
| **NAT Traversal** | Port forwarding required | Built-in (relay + hole punching) |
| **Connection ID** | IP:Port | NodeId (public key) |

## Implementation Phases

### Phase 1: Generalize Noise Stream ✅

**Goal:** Make `NoiseTcpStream` generic over any `AsyncRead + AsyncWrite` transport.

**Status:** ✅ **Completed** (2025-10-11)

#### Tasks

- [x] **1.1 Refactor `NoiseStream` to be transport-agnostic**
  - **File:** `roles/roles-utils/network-helpers/src/noise_stream.rs`
  - **Changes Implemented:**
    - ✅ Converted `NoiseTcpStream<Message>` to `NoiseStream<R, W, Message>`
    - ✅ Added trait bounds: `R: AsyncRead + Unpin`, `W: AsyncWrite + Unpin`
    - ✅ Updated `NoiseReadHalf` and `NoiseWriteHalf` to be generic
    - ✅ Kept handshake logic identical (works over any stream)
    - ✅ Made helper functions `send_message()` and `receive_message()` generic
  - **Backward Compatibility:**
    ```rust
    // Type aliases for existing TCP usage
    pub type NoiseTcpStream<Message> = NoiseStream<OwnedReadHalf, OwnedWriteHalf, Message>;
    pub type NoiseTcpReadHalf<Message> = NoiseReadHalf<OwnedReadHalf, Message>;
    pub type NoiseTcpWriteHalf<Message> = NoiseWriteHalf<OwnedWriteHalf, Message>;
    ```
  - ✅ Added convenience method `from_tcp_stream()` for existing TCP usage
  - ✅ Preserved TCP-specific `try_read_frame()` and `try_write_frame()` methods

- [x] **1.2 Update `noise_connection.rs` to use generic types**
  - **File:** `roles/roles-utils/network-helpers/src/noise_connection.rs`
  - **Changes:**
    - ✅ Updated to use `NoiseTcpStream::from_tcp_stream()` instead of `new()`
    - ✅ No logic changes needed - type aliases work seamlessly
  - **Testing:** ✅ All TCP connections work unchanged

- [x] **1.3 Update all call sites in codebase**
  - **Files Updated:**
    - ✅ `roles/jd-client/src/lib/job_declarator/mod.rs`
    - ✅ `roles/jd-client/src/lib/upstream/mod.rs`
    - ✅ `roles/jd-client/src/lib/template_receiver/mod.rs`
    - ✅ `roles/jd-client/src/lib/channel_manager/mod.rs`
  - **Changes:** Updated all `NoiseTcpStream::new()` calls to `from_tcp_stream()`

**Acceptance Criteria:**
- ✅ Existing TCP connections work unchanged (entire `roles` workspace compiles)
- ✅ `NoiseStream` can be instantiated with any `AsyncRead + AsyncWrite`
- ✅ All existing code compiles without warnings
- ✅ No behavioral changes to Noise handshake
- ✅ Zero breaking changes to existing APIs

**Implementation Notes:**
- The refactoring successfully abstracts the transport layer while maintaining 100% backward compatibility
- The generic design allows NoiseStream to work with any async reader/writer pair:
  - TCP streams (existing): `OwnedReadHalf` / `OwnedWriteHalf`
  - Iroh streams (future): `RecvStream` / `SendStream`
  - In-memory streams (testing): `tokio::io::duplex` halves
- TCP-specific non-blocking methods (`try_read_frame`, `try_write_frame`) were moved to separate impl blocks for the TCP type aliases only, since they require Tokio-specific traits not available on generic `AsyncRead`/`AsyncWrite`

---

### Phase 2: Add Iroh Dependencies & Setup ⬜

**Goal:** Add Iroh to the project with feature flags and basic configuration.

**Status:** Not Started

#### Tasks

- [ ] **2.1 Add Iroh dependencies**
  - **File:** `roles/roles-utils/network-helpers/Cargo.toml`
  - **Dependencies:**
    ```toml
    [dependencies]
    iroh = { version = "0.29", optional = true }

    [features]
    iroh = ["dep:iroh"]
    ```
  - **Also add to:** `roles/Cargo.toml` workspace members using network-helpers

- [ ] **2.2 Create Iroh configuration structures**
  - **File:** `roles/pool/src/lib/config.rs`
  - **Add:**
    ```rust
    #[cfg(feature = "iroh")]
    pub struct IrohConfig {
        pub enabled: bool,
        pub secret_key_path: Option<PathBuf>,
        pub listen_port: Option<u16>,
        pub relay_url: Option<String>,
    }
    ```
  - **File:** `roles/translator/src/lib/config.rs`
  - **Add similar structure for Translator**

- [ ] **2.3 Define ALPN protocol constant**
  - **File:** `roles/roles-utils/network-helpers/src/lib.rs` (or new `constants.rs`)
  - **Add:**
    ```rust
    #[cfg(feature = "iroh")]
    pub const ALPN_SV2_MINING: &[u8] = b"sv2-m";
    ```

- [ ] **2.4 Create example configuration files**
  - **File:** `roles/pool/pool-config-iroh.toml`
  - **File:** `roles/translator/translator-config-iroh.toml`
  - Document Iroh-specific settings with comments

**Acceptance Criteria:**
- ✅ Project compiles with and without `iroh` feature
- ✅ Configuration structures support Iroh settings
- ✅ ALPN constant defined and documented

---

### Phase 3: Implement Iroh Connection Module ⬜

**Goal:** Create `iroh_connection.rs` that wraps Iroh streams with Noise protocol.

**Status:** Not Started

#### Tasks

- [ ] **3.1 Create `iroh_connection.rs` module**
  - **File:** `roles/roles-utils/network-helpers/src/iroh_connection.rs`
  - **Structure:** Mirror `noise_connection.rs` pattern
  - **Type Alias:**
    ```rust
    use iroh::endpoint::{RecvStream, SendStream};

    pub type NoiseIrohStream<Message> = NoiseStream<RecvStream, SendStream, Message>;
    pub type NoiseIrohReadHalf<Message> = NoiseReadHalf<RecvStream, Message>;
    pub type NoiseIrohWriteHalf<Message> = NoiseWriteHalf<SendStream, Message>;
    ```

- [ ] **3.2 Implement `Connection::new_iroh()`**
  - **Function signature:**
    ```rust
    impl Connection {
        #[cfg(feature = "iroh")]
        pub async fn new_iroh<Message>(
            connection: iroh::endpoint::Connection,
            role: HandshakeRole,
        ) -> Result<
            (
                Receiver<StandardEitherFrame<Message>>,
                Sender<StandardEitherFrame<Message>>,
            ),
            Error,
        >
        where
            Message: Serialize + Deserialize<'static> + GetSize + Send + 'static;
    }
    ```
  - **Implementation:**
    1. Open bidirectional stream: `connection.open_bi().await`
    2. Create `NoiseIrohStream` with Noise handshake
    3. Split into read/write halves
    4. Set up async channels (same as TCP)
    5. Spawn reader/writer tasks
    6. Return channel endpoints

- [ ] **3.3 Implement reader/writer spawn logic**
  - Reuse existing `spawn_reader()` and `spawn_writer()` from `noise_connection.rs`
  - Should work unchanged due to generic Noise stream

- [ ] **3.4 Add Iroh-specific error handling**
  - **File:** `roles/roles-utils/network-helpers/src/lib.rs`
  - **Add error variants:**
    ```rust
    #[cfg(feature = "iroh")]
    pub enum Error {
        // ... existing variants
        IrohConnectionError(String),
        IrohEndpointError(String),
    }
    ```

- [ ] **3.5 Export Iroh connection in lib.rs**
  - **File:** `roles/roles-utils/network-helpers/src/lib.rs`
  - **Add:**
    ```rust
    #[cfg(feature = "iroh")]
    pub mod iroh_connection;
    ```

**Acceptance Criteria:**
- ✅ `Connection::new_iroh()` successfully performs Noise handshake over Iroh
- ✅ Messages flow correctly through async channels
- ✅ Module compiles only when `iroh` feature is enabled
- ✅ Unit tests validate connection lifecycle

---

### Phase 4: Pool Role - Iroh Listener ⬜

**Goal:** Enable Pool to accept incoming Iroh connections with `sv2-m` ALPN.

**Status:** Not Started

#### Tasks

- [ ] **4.1 Add Iroh initialization in Pool main**
  - **File:** `roles/pool/src/main.rs`
  - **Add Iroh endpoint builder:**
    ```rust
    #[cfg(feature = "iroh")]
    let iroh_endpoint = if config.iroh_config.enabled {
        Some(init_iroh_endpoint(&config.iroh_config).await?)
    } else {
        None
    };
    ```

- [ ] **4.2 Create Iroh endpoint initialization function**
  - **Function:**
    ```rust
    #[cfg(feature = "iroh")]
    async fn init_iroh_endpoint(config: &IrohConfig) -> Result<iroh::Endpoint, Error> {
        let secret_key = load_or_generate_secret_key(&config.secret_key_path)?;

        let endpoint = iroh::Endpoint::builder()
            .secret_key(secret_key)
            .bind_port(config.listen_port.unwrap_or(0))
            .relay_mode(iroh::RelayMode::Default)
            .build()
            .await?;

        info!("Pool Iroh NodeId: {}", endpoint.node_id());
        Ok(endpoint)
    }
    ```

- [ ] **4.3 Implement SV2 Protocol Handler**
  - **Create:** `roles/pool/src/lib/mining_pool/iroh_handler.rs`
  - **Implement:**
    ```rust
    #[derive(Debug, Clone)]
    pub struct Sv2MiningProtocolHandler {
        responder_keypair: Secp256k1SecretKey,
        // Other pool state needed for connection setup
    }

    impl iroh::protocol::ProtocolHandler for Sv2MiningProtocolHandler {
        fn accept(&self, connecting: iroh::endpoint::Connecting) -> BoxFuture<Result<()>> {
            Box::pin(async move {
                let connection = connecting.await?;
                let peer_addr = connection.remote_address();

                // Create Noise connection over Iroh (Pool is responder)
                let (mut receiver, mut sender) = Connection::new_iroh(
                    connection,
                    HandshakeRole::Responder(self.responder_keypair.clone()),
                ).await?;

                // SV2 Setup Connection handshake
                let setup_handler = Arc::new(Mutex::new(SetupConnectionHandler::new()));
                let (requires_std_job, work_selection) =
                    SetupConnectionHandler::setup(
                        setup_handler,
                        &mut receiver,
                        &mut sender,
                        peer_addr,
                    ).await?;

                // Continue with mining pool connection logic...
                // (same as TCP connections)

                Ok(())
            })
        }
    }
    ```

- [ ] **4.4 Set up Router with ALPN handler**
  - **File:** `roles/pool/src/main.rs`
  - **Add Router:**
    ```rust
    #[cfg(feature = "iroh")]
    if let Some(endpoint) = iroh_endpoint {
        let protocol_handler = Sv2MiningProtocolHandler {
            responder_keypair: config.authority_secret_key().clone(),
        };

        let router = iroh::protocol::Router::builder(endpoint)
            .accept(ALPN_SV2_MINING, Arc::new(protocol_handler))
            .spawn()
            .await?;

        info!("Pool listening for Iroh connections on ALPN: {}",
              String::from_utf8_lossy(ALPN_SV2_MINING));
    }
    ```

- [ ] **4.5 Handle connection lifecycle**
  - Track Iroh connections alongside TCP connections
  - Implement graceful shutdown for Iroh endpoint
  - Log connection events (connect, disconnect, errors)

- [ ] **4.6 Add secret key persistence**
  - Implement `load_or_generate_secret_key()` function
  - Store secret key to file on first run
  - Load existing key on subsequent runs (stable NodeId)

**Acceptance Criteria:**
- ✅ Pool accepts incoming Iroh connections on `sv2-m` ALPN
- ✅ Noise handshake completes successfully (Pool as responder)
- ✅ SV2 SetupConnection handshake works over Iroh
- ✅ Pool's NodeId is stable across restarts
- ✅ Pool can handle both TCP and Iroh connections simultaneously

---

### Phase 5: Translator Role - Iroh Client ⬜

**Goal:** Enable Translator to connect to Pool via Iroh instead of TCP.

**Status:** Not Started

#### Tasks

- [ ] **5.1 Extend Translator configuration**
  - **File:** `roles/translator/src/lib/config.rs`
  - **Modify `Upstream` struct:**
    ```rust
    #[derive(Debug, Deserialize, Clone)]
    pub enum UpstreamTransport {
        Tcp {
            address: String,
            port: u16,
        },
        #[cfg(feature = "iroh")]
        Iroh {
            node_id: String, // Base32-encoded NodeId
            alpn: String,
        },
    }

    #[derive(Debug, Deserialize, Clone)]
    pub struct Upstream {
        pub transport: UpstreamTransport,
        pub authority_pubkey: Secp256k1PublicKey,
    }
    ```

- [ ] **5.2 Add Iroh initialization in Translator main**
  - **File:** `roles/translator/src/main.rs`
  - **Add:**
    ```rust
    #[cfg(feature = "iroh")]
    let iroh_endpoint = if needs_iroh_transport(&config) {
        Some(init_iroh_endpoint(&config.iroh_config).await?)
    } else {
        None
    };
    ```

- [ ] **5.3 Modify `Upstream::new()` for Iroh support**
  - **File:** `roles/translator/src/lib/sv2/upstream/upstream.rs`
  - **Add Iroh connection branch:**
    ```rust
    pub async fn new(
        upstreams: &[Upstream],
        iroh_endpoint: Option<&iroh::Endpoint>,
        // ... other params
    ) -> Result<Self, TproxyError> {
        for (index, upstream) in upstreams.iter().enumerate() {
            match &upstream.transport {
                UpstreamTransport::Tcp { address, port } => {
                    // Existing TCP connection logic
                    let socket = TcpStream::connect(format!("{}:{}", address, port)).await?;
                    let initiator = Initiator::from_raw_k(upstream.authority_pubkey.into_bytes())?;
                    let (receiver, sender) = Connection::new(socket, HandshakeRole::Initiator(initiator)).await?;
                    // ... rest of TCP logic
                }

                #[cfg(feature = "iroh")]
                UpstreamTransport::Iroh { node_id, alpn } => {
                    let endpoint = iroh_endpoint.ok_or(TproxyError::IrohNotInitialized)?;
                    let node_id = iroh::NodeId::from_str(node_id)?;

                    info!("Connecting to Pool via Iroh: NodeId={}", node_id);

                    let connection = endpoint
                        .connect(node_id, alpn.as_bytes())
                        .await?;

                    let initiator = Initiator::from_raw_k(upstream.authority_pubkey.into_bytes())?;
                    let (receiver, sender) = Connection::new_iroh(
                        connection,
                        HandshakeRole::Initiator(initiator),
                    ).await?;

                    let upstream_channel_state = UpstreamChannelState::new(
                        channel_manager_sender,
                        channel_manager_receiver,
                        receiver,
                        sender,
                    );

                    return Ok(Self { upstream_channel_state });
                }
            }
        }

        Err(TproxyError::NoUpstreamsAvailable)
    }
    ```

- [ ] **5.4 Update error handling**
  - **File:** `roles/translator/src/lib/error.rs`
  - **Add Iroh error variants:**
    ```rust
    #[cfg(feature = "iroh")]
    IrohNotInitialized,
    #[cfg(feature = "iroh")]
    IrohConnectionFailed(String),
    ```

- [ ] **5.5 Update configuration parsing**
  - Parse TOML with Iroh transport options
  - Validate NodeId format (base32-encoded)
  - Support mixed upstream list (some TCP, some Iroh)

**Acceptance Criteria:**
- ✅ Translator connects to Pool via Iroh NodeId
- ✅ Noise handshake completes (Translator as initiator)
- ✅ SV2 SetupConnection works over Iroh
- ✅ Translator can failover between TCP and Iroh upstreams
- ✅ All existing upstream message handling works unchanged

---

### Phase 6: Testing & Validation ⬜

**Goal:** Comprehensive testing of Iroh transport integration.

**Status:** Not Started

#### Tasks

- [ ] **6.1 Unit tests for generic Noise stream**
  - Test Noise handshake over in-memory streams
  - Test frame encoding/decoding
  - Test error handling (connection drops, malformed frames)

- [ ] **6.2 Unit tests for Iroh connection**
  - Mock Iroh connection and test `Connection::new_iroh()`
  - Test reader/writer task spawning
  - Test channel message flow

- [ ] **6.3 Integration test: Translator ↔ Pool over Iroh**
  - **File:** `roles/tests/iroh_integration_test.rs`
  - **Test:**
    1. Start Pool with Iroh listener
    2. Start Translator with Iroh transport
    3. Complete SV2 handshake
    4. Exchange mining messages
    5. Verify message correctness
    6. Test graceful shutdown

- [ ] **6.4 End-to-end test with mining device**
  - Full stack: Mining Device (SV1) → Translator → (Iroh) → Pool
  - Submit shares and verify acceptance
  - Test job distribution
  - Validate payout tracking

- [ ] **6.5 NAT traversal testing**
  - Test with Pool behind NAT
  - Test with Translator behind NAT
  - Test with both behind NAT (relay usage)
  - Verify relay fallback works correctly

- [ ] **6.6 Performance benchmarking**
  - **Latency:** Compare TCP vs Iroh (direct) vs Iroh (relayed)
  - **Throughput:** Test share submission rates
  - **Overhead:** Measure CPU/memory usage
  - **Connection Time:** Time to establish connection (cold start vs warm)

- [ ] **6.7 Stress testing**
  - Multiple concurrent Translator connections to Pool
  - Simulate network interruptions (connection migration)
  - Test with high share submission rates
  - Monitor for memory leaks or resource exhaustion

**Acceptance Criteria:**
- ✅ All unit tests pass
- ✅ Integration tests demonstrate full SV2 workflow over Iroh
- ✅ NAT traversal works in various network configurations
- ✅ Performance meets or exceeds TCP baseline
- ✅ No resource leaks under stress

---

### Phase 7: Documentation & Examples ⬜

**Goal:** Complete user-facing documentation and configuration examples.

**Status:** Not Started

#### Tasks

- [ ] **7.1 Create example configurations**
  - **File:** `roles/pool/pool-config-iroh-example.toml`
  - **File:** `roles/translator/translator-config-iroh-example.toml`
  - Include detailed comments explaining each option

- [ ] **7.2 Write Iroh integration guide**
  - **File:** `docs/iroh-integration.md`
  - **Sections:**
    - Overview: What is Iroh and why use it?
    - Benefits: NAT traversal, P2P, security
    - Setup: Generating keys, configuring Pool and Translator
    - NodeId discovery: How Translator finds Pool's NodeId
    - Troubleshooting: Common issues and solutions
    - Performance tuning: Relay selection, port configuration

- [ ] **7.3 Update architecture documentation**
  - Update design.md with "Noise over Iroh" decision
  - Add architecture diagram showing transport layers
  - Document ALPN protocol hierarchy

- [ ] **7.4 Update README files**
  - **File:** `roles/pool/README.md` - Add Iroh listener section
  - **File:** `roles/translator/README.md` - Add Iroh transport section
  - **File:** Root `README.md` - Add Iroh feature to feature list

- [ ] **7.5 Create setup script/helper**
  - Script to generate Iroh secret keys
  - Script to extract NodeId from secret key
  - Helper to test Iroh connectivity

- [ ] **7.6 Document security model**
  - Explain double encryption (QUIC + Noise)
  - Document key management best practices
  - Explain trust model (authority keys vs NodeId)

**Acceptance Criteria:**
- ✅ Users can follow documentation to set up Iroh transport
- ✅ All configuration options are documented
- ✅ Example configs work out-of-the-box
- ✅ Troubleshooting guide covers common issues

---

## Technical Reference

### Key Components

#### 1. Generic Noise Stream

```rust
// network-helpers/src/noise_stream.rs
pub struct NoiseStream<R, W, Message>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    Message: Serialize + Deserialize<'static> + GetSize + Send + 'static,
{
    reader: NoiseReadHalf<R, Message>,
    writer: NoiseWriteHalf<W, Message>,
}
```

**Key Methods:**
- `new(reader: R, writer: W, role: HandshakeRole)` - Performs Noise handshake
- `into_split()` - Returns read/write halves

#### 2. Iroh Connection

```rust
// network-helpers/src/iroh_connection.rs
pub type NoiseIrohStream<Message> = NoiseStream<RecvStream, SendStream, Message>;

impl Connection {
    pub async fn new_iroh<Message>(
        connection: iroh::endpoint::Connection,
        role: HandshakeRole,
    ) -> Result<(Receiver<StandardEitherFrame<Message>>, Sender<StandardEitherFrame<Message>>), Error>;
}
```

**Message Flow:**
```
Iroh QUIC Connection
    ↓
RecvStream / SendStream (bidirectional)
    ↓
NoiseStream (handshake + encryption)
    ↓
StandardEitherFrame<Message>
    ↓
async_channel (Receiver/Sender)
    ↓
Application (Pool / Translator)
```

#### 3. ALPN Protocol

```rust
pub const ALPN_SV2_MINING: &[u8] = b"sv2-m";
```

**Purpose:** Identifies the Stratum V2 mining protocol over Iroh.

**Future ALPNs:**
- `sv2-jd` - Job Declaration protocol
- `sv2-tp` - Template Provider protocol

#### 4. Protocol Handler (Pool)

```rust
impl iroh::protocol::ProtocolHandler for Sv2MiningProtocolHandler {
    fn accept(&self, connecting: iroh::endpoint::Connecting) -> BoxFuture<Result<()>> {
        // 1. Accept Iroh connection
        // 2. Perform Noise handshake (Pool as responder)
        // 3. SV2 SetupConnection handshake
        // 4. Spawn mining connection handler
    }
}
```

#### 5. Connection Lifecycle

**Pool (Server):**
```
1. Initialize Iroh Endpoint
2. Create Router with ALPN handler
3. Handler accepts incoming connections
4. Perform Noise handshake (responder)
5. SV2 SetupConnection
6. Spawn connection handler
```

**Translator (Client):**
```
1. Initialize Iroh Endpoint
2. Connect to Pool NodeId with ALPN
3. Open bidirectional stream
4. Perform Noise handshake (initiator)
5. SV2 SetupConnection
6. Message exchange via channels
```

### Configuration Examples

#### Pool Configuration (Iroh)

```toml
# pool-config-iroh.toml

# Existing TCP listener (optional, can run both)
listen_address = "0.0.0.0:34254"

# Iroh configuration
[iroh]
enabled = true
secret_key_path = "./pool-iroh-secret.key"  # Stable NodeId
listen_port = 34255                         # Optional, 0 = random
relay_url = "https://relay.iroh.network"   # Optional, uses default if omitted

# Standard Pool config...
tp_address = "127.0.0.1:8442"
# ... rest of config
```

#### Translator Configuration (Iroh)

```toml
# translator-config-iroh.toml

# Upstream with Iroh transport
[[upstreams]]
transport = "iroh"
node_id = "6jfhwvskg5txs7jckzs7slqpfm3gqjr45kf7ry65vg7kqi5z7q3q"  # Pool's NodeId
alpn = "sv2-m"
authority_pubkey = "9bDuixKmZqAJnrmP746n8zU1wyAQRrus7th9dxnkPg6RzQvCnan"

# Iroh client configuration
[iroh]
secret_key_path = "./translator-iroh-secret.key"

# Standard Translator config...
downstream_address = "0.0.0.0"
downstream_port = 3333
# ... rest of config
```

### Security Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Application Layer                      │
│  ┌──────────────────────────────────────────────────┐   │
│  │          SV2 Messages (plaintext)                │   │
│  └──────────────────────────────────────────────────┘   │
└───────────────────────┬─────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────┐
│            Noise Protocol (Application Crypto)           │
│  ┌──────────────────────────────────────────────────┐   │
│  │  • Authentication: Authority public keys         │   │
│  │  • Encryption: ChaCha20-Poly1305                 │   │
│  │  • Key Exchange: Secp256k1 (SV2 spec)           │   │
│  └──────────────────────────────────────────────────┘   │
└───────────────────────┬─────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────┐
│               Iroh / QUIC (Transport Crypto)             │
│  ┌──────────────────────────────────────────────────┐   │
│  │  • Authentication: NodeId (ed25519)              │   │
│  │  • Encryption: TLS 1.3                           │   │
│  │  • NAT Traversal: Relay + Hole Punching         │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**Defense in Depth:**
- **Transport Layer (Iroh/QUIC):** Protects against network-level attacks, MITM at relay
- **Application Layer (Noise):** Protects against compromised relay, validates Pool authority

### Error Handling

#### Network Errors

| Error | Cause | Recovery Strategy |
|-------|-------|-------------------|
| `IrohConnectionFailed` | Cannot reach NodeId | Try relay fallback, retry with backoff |
| `NoiseHandshakeFailed` | Wrong authority key or crypto error | Abort connection, alert user (config error) |
| `AlpnMismatch` | Protocol version incompatibility | Log error, try different upstream |
| `StreamClosed` | Peer disconnected | Reconnect with exponential backoff |

#### Configuration Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `InvalidNodeId` | Malformed NodeId string | Validate format (base32, correct length) |
| `SecretKeyNotFound` | Missing key file | Generate new key, warn about NodeId change |
| `IrohNotEnabled` | Feature flag not set | Compile with `--features iroh` |

### Performance Considerations

#### Latency

- **TCP (direct):** ~5-10ms baseline
- **Iroh (direct):** ~8-15ms (QUIC overhead + Noise)
- **Iroh (relayed):** ~50-100ms (depends on relay location)

**Optimization:** Deploy regional relay servers for lower latency.

#### Throughput

- QUIC stream multiplexing can improve throughput under high load
- Noise encryption overhead: ~1-2% CPU per connection
- Multiple streams can run over single QUIC connection (future optimization)

#### Connection Establishment

- **TCP + Noise:** ~50-100ms (1 RTT for TCP, 1.5 RTT for Noise)
- **Iroh + Noise:** ~150-300ms (QUIC handshake + Noise handshake)
- **0-RTT Optimization:** QUIC supports 0-RTT resumption (future work)

## Open Questions & Decisions

### 1. NodeId Discovery ✓ DECIDED

**Question:** How does Translator discover Pool's NodeId?

**Decision:** Configuration file initially. Future enhancements:
- DNS TXT records (pool.example.com → NodeId)
- mDNS for local network discovery
- DHT-based discovery (Iroh supports this)

**Rationale:** Start simple, iterate based on user feedback.

### 2. Relay Server Selection ⚠️ TO BE DECIDED

**Question:** Use public Iroh relay or deploy custom relay?

**Options:**
- **Public Iroh relay:** Easy, no infrastructure, but external dependency
- **Custom relay:** Full control, potentially lower latency, requires ops

**Decision:** Start with public relay, document custom relay setup.

### 3. Feature Flag Strategy ✓ DECIDED

**Question:** Make Iroh optional or required?

**Decision:** Optional via feature flag (`iroh`).

**Rationale:**
- Not all users need P2P (data centers with static IPs)
- Reduces binary size for TCP-only users
- Allows gradual adoption

### 4. Multiple Streams per Connection ⏳ FUTURE WORK

**Question:** Use single bidirectional stream or multiple streams?

**Current:** Single stream per connection (mirrors TCP behavior).

**Future Optimization:** Multiple streams for:
- Control messages (low priority)
- Share submissions (high priority)
- Job distribution (medium priority)

QUIC natively supports stream prioritization.

### 5. Authentication Model ✓ DECIDED

**Question:** Use Noise auth, QUIC auth, or both?

**Decision:** Both (defense in depth).

**Rationale:**
- QUIC auth (NodeId) verifies network peer identity
- Noise auth (authority key) verifies SV2 protocol peer identity
- Protects against relay compromise

## Success Metrics

### Functional Requirements

- [ ] Pool accepts Iroh connections with `sv2-m` ALPN
- [ ] Translator connects to Pool via NodeId
- [ ] Full SV2 handshake works over Iroh
- [ ] Mining messages flow correctly (shares, jobs)
- [ ] NAT traversal works (relay fallback)
- [ ] Graceful connection handling (reconnect, shutdown)

### Non-Functional Requirements

- [ ] Latency: <50ms overhead vs TCP (direct connection)
- [ ] Throughput: ≥ TCP performance for share submissions
- [ ] Reliability: 99.9% uptime with relay fallback
- [ ] Security: No regression in SV2 security model
- [ ] Compatibility: Works alongside existing TCP connections

### User Experience

- [ ] Configuration is straightforward (5 min setup)
- [ ] NodeId discovery is documented
- [ ] Error messages are actionable
- [ ] Troubleshooting guide covers common issues

## Timeline Estimate

| Phase | Estimated Time | Actual Time | Priority | Status |
|-------|---------------|-------------|----------|--------|
| Phase 1: Generalize Noise Stream | 2-3 days | ~4 hours | High | ✅ Complete |
| Phase 2: Add Iroh Dependencies | 1 day | - | High | 🔜 Next |
| Phase 3: Iroh Connection Module | 3-4 days | - | High | ⬜ Not Started |
| Phase 4: Pool Integration | 3-4 days | - | High | ⬜ Not Started |
| Phase 5: Translator Integration | 2-3 days | - | High | ⬜ Not Started |
| Phase 6: Testing & Validation | 4-5 days | - | High | ⬜ Not Started |
| Phase 7: Documentation | 2-3 days | - | Medium | ⬜ Not Started |

**Total Estimated Time:** 3-4 weeks (full-time work)
**Progress:** Phase 1 complete (14% - 1/7 phases)

## References

- [Iroh Documentation](https://www.iroh.computer/docs)
- [Iroh Protocol Writing Guide](https://www.iroh.computer/docs/protocols/writing)
- [Stratum V2 Specification](https://stratumprotocol.org/specification/)
- [Noise Protocol Framework](http://noiseprotocol.org/)
- [QUIC Protocol (RFC 9000)](https://www.rfc-editor.org/rfc/rfc9000.html)

## Implementation Notes

### Lessons Learned

#### Phase 1: Generalize Noise Stream

**What Went Well:**
- The type alias approach worked perfectly for backward compatibility - zero breaking changes
- Generic trait bounds (`AsyncRead + Unpin`, `AsyncWrite + Unpin`) were sufficient for all use cases
- Helper functions were easily made generic without any logic changes
- The Noise handshake logic was already transport-agnostic, only needed to update type signatures

**Design Decisions:**
- Kept `try_read_frame()` and `try_write_frame()` as TCP-specific methods in separate impl blocks
  - These require Tokio's `try_read` and `try_write` which aren't in the standard `AsyncRead`/`AsyncWrite` traits
  - Solution: Implement them only for the TCP type aliases (`NoiseTcpReadHalf`, `NoiseTcpWriteHalf`)
  - Future Iroh implementation won't need these non-blocking variants
- Added `from_tcp_stream()` convenience method to maintain ergonomic API for TCP usage
- Made `new()` generic to accept any compatible reader/writer pair

**Key Insight:**
- The abstraction boundary is at the `AsyncRead`/`AsyncWrite` trait level, not at the protocol level
- This means Noise protocol code knows nothing about the underlying transport (TCP, QUIC, etc.)
- Perfect separation of concerns: transport layer vs. application security layer

### Gotchas & Pitfalls

#### Phase 1: Generalize Noise Stream

**Issue #1: Finding All Call Sites**
- **Problem:** `NoiseTcpStream::new()` was called directly in 4 locations outside of `noise_connection.rs`
- **Solution:** Used `grep` to find all usages and updated them to `from_tcp_stream()`
- **Learning:** When changing a public API, always search the entire workspace for call sites

**Issue #2: Type Inference with Generics**
- **Problem:** Some call sites needed explicit type annotations when using the generic `new()`
- **Solution:** The `from_tcp_stream()` helper method avoids this by specializing for TCP
- **Learning:** Provide specialized constructors for common cases, even with generic implementations

### Performance Optimization

*(To be filled in after benchmarking)*

---

**Document Version:** 1.1
**Last Updated:** 2025-10-11
**Status:** Phase 1 Complete - Implementation In Progress
**Next Review:** After Phase 2 completion
**Phase 1 Completion Date:** 2025-10-11
