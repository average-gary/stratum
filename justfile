# eHash Project Commands

# Setup: Initialize submodules and build everything for the tutorial
setup:
    @echo "Syncing submodule URLs..."
    git submodule sync
    @echo "Initializing git submodules..."
    git submodule update --init --recursive
    @echo "Building all workspace packages..."
    cargo build --workspace
    @echo "✓ Setup complete! Run 'just tutorial' to start."

# Run the interactive eHash tutorial
tutorial:
    cd test-utils/ehash-tutorial && cargo run

# Build and run the tutorial in release mode (faster, but longer compile)
tutorial-release:
    cd test-utils/ehash-tutorial && cargo run --release

# Build the tutorial
tutorial-build:
    cd test-utils/ehash-tutorial && cargo build

# Build Stratum v2 binaries for the tutorial
build-stratum:
    cd roles && cargo build -p pool_sv2 -p translator_sv2

# Build mining device for the tutorial
build-miner:
    cd roles/test-utils/mining-device && cargo build

# Build the entire workspace
build-all:
    cargo build --workspace

# Full rebuild in release mode
build-all-release:
    cargo build --workspace --release

# Clean up tutorial test directories and processes
clean-tutorial:
    #!/usr/bin/env bash
    set +e
    echo "Cleaning tutorial test directories..."
    rm -rf /tmp/ehash-tutorial-* 2>/dev/null
    echo "Stopping any running tutorial processes..."
    pkill -f "pool_sv2.*pool-config-ehash" 2>/dev/null
    pkill -f "translator_sv2.*tproxy-config-ehash" 2>/dev/null
    pkill -f "mining_device" 2>/dev/null
    echo "✓ Tutorial environment cleaned!"

# Reset repository for tutorial testing (clean + remove generated files)
reset-tutorial: clean-tutorial
    #!/usr/bin/env bash
    set +e
    echo "Removing tutorial-generated files..."
    rm -f test-utils/ehash-tutorial/pool-config-ehash.toml 2>/dev/null
    rm -f test-utils/ehash-tutorial/tproxy-config-ehash.toml 2>/dev/null
    echo "Cleaning cargo build artifacts..."
    cd test-utils/ehash-tutorial && cargo clean 2>/dev/null
    echo "✓ Repository reset for tutorial testing!"
    echo ""
    echo "You can now run: just tutorial"

# Reset tutorial AND remove built binaries (for testing "no binaries" flow)
reset-tutorial-full: clean-tutorial
    #!/usr/bin/env bash
    set +e
    echo "Removing tutorial-generated files..."
    rm -f test-utils/ehash-tutorial/pool-config-ehash.toml 2>/dev/null
    rm -f test-utils/ehash-tutorial/tproxy-config-ehash.toml 2>/dev/null
    echo "Removing built binaries from workspace..."
    rm -f roles/target/debug/pool_sv2 2>/dev/null
    rm -f roles/target/debug/translator_sv2 2>/dev/null
    rm -f roles/target/debug/mining_device 2>/dev/null
    rm -f roles/target/release/pool_sv2 2>/dev/null
    rm -f roles/target/release/translator_sv2 2>/dev/null
    rm -f roles/target/release/mining_device 2>/dev/null
    rm -f roles/test-utils/mining-device/target/debug/mining_device 2>/dev/null
    rm -f roles/test-utils/mining-device/target/release/mining_device 2>/dev/null
    echo "Cleaning tutorial build artifacts..."
    cd test-utils/ehash-tutorial && cargo clean 2>/dev/null
    echo "✓ Full tutorial reset complete!"
    echo ""
    echo "Tutorial will show 'binaries not built' on next run."
    echo "Run: just tutorial"
