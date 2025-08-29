//! Trait interface for share accounting storage backends.

use async_trait::async_trait;
use bitcoin::hashes::sha256d::Hash;

use crate::{
    error::StorageResult,
    types::{
        BatchAcknowledgmentRecord, BlockRecord, ShareAccountingData, ShareRecord,
    },
};

/// Trait defining the interface for share accounting storage backends.
/// 
/// This trait abstracts the persistence layer for share accounting data,
/// allowing different storage implementations (SQLite, RocksDB, PostgreSQL, etc.)
/// to be used interchangeably.
#[async_trait]
pub trait ShareAccountingStorage: Send + Sync {
    /// Initialize the storage backend (create tables, directories, etc.)
    async fn initialize(&mut self) -> StorageResult<()>;

    /// Close the storage backend and clean up resources
    async fn close(&mut self) -> StorageResult<()>;

    // === Share Accounting Data Operations ===

    /// Store or update share accounting data for a channel
    async fn store_share_accounting(
        &mut self,
        data: &ShareAccountingData,
    ) -> StorageResult<()>;

    /// Retrieve share accounting data for a channel
    async fn get_share_accounting(
        &self,
        channel_id: &str,
    ) -> StorageResult<Option<ShareAccountingData>>;

    /// Delete share accounting data for a channel
    async fn delete_share_accounting(&mut self, channel_id: &str) -> StorageResult<()>;

    /// List all channel IDs with stored accounting data
    async fn list_channels(&self) -> StorageResult<Vec<String>>;

    // === Share Record Operations ===

    /// Store a share submission record
    async fn store_share_record(&mut self, record: &ShareRecord) -> StorageResult<()>;

    /// Retrieve share records for a channel within a time range
    async fn get_share_records(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<ShareRecord>>;

    /// Check if a share hash has been seen (for duplicate detection)
    async fn is_share_seen(&self, channel_id: &str, share_hash: &Hash) -> StorageResult<bool>;

    /// Remove old share records older than the specified timestamp
    async fn cleanup_old_shares(&mut self, older_than: u64) -> StorageResult<usize>;

    // === Block Discovery Operations ===

    /// Store a block discovery record
    async fn store_block_record(&mut self, record: &BlockRecord) -> StorageResult<()>;

    /// Retrieve block records within a time range
    async fn get_block_records(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<BlockRecord>>;

    /// Get block records for a specific channel
    async fn get_channel_blocks(&self, channel_id: &str) -> StorageResult<Vec<BlockRecord>>;

    // === Batch Acknowledgment Operations ===

    /// Store a batch acknowledgment record
    async fn store_batch_acknowledgment(
        &mut self,
        record: &BatchAcknowledgmentRecord,
    ) -> StorageResult<()>;

    /// Get batch acknowledgment records for a channel
    async fn get_batch_acknowledgments(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<BatchAcknowledgmentRecord>>;

    // === Analytics and Statistics ===

    /// Get total shares submitted across all channels within a time range
    async fn get_total_shares(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<u64>;

    /// Get total work contributed across all channels within a time range
    async fn get_total_work(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<u64>;

    /// Get channel-specific statistics
    async fn get_channel_stats(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<ChannelStats>;

    /// Health check for the storage backend
    async fn health_check(&self) -> StorageResult<StorageHealth>;
}

/// Channel-specific statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel_id: String,
    pub total_shares: u64,
    pub accepted_shares: u64,
    pub rejected_shares: u64,
    pub total_work: u64,
    pub blocks_found: u64,
    pub best_difficulty: f64,
    pub first_share_timestamp: Option<u64>,
    pub last_share_timestamp: Option<u64>,
}

/// Storage backend health information
#[derive(Debug, Clone)]
pub struct StorageHealth {
    pub is_healthy: bool,
    pub backend_type: String,
    pub connection_status: String,
    pub last_operation_timestamp: Option<u64>,
    pub error_message: Option<String>,
}