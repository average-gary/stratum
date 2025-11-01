# ehashimint - eHash Testing Environment Manager

A Rust-based CLI tool for setting up and managing local eHash testing environments, inspired by Fedimint's `devimint`. Replace shell scripts with a robust, cross-platform solution for testing eHash (ecash hashrate) implementations.

## What is eHash?

eHash is a Cashu ecash-based system for representing hashrate as transferable tokens:

- **Miners earn eHash tokens** for submitting valid shares
- Tokens use **NUT-20 P2PK authentication** (per-share locking pubkeys)
- Tokens are **denominated in "HASH"** units (configurable conversion rate)
- **External wallets** redeem tokens via NUT-04/NUT-20 authenticated flow
- Enables **hashrate markets, derivatives, and DeFi applications**

## Features

- **Three Testing Scenarios**: TProxy-Pool, TProxy-JDC, and JDC-Pool configurations
- **Automatic Process Management**: Start, stop, and monitor all services
- **Built-in Miner Support**: Automatically start CPU miner with `--with-miner`
- **Configuration Generation**: Default configs or use custom TOML files
- **Clean Process Lifecycle**: Proper cleanup and signal handling
- **Status Monitoring**: Check running processes and view logs

## Installation

### Prerequisites

Build all required binaries from the Stratum V2 workspace root:

```bash
# Build core services
cargo build --release -p pool_sv2
cargo build --release -p translator_sv2
cargo build --release -p jd_server
cargo build --release -p jd_client

# Build CPU miner (for --with-miner)
cd roles/test-utils/mining-device
cargo build --release
cd -

# Build ehashimint
cd test-utils/ehashimint
cargo build --release
```

Add to PATH (optional):

```bash
export PATH="$PATH:$(pwd)/target/release"
```

## Quick Start

### Scenario 1: TProxy with Pool Mint (Simplest)

Pool mints eHash, TProxy translates SV1→SV2:

```bash
ehashimint tproxy-pool --with-miner
```

**Architecture:**
```
SV1 Miners → TProxy → Pool (Mint)
```

### Scenario 2: TProxy with JDC Mint and JDS

JDC mints locally, Pool validates shares:

```bash
ehashimint tproxy-jdc --with-miner
```

**Architecture:**
```
SV1 Miners → TProxy → JDC (Mint) → Pool
                        ↕
                       JDS
```

### Scenario 3: JDC with Pool Mint and JDS

JDC as proxy, Pool mints eHash:

```bash
ehashimint jdc-pool --with-miner
```

**Architecture:**
```
SV2 Miners → JDC (Wallet) → Pool (Mint)
              ↕
             JDS
```

## Usage

### Commands

```bash
# Run scenarios
ehashimint tproxy-pool [--with-miner] [--pool-config PATH] [--tproxy-config PATH]
ehashimint tproxy-jdc [--with-miner] [--pool-config PATH] [--tproxy-config PATH] [--jdc-config PATH] [--jds-config PATH]
ehashimint jdc-pool [--with-miner] [--pool-config PATH] [--jdc-config PATH] [--jds-config PATH]

# Management
ehashimint status       # Show running processes
ehashimint stop         # Stop all processes
ehashimint clean        # Clean up test directories

# Options
-d, --test-dir <PATH>   # Custom test directory (default: /tmp/ehashimint-<PID>)
-v, --verbose           # Enable verbose logging
```

### Using Custom Configs

```bash
# Use custom Pool configuration
ehashimint tproxy-pool --pool-config ./my-pool.toml

# Multiple custom configs
ehashimint tproxy-jdc \
  --pool-config ./my-pool.toml \
  --tproxy-config ./my-tproxy.toml \
  --jdc-config ./my-jdc.toml \
  --jds-config ./my-jds.toml
```

### Environment Variables

```bash
export EHASH_TEST_DIR=/path/to/test/dir
ehashimint tproxy-pool  # Will use EHASH_TEST_DIR
```

## Configuration

### Default Configurations

ehashimint generates sensible defaults for each scenario. See `src/config.rs` for details.

#### Pool Configuration (TOML)

```toml
listen_address = "127.0.0.1:34254"
tp_address = "127.0.0.1:8442"  # Template Provider

[ehash]
enabled = true
mint_url = "http://127.0.0.1:3338"
min_leading_zeros = 32
conversion_rate = 1000000
default_locking_pubkey = "hpub1qyq2fw8qdwmhzgfzecvl5a3jyy8v8lf7wj8rfxp8sxvh7vxqzqfxl6yw"
```

#### TProxy Configuration (TOML)

```toml
upstream_address = "127.0.0.1:34254"
upstream_port = 34254
listening_address = "127.0.0.1:34255"
listening_port = 34255

[ehash_wallet]
enabled = true
default_locking_pubkey = "hpub1qyq2fw8qdwmhzgfzecvl5a3jyy8v8lf7wj8rfxp8sxvh7vxqzqfxl6yw"
```

#### JDC Configuration (TOML)

```toml
listen_mining_address = "127.0.0.1:34260"
listen_mining_port = 34260
jds_address = "127.0.0.1:34264"
upstream_address = "127.0.0.1:34254"
upstream_port = 34254

[ehash]
enabled = true  # Mint mode (false = Wallet mode)
mint_url = "http://127.0.0.1:3339"
min_leading_zeros = 32
conversion_rate = 1000000
default_locking_pubkey = "hpub1qyq2fw8qdwmhzgfzecvl5a3jyy8v8lf7wj8rfxp8sxvh7vxqzqfxl6yw"
```

#### JDS Configuration (TOML)

```toml
listen_address = "127.0.0.1:34264"
listen_port = 34264
tp_address = "127.0.0.1:8442"
```

## Test Directory Structure

```
/tmp/ehashimint-<PID>/
├── configs/          # Generated TOML configurations
│   ├── pool.toml
│   ├── tproxy.toml
│   ├── jdc.toml
│   └── jds.toml
├── logs/            # Service logs
│   ├── pool.log
│   ├── tproxy.log
│   ├── jdc.log
│   ├── jds.log
│   └── miner.log
├── pids/            # Process ID files
│   ├── pool.pid
│   ├── tproxy.pid
│   └── ...
└── dbs/             # eHash mint databases
    ├── ehash_mint.db
    └── jdc_ehash_mint.db
```

## Monitoring

### Check Process Status

```bash
ehashimint status
```

Output:
```
Process Status:
═══════════════════════════════════════════════════════
pool                 ✓ Running (PID: 12345)
tproxy               ✓ Running (PID: 12346)
miner                ✓ Running (PID: 12347)
```

### View Logs

```bash
# Follow Pool logs
tail -f /tmp/ehashimint-<PID>/logs/pool.log

# Check for eHash minting events
grep "MintQuote" /tmp/ehashimint-<PID>/logs/pool.log
```

### Query Mint Database

```bash
sqlite3 /tmp/ehashimint-<PID>/dbs/ehash_mint.db \
  "SELECT quote_id, amount, unit, state FROM mint_quote LIMIT 10;"
```

## Comparison to Shell Scripts

### Before (scripts/ehash-testing)

```bash
scripts/ehash-testing/
├── 1-tproxy-pool-mint/
│   ├── run.sh           # Bash script
│   ├── pool.toml        # Static config
│   └── tproxy.toml      # Static config
├── 2-tproxy-jdc-mint/
│   └── ...
└── 3-jdc-pool-mint/
    └── ...
```

**Issues:**
- Manual process management (background jobs, PID tracking)
- No cross-platform support (shell-specific)
- Limited error handling
- Duplicated setup code
- Hard to extend or customize

### After (ehashimint)

```bash
# Single command, automatic setup
ehashimint tproxy-pool --with-miner

# Or with custom configs
ehashimint tproxy-pool --pool-config my-pool.toml
```

**Benefits:**
- ✅ Rust-based: type-safe, cross-platform
- ✅ Automatic config generation
- ✅ Proper process lifecycle management
- ✅ Status monitoring and cleanup
- ✅ Single binary, no external dependencies
- ✅ Extensible architecture (add new scenarios easily)

## Architecture

### Module Structure

```
ehashimint/
├── src/
│   ├── main.rs          # CLI entry point
│   ├── cli.rs           # Command implementations (status, stop, clean)
│   ├── config.rs        # Configuration types and defaults
│   ├── process.rs       # Process management (spawn, kill, monitor)
│   └── scenarios/       # Testing scenarios
│       ├── mod.rs
│       ├── tproxy_pool.rs
│       ├── tproxy_jdc.rs
│       └── jdc_pool.rs
└── Cargo.toml
```

### Process Manager

`ProcessManager` handles:
- Spawning processes with redirected stdout/stderr
- Writing PID files for status tracking
- Graceful shutdown (SIGTERM)
- Process status monitoring
- Automatic cleanup on drop

### Scenario Pattern

Each scenario:
1. Creates test directory structure
2. Generates or loads configurations
3. Spawns services in correct order
4. Optionally starts CPU miner
5. Displays connection details and logs
6. Waits for Ctrl+C (services continue in background)

## Troubleshooting

### Binaries Not Found

```
Error: Failed to spawn pool_sv2
```

**Solution**: Build binaries first:
```bash
cargo build --release -p pool_sv2 -p translator_sv2 -p jd_server -p jd_client
```

### Port Already in Use

```
Error: Address already in use
```

**Solution**: Stop existing processes or use custom configs with different ports:
```bash
ehashimint stop
# Or customize ports in config files
```

### Services Not Starting

```bash
# Check logs for errors
ls /tmp/ehashimint-*/logs/
tail -f /tmp/ehashimint-*/logs/pool.log
```

### Template Provider Not Running

```
Error: Failed to connect to Template Provider
```

**Solution**: Start bitcoind or template_provider:
```bash
bitcoind -regtest -server -rpcuser=user -rpcpassword=pass
# Or
cargo run --release -p template_provider
```

## Development

### Adding a New Scenario

1. Create scenario module in `src/scenarios/my_scenario.rs`
2. Implement `run()` function following existing patterns
3. Add command variant in `src/main.rs`
4. Update documentation

### Running Tests

```bash
cargo test
```

### Code Structure Principles

- **Separation of concerns**: CLI, config, process management, scenarios
- **Configuration as code**: Type-safe TOML configs with serde
- **Process lifecycle**: Proper spawn, monitor, cleanup
- **Error handling**: anyhow for ergonomic error propagation
- **Logging**: tracing for structured logging

## Contributing

When adding features:
1. Follow existing code patterns
2. Update documentation
3. Test all three scenarios
4. Ensure proper cleanup (no orphaned processes)

## License

Same as Stratum V2 Reference Implementation (MIT OR Apache-2.0)

## See Also

- [eHash Specification](../../.kiro/specs/hashpool-ehash-mint/)
- [NUT-04: Mint Quotes](https://github.com/cashubtc/nuts/blob/main/04.md)
- [NUT-20: P2PK Authentication](https://github.com/cashubtc/nuts/blob/main/20.md)
- [Fedimint devimint](https://github.com/fedimint/fedimint/tree/master/devimint)
