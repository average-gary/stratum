# Stratum V2 Storage Role

A storage role for persisting share accounting and validation data in Stratum V2 mining operations.

## Overview

The Storage role provides a trait-based interface for persisting share accounting data, validation results, and mining statistics. It abstracts the storage backend, allowing different implementations (in-memory, SQLite, RocksDB, etc.) to be used interchangeably.

## Features

- **Trait-based Architecture**: Clean abstraction layer for different storage backends
- **Share Accounting Persistence**: Store and retrieve per-channel share statistics
- **Historical Data**: Track share submissions, block discoveries, and batch acknowledgments
- **Duplicate Detection**: Persist share hashes for duplicate detection across restarts
- **Analytics**: Generate channel-specific and global mining statistics
- **Multiple Backends**: Support for memory, SQLite, RocksDB, and custom implementations

## Data Types

### Core Data Structures

- `ShareAccountingData`: Per-channel share accounting statistics
- `ShareRecord`: Individual share submission records
- `BlockRecord`: Block discovery events
- `BatchAcknowledgmentRecord`: Batch processing acknowledgments
- `ChannelStats`: Analytics and statistics for channels

### Storage Interface

The `ShareAccountingStorage` trait provides:

```rust
#[async_trait]
pub trait ShareAccountingStorage: Send + Sync {
    // Lifecycle management
    async fn initialize(&mut self) -> StorageResult<()>;
    async fn close(&mut self) -> StorageResult<()>;
    
    // Share accounting operations
    async fn store_share_accounting(&mut self, data: &ShareAccountingData) -> StorageResult<()>;
    async fn get_share_accounting(&self, channel_id: &str) -> StorageResult<Option<ShareAccountingData>>;
    
    // Share record operations
    async fn store_share_record(&mut self, record: &ShareRecord) -> StorageResult<()>;
    async fn is_share_seen(&self, channel_id: &str, share_hash: &Hash) -> StorageResult<bool>;
    
    // Analytics
    async fn get_channel_stats(&self, channel_id: &str, start_timestamp: Option<u64>, end_timestamp: Option<u64>) -> StorageResult<ChannelStats>;
    async fn health_check(&self) -> StorageResult<StorageHealth>;
    
    // ... and more
}
```

## Available Backends

### Memory Backend (Default)

```rust
use storage_sv2::backends::memory::MemoryStorage;

let mut storage = MemoryStorage::new();
storage.initialize().await?;
```

**Features:**
- Fast in-memory storage
- Perfect for testing and development
- No persistence (data lost on restart)
- Thread-safe with RwLock protection

### SQLite Backend (Feature: `sqlite-backend`)

```toml
[dependencies]
storage_sv2 = { path = "../storage", features = ["sqlite-backend"] }
```

### RocksDB Backend (Feature: `rocksdb-backend`)

```toml
[dependencies]
storage_sv2 = { path = "../storage", features = ["rocksdb-backend"] }
```

## Integration with Other Roles

### Pool Role Integration

```rust
use storage_sv2::{ShareAccountingStorage, types::*};

// In your pool's share validation logic
async fn on_share_accepted(
    storage: &mut dyn ShareAccountingStorage,
    channel_id: &str,
    share_hash: Hash,
    share_work: u64,
    sequence_number: u32,
) -> Result<(), StorageError> {
    // Store the share record
    let share_record = ShareRecord {
        id: format!("{}_{}", channel_id, sequence_number),
        channel_id: channel_id.to_string(),
        share_hash,
        sequence_number,
        share_work,
        difficulty: calculate_difficulty(share_work),
        timestamp: current_timestamp(),
        accepted: true,
        validation_result: Some(ShareValidationOutcome::Valid),
    };
    
    storage.store_share_record(&share_record).await?;
    
    // Update accounting data
    let mut accounting = storage.get_share_accounting(channel_id).await?
        .unwrap_or_else(|| ShareAccountingData::new(channel_id));
    
    accounting.shares_accepted += 1;
    accounting.share_work_sum += share_work;
    accounting.last_share_sequence_number = sequence_number;
    accounting.last_updated = current_timestamp();
    
    storage.store_share_accounting(&accounting).await?;
    
    Ok(())
}
```

## Running the Storage Role

### Basic Usage

```bash
# Using memory backend (default)
cargo run --bin storage-sv2

# Using SQLite backend
cargo run --bin storage-sv2 --features sqlite-backend -- --backend sqlite

# With configuration file
cargo run --bin storage-sv2 -- --config storage-config.toml
```

### Configuration

Example `storage-config.toml`:

```toml
[storage]
backend = "sqlite"
database_path = "./stratum_storage.db"
cleanup_interval_hours = 24
max_share_history_days = 30

[logging]
level = "info"
file = "./logs/storage.log"
```

## Architecture Benefits

1. **Abstraction**: Other roles interact through the trait, not specific implementations
2. **Flexibility**: Easy to switch backends or add new storage types
3. **Testing**: Memory backend enables fast, isolated testing
4. **Scalability**: Can implement distributed storage backends
5. **Recovery**: Persistent backends enable recovery after crashes/restarts

## Development

### Adding a New Backend

1. Create a new module in `src/lib/backends/`
2. Implement the `ShareAccountingStorage` trait
3. Add feature flag in `Cargo.toml`
4. Update `backends/mod.rs` to include the new backend

### Testing

```bash
# Run tests with default (memory) backend
cargo test

# Run tests with specific backend features
cargo test --features sqlite-backend
cargo test --features rocksdb-backend
```

## Use Cases

- **Share Accounting Recovery**: Restore share statistics after pool restarts
- **Duplicate Detection**: Persist share hashes to detect duplicates across sessions
- **Mining Analytics**: Generate reports on channel performance and block discoveries
- **Audit Trail**: Maintain historical records of all share submissions
- **Load Balancing**: Track channel statistics for intelligent work distribution