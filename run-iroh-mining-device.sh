#!/bin/bash
# Helper script to run mining device connecting to Pool over Iroh
#
# Usage: ./run-iroh-mining-device.sh <POOL_NODE_ID>
#
# Example:
#   ./run-iroh-mining-device.sh f6cbfdb5def056fd1814dedcc0f0dd2b982919cbb75c76612aa9bcc404eae1a8

set -e

if [ -z "$1" ]; then
    echo "Error: Pool NodeId required"
    echo "Usage: $0 <POOL_NODE_ID>"
    echo ""
    echo "You can find the Pool's NodeId in the Pool startup logs:"
    echo "  Look for: 'Pool Iroh listener initialized. NodeId: <NODE_ID>'"
    exit 1
fi

POOL_NODE_ID="$1"

echo "Starting mining device with Iroh transport..."
echo "Pool NodeId: $POOL_NODE_ID"
echo ""

cargo run --manifest-path=roles/test-utils/mining-device/Cargo.toml --features iroh -- \
    --pool-iroh-node-id "$POOL_NODE_ID" \
    --pool-iroh-alpn "sv2-m" \
    --iroh-secret-key-path ./mining-device-iroh-secret.key \
    --pubkey-pool 9bDuixKmZqAJnrmP746n8zU1wyAQRrus7th9dxnkPg6RzQvCnan \
    --handicap 1000
