# Interactive eHash Tutorial

An interactive terminal-based tutorial that guides you through setting up and using the eHash protocol with Stratum v2 mining.

## Quick Start

### 1. First-Time Setup

From the eHash repository root:

```bash
cd ~/code/ehash  # or wherever your ehash repo is

# Run the one-time setup (initializes submodules and builds everything)
just setup
```

This will:
- Initialize git submodules (cdk, etc.)
- Build all required binaries (pool_sv2, translator_sv2, mining_device)
- Takes 5-15 minutes on first run

### 2. Run the Tutorial

```bash
just tutorial
```

Or run directly:

```bash
cd test-utils/ehash-tutorial
cargo run
```

## What You'll Learn

The tutorial guides you through:

1. **Setup** - Building the required Stratum v2 binaries
2. **Pool Operator** - Setting up a mining pool with eHash minting
3. **Proxy Operator** - Configuring a translation proxy with eHash support
4. **Pioneer** - Mining and earning eHash tokens

## Tutorial Features

### Interactive CLI Commands

Type real production commands in a safe, guided environment:
- `cargo build -p pool_sv2 -p translator_sv2` - Build binaries
- `pool_sv2 --config pool-config-ehash.toml` - Start the pool
- `cdk-cli wallet create --name my-wallet` - Create wallets
- `mining_device --pool-address 127.0.0.1:34255` - Start mining

### Smart Input Assistance

- **→ (Right arrow)** - Auto-complete suggested commands
- **Tab** - Show available completions
- **↑↓** - Navigate command history or scroll messages
- **Esc** - Clear messages and return to normal mode

### Real-Time Feedback

- See actual cargo build output
- Monitor running processes with PID tracking
- View logs from started services
- Get helpful error messages with debugging info

## Keyboard Shortcuts

### Normal Mode
- `→` - Accept placeholder suggestion
- `Tab` - Show completions
- `↑↓` - Navigate command history
- `Enter` - Execute command
- `Ctrl+C` - Quit tutorial

### Scroll Mode (when viewing long messages)
- `↑↓` - Scroll message (1 line at a time)
- `PgUp/PgDn` - Scroll faster (5 lines at a time)
- `Esc` - Exit scroll mode
- `c` - Clear message

## Troubleshooting

### "Binary not found" errors

The tutorial looks for binaries in your PATH or in standard locations. If you see errors like:

```
❌ pool_sv2 binary not found!
```

Run the setup:
```bash
cd ~/code/ehash
just setup
```

### Submodule errors

If you see errors about missing dependencies:

```bash
git submodule update --init --recursive
cargo build --workspace
```

### Build failures in tutorial

The tutorial runs `cargo build` in the `roles/` directory. Make sure:
- You're in the correct ehash repository
- Submodules are initialized
- You have a working Rust toolchain

## Manual Setup (without just)

If you don't have `just` installed:

```bash
cd ~/code/ehash

# Initialize submodules
git submodule update --init --recursive

# Build everything
cargo build --workspace

# Run tutorial
cd test-utils/ehash-tutorial
cargo run
```

## Requirements

- Rust 1.70+ (or whatever version the project uses)
- Git (for submodules)
- Linux or macOS (Windows support untested)
- Terminal with UTF-8 support

## Architecture

The tutorial is a TUI (Terminal User Interface) application built with:
- **ratatui** - Terminal UI framework
- **crossterm** - Cross-platform terminal manipulation
- **ehashimint** - Process management for Stratum v2 services

It creates a sandboxed test environment in `/tmp/ehash-tutorial-{PID}` where:
- Configs are generated automatically
- Processes run in isolation
- Logs are captured for viewing

## Development

Build the tutorial:
```bash
cd test-utils/ehash-tutorial
cargo build
```

Run tests:
```bash
cargo test
```

## License

Same as the parent eHash project.
