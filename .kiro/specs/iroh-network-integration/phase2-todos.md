# Phase 2 TODOs - Future Implementation Markers

This document tracks all TODO comments added during Phase 2 to mark code and configuration that will need implementation in future phases.

## Configuration Files

### Pool Configuration
**File:** `roles/pool/config-examples/pool-config-iroh-example.toml`

**TODO (Phase 4):** Lines 28-30
```toml
# TODO(Phase 4): Implement Iroh listener in Pool main.rs
#   This configuration is defined but the Pool doesn't yet initialize an Iroh endpoint
#   or accept incoming Iroh connections. Phase 4 will implement the listener logic.
```

**TODO (Phase 4):** Lines 58-61
```toml
# TODO(Phase 4): After Phase 4 implementation is complete, starting the Pool with
#   Iroh enabled will print the NodeId to the logs. Check for a line like:
#   "Pool Iroh NodeId: 6jfhwvskg5txs7jckzs7slqpfm3gqjr45kf7ry65vg7kqi5z7q3q"
# This NodeId is what Translators need to connect to this Pool via Iroh.
```

### Translator Configuration
**File:** `roles/translator/config-examples/tproxy-config-iroh-example.toml`

**TODO (Phase 5):** Lines 41-43
```toml
# TODO(Phase 5): Implement Iroh transport support in Upstream::new()
#   The UpstreamTransport enum is defined but not yet used in the connection logic.
#   Once Phase 5 is complete, you will be able to configure Iroh upstreams like this:
```

## Source Code Files

### Pool Configuration Module
**File:** `roles/pool/src/lib/config.rs`

**TODO (Phase 4):** Lines 22-26
```rust
/// TODO(Phase 4): This configuration is read but not yet used to initialize an Iroh endpoint.
/// Phase 4 will implement:
/// - Iroh endpoint initialization in Pool main.rs
/// - Protocol handler for accepting incoming Iroh connections
/// - Secret key persistence and NodeId logging
```

**What needs to be done:**
- Read `iroh_config` from PoolConfig in `main.rs`
- Initialize Iroh endpoint with secret key from config
- Set up protocol handler for ALPN "sv2-m"
- Accept incoming connections and perform Noise handshake
- Log the Pool's NodeId on startup

### Translator Configuration Module
**File:** `roles/translator/src/lib/config.rs`

**TODO (Phase 5):** Lines 24-25
```rust
/// TODO(Phase 5): This configuration is read but not yet used to initialize an Iroh endpoint.
/// Phase 5 will implement Iroh endpoint initialization in Translator main.rs.
```

**TODO (Phase 5):** Lines 32-33
```rust
/// TODO(Phase 5): This enum is defined but not yet used in Upstream::new().
/// Phase 5 will implement the connection logic that switches based on transport type.
```

**What needs to be done:**
- Read `iroh_config` from TranslatorConfig in `main.rs`
- Initialize Iroh endpoint with secret key from config
- Modify `Upstream::new()` to handle `UpstreamTransport::Iroh` variant
- Connect to Pool's NodeId using the ALPN protocol
- Perform Noise handshake over Iroh connection

### Network Helpers Module
**File:** `roles/roles-utils/network-helpers/src/lib.rs`

**TODO (Phase 3):** Lines 6-8
```rust
// TODO(Phase 3): Add iroh_connection module
// #[cfg(feature = "iroh")]
// pub mod iroh_connection;
```

**What needs to be done:**
- Create `roles/roles-utils/network-helpers/src/iroh_connection.rs`
- Define type aliases for Iroh streams with Noise:
  - `NoiseIrohStream<Message>`
  - `NoiseIrohReadHalf<Message>`
  - `NoiseIrohWriteHalf<Message>`
- Implement `Connection::new_iroh()` method
- Reuse existing reader/writer spawn logic from noise_connection

## Implementation Dependencies

```
Phase 3 (Iroh Connection Module)
    ↓
Phase 4 (Pool Integration) ← Depends on Phase 3
    ↓
Phase 5 (Translator Integration) ← Depends on Phase 3 and Phase 4
    ↓
Phase 6 (Testing & Validation)
    ↓
Phase 7 (Documentation)
```

## Verification Checklist

After each phase is implemented, these TODOs should be:
1. ✅ Addressed with actual implementation
2. ✅ Removed or updated with new status
3. ✅ Tested to ensure functionality works as documented
4. ✅ Configuration examples updated to show working usage

## Notes

- All TODOs are gated behind `#[cfg(feature = "iroh")]` or documented in example configs
- No runtime impact when iroh feature is disabled
- Configuration structures are fully defined and ready to use
- Only the connection/initialization logic needs implementation
