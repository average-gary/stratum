# Iroh Network Quick Start Guide

This guide shows you how to quickly run the Pool, Translator, and mining-device using Iroh's peer-to-peer (P2P) network transport instead of traditional TCP connections. Iroh provides NAT traversal capabilities, making it easier to connect components even when they're behind firewalls or NAT.

## What is Iroh?

Iroh is a P2P networking library that provides:
- **NAT Traversal**: Components can connect even behind firewalls/NAT without port forwarding
- **Stable Identity**: Each node has a persistent NodeId derived from a secret key
- **Relay Support**: Automatic fallback to relay servers when direct connection isn't possible
- **ALPN Protocol**: Protocol identification for multiplexing different services

## Prerequisites

1. Rust toolchain installed (see main [README.md](./README.md))
2. Build the roles with Iroh support:
   ```bash
   # Build all roles in the workspace with Iroh features
   cargo build --release --features iroh
   
   # Or build specific roles individually:
   cargo build --release --bin pool_sv2 --features iroh
   cargo build --release --bin translator_sv2 --features iroh
   cargo build --release --manifest-path=roles/test-utils/mining-device/Cargo.toml --features iroh
   ```

## Quick Start: Pool with Iroh

### 1. Start the Pool with Iroh enabled

```bash
cargo run --release --bin pool_sv2 -- -c iroh-pool-config.toml
```

### 2. Find the Pool's NodeId

Look for this line in the Pool startup logs:
```
Pool Iroh listener initialized. NodeId: f6cbfdb5def056fd1814dedcc0f0dd2b982919cbb75c76612aa9bcc404eae1a8
```

**Save this NodeId** - you'll need it to connect clients to the Pool.

### 3. Configuration

The Pool config (`iroh-pool-config.toml`) includes:

```toml
[iroh_config]
enabled = true
secret_key_path = "./pool-iroh-secret.key"  # Stable NodeId across restarts
listen_port = 0  # 0 = random port (recommended)
# relay_url = "https://relay.iroh.network"  # Optional: custom relay
```

Key points:
- `secret_key_path`: File stores the secret key; NodeId is derived from it
- `listen_port`: Set to 0 to let the system choose an available port
- The Pool can run both TCP and Iroh listeners simultaneously

## Quick Start: Translator with Iroh

### 1. Get the Pool's NodeId

You need the Pool's NodeId from the Pool startup logs (see above).

### 2. Update Translator Configuration

Edit `iroh-translator-config.toml` and set the upstream to use Iroh:

```toml
[iroh_config]
secret_key_path = "./translator-iroh-secret.key"

[[upstreams]]
transport = "iroh"
node_id = "YOUR_POOL_NODE_ID_HERE"  # From Pool logs
alpn = "sv2-m"
authority_pubkey = "9auqWEzQDVyd2oe1JVGFLMLHZtCo2FFqZwtKA5gd9xbuEu7PH72"
```

### 3. Start the Translator

```bash
cargo run --release --bin translator_sv2 -- -c iroh-translator-config.toml
```

The Translator will:
- Accept SV1 mining devices on TCP (default: `0.0.0.0:34255`)
- Connect to the Pool via Iroh P2P using the NodeId
- Translate between SV1 (downstream) and SV2 (upstream)

## Quick Start: Mining Device with Iroh

### Method 1: Using the Helper Script (Recommended)

```bash
./run-iroh-mining-device.sh <POOL_NODE_ID>
```

Example:
```bash
./run-iroh-mining-device.sh f6cbfdb5def056fd1814dedcc0f0dd2b982919cbb75c76612aa9bcc404eae1a8
```

### Method 2: Manual Command

```bash
cargo run --release --manifest-path=roles/test-utils/mining-device/Cargo.toml --features iroh -- \
    --pool-iroh-node-id f6cbfdb5def056fd1814dedcc0f0dd2b982919cbb75c76612aa9bcc404eae1a8 \
    --pool-iroh-alpn "sv2-m" \
    --iroh-secret-key-path ./mining-device-iroh-secret.key \
    --pubkey-pool 9auqWEzQDVyd2oe1JVGFLMLHZtCo2FFqZwtKA5gd9xbuEu7PH72 \
    --handicap 1000
```

Key parameters:
- `--pool-iroh-node-id`: The Pool's NodeId from startup logs
- `--pool-iroh-alpn`: Protocol identifier (default: "sv2-m" for SV2 mining)
- `--iroh-secret-key-path`: Path to store mining device's secret key
- `--pubkey-pool`: Pool's authority public key (from config)
- `--handicap`: Microseconds between hashes (higher = slower, for testing)

## Complete Example: End-to-End Setup

### Step 1: Start a Template Provider (required for Pool)

The Pool requires a Template Provider running at `127.0.0.1:8442`. You can use the SV2 Template Provider:

```bash
# Clone and run the SV2 Template Provider
git clone https://github.com/Sjors/sv2-tp.git
cd sv2-tp
git checkout v1.0.2
cargo run --release
```

This will start the Template Provider on the default port `8442`. For more details and configuration options, see the [SV2 Template Provider repository](https://github.com/Sjors/sv2-tp/tree/v1.0.2).

### Step 2: Start the Pool with Iroh

```bash
# In terminal 2
cargo run --release --bin pool_sv2 -- -c iroh-pool-config.toml
```

Copy the NodeId from the logs.

### Step 3a: Option 1 - Connect Mining Device Directly (SV2)

```bash
# In terminal 3
./run-iroh-mining-device.sh <POOL_NODE_ID_FROM_STEP_2>
```

### Step 3b: Option 2 - Connect via Translator (SV1 to SV2)

First, update `iroh-translator-config.toml` with the Pool's NodeId, then:

```bash
# In terminal 3
cargo run --release --bin translator_sv2 -- -c iroh-translator-config.toml

# In terminal 4 - Connect an SV1 miner to the Translator
# Your SV1 mining software should connect to: 127.0.0.1:34255
```

## Configuration Files

Example configurations are provided:

| Component   | Example Config | Description |
|-------------|----------------|-------------|
| Pool        | `iroh-pool-config.toml` | Pool with Iroh listener |
|             | `roles/pool/config-examples/pool-config-iroh-example.toml` | Template with comments |
| Translator  | `iroh-translator-config.toml` | Translator connecting to Pool via Iroh |
|             | `roles/translator/config-examples/tproxy-config-iroh-example.toml` | Template with comments |

## Understanding NodeIds and ALPNs

### NodeId
- A unique identifier for each Iroh node (base32-encoded, 64 characters)
- Derived from the secret key stored in `secret_key_path`
- Remains stable across restarts as long as the secret key file exists
- Example: `f6cbfdb5def056fd1814dedcc0f0dd2b982919cbb75c76612aa9bcc404eae1a8`

### ALPN (Application-Layer Protocol Negotiation)
- Protocol identifier used to multiplex different services over Iroh
- `"sv2-m"`: Stratum V2 Mining protocol
- Allows different protocols to run on the same Iroh endpoint

## Troubleshooting

### "Pool NodeId not found in logs"

Make sure `iroh_config.enabled = true` in your Pool config and check that the Pool started successfully.

### "Connection failed: NodeId not found"

1. Verify the NodeId is correct (64-character base32 string)
2. Ensure the Pool is running and has Iroh enabled
3. Check that both nodes can reach the relay server (or each other directly)

### "Transport error: Iroh feature not enabled"

Build with Iroh support:
```bash
cargo build --release --features iroh
```

### Secret key files

- Secret key files are automatically created on first run if they don't exist
- To get a new NodeId, simply delete the secret key file and restart
- Keep backups of secret keys if you need stable NodeIds for production

## Advantages of Iroh Transport

1. **NAT Traversal**: No port forwarding required
2. **Firewall Friendly**: Works behind most corporate firewalls
3. **Automatic Relay**: Falls back to relay servers when direct connection fails
4. **Flexible Topology**: Easy to deploy distributed setups
5. **Stable Identity**: NodeIds remain constant across restarts/IP changes

## Comparison: TCP vs Iroh

| Feature | TCP | Iroh |
|---------|-----|------|
| NAT Traversal | Requires port forwarding | Automatic |
| Configuration | Need IP:Port | Need NodeId only |
| Firewalls | May require rules | Usually works |
| Identity | Changes with IP | Stable NodeId |
| Relay Support | No | Yes |
| Setup Complexity | Low (local) / High (remote) | Medium |

## Next Steps

- See [roles/pool/README.md](roles/pool/README.md) for detailed Pool documentation
- See [roles/translator/README.md](roles/translator/README.md) for Translator documentation
- See [roles/test-utils/mining-device/README.md](roles/test-utils/mining-device/README.md) for mining device documentation
- Explore custom relay servers for production deployments

## Security Considerations

- **Secret Keys**: Keep your `*-iroh-secret.key` files secure
- **Authority Keys**: The `authority_pubkey` provides authentication via Noise protocol
- **Relay Servers**: Default relay is public; consider running your own for production
- **Network Exposure**: Iroh nodes are discoverable via their NodeId

## Support

For issues or questions:
- Check the main [README.md](./README.md) for general setup
- Review component-specific READMEs in their respective directories
- Check logs for detailed error messages
