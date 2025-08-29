//! In-memory storage backend for testing and development.
//!
//! This backend stores all data in memory and is useful for:
//! - Testing and development
//! - Temporary storage scenarios
//! - High-performance scenarios where persistence isn't required

use async_trait::async_trait;
use bitcoin::hashes::sha256d::Hash;
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

use crate::{
    error::StorageResult,
    share_accounting_storage::{ChannelStats, ShareAccountingStorage, StorageHealth},
    types::{
        BatchAcknowledgmentRecord, BlockRecord, ShareAccountingData, ShareRecord,
    },
};

/// In-memory implementation of ShareAccountingStorage.
/// 
/// Data is stored in memory using HashMap collections protected by RwLock
/// for thread-safe concurrent access. All data is lost when the process stops.
pub struct MemoryStorage {
    share_accounting: RwLock<HashMap<String, ShareAccountingData>>,
    share_records: RwLock<HashMap<String, Vec<ShareRecord>>>,
    block_records: RwLock<Vec<BlockRecord>>,
    batch_records: RwLock<HashMap<String, Vec<BatchAcknowledgmentRecord>>>,
    seen_shares: RwLock<HashMap<String, HashSet<Hash>>>,
    last_operation_timestamp: RwLock<Option<u64>>,
}

impl MemoryStorage {
    /// Create a new MemoryStorage instance
    pub fn new() -> Self {
        Self {
            share_accounting: RwLock::new(HashMap::new()),
            share_records: RwLock::new(HashMap::new()),
            block_records: RwLock::new(Vec::new()),
            batch_records: RwLock::new(HashMap::new()),
            seen_shares: RwLock::new(HashMap::new()),
            last_operation_timestamp: RwLock::new(None),
        }
    }

    async fn update_last_operation_timestamp(&self) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        *self.last_operation_timestamp.write().await = Some(timestamp);
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ShareAccountingStorage for MemoryStorage {
    async fn initialize(&mut self) -> StorageResult<()> {
        tracing::info!("Initializing memory storage backend");
        self.update_last_operation_timestamp().await;
        Ok(())
    }

    async fn close(&mut self) -> StorageResult<()> {
        tracing::info!("Closing memory storage backend");
        // Clear all data on close
        self.share_accounting.write().await.clear();
        self.share_records.write().await.clear();
        self.block_records.write().await.clear();
        self.batch_records.write().await.clear();
        self.seen_shares.write().await.clear();
        Ok(())
    }

    async fn store_share_accounting(&mut self, data: &ShareAccountingData) -> StorageResult<()> {
        self.update_last_operation_timestamp().await;
        let mut accounting = self.share_accounting.write().await;
        accounting.insert(data.channel_id.clone(), data.clone());
        Ok(())
    }

    async fn get_share_accounting(
        &self,
        channel_id: &str,
    ) -> StorageResult<Option<ShareAccountingData>> {
        let accounting = self.share_accounting.read().await;
        Ok(accounting.get(channel_id).cloned())
    }

    async fn delete_share_accounting(&mut self, channel_id: &str) -> StorageResult<()> {
        self.update_last_operation_timestamp().await;
        let mut accounting = self.share_accounting.write().await;
        accounting.remove(channel_id);
        Ok(())
    }

    async fn list_channels(&self) -> StorageResult<Vec<String>> {
        let accounting = self.share_accounting.read().await;
        Ok(accounting.keys().cloned().collect())
    }

    async fn store_share_record(&mut self, record: &ShareRecord) -> StorageResult<()> {
        self.update_last_operation_timestamp().await;
        
        let mut records = self.share_records.write().await;
        records
            .entry(record.channel_id.clone())
            .or_default()
            .push(record.clone());

        // Update seen shares if the share was accepted
        if record.accepted {
            let mut seen = self.seen_shares.write().await;
            seen.entry(record.channel_id.clone())
                .or_default()
                .insert(record.share_hash);
        }

        Ok(())
    }

    async fn get_share_records(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<ShareRecord>> {
        let records = self.share_records.read().await;
        let empty_vec = Vec::new();
        let channel_records = records.get(channel_id).unwrap_or(&empty_vec);

        let mut filtered: Vec<ShareRecord> = channel_records
            .iter()
            .filter(|record| {
                if let Some(start) = start_timestamp {
                    if record.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_timestamp {
                    if record.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by timestamp (most recent first)
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    async fn is_share_seen(&self, channel_id: &str, share_hash: &Hash) -> StorageResult<bool> {
        let seen = self.seen_shares.read().await;
        Ok(seen
            .get(channel_id)
            .map(|hashes| hashes.contains(share_hash))
            .unwrap_or(false))
    }

    async fn cleanup_old_shares(&mut self, older_than: u64) -> StorageResult<usize> {
        self.update_last_operation_timestamp().await;
        let mut records = self.share_records.write().await;
        let mut total_removed = 0;

        for channel_records in records.values_mut() {
            let original_len = channel_records.len();
            channel_records.retain(|record| record.timestamp >= older_than);
            total_removed += original_len - channel_records.len();
        }

        Ok(total_removed)
    }

    async fn store_block_record(&mut self, record: &BlockRecord) -> StorageResult<()> {
        self.update_last_operation_timestamp().await;
        let mut blocks = self.block_records.write().await;
        blocks.push(record.clone());
        Ok(())
    }

    async fn get_block_records(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<BlockRecord>> {
        let blocks = self.block_records.read().await;
        let mut filtered: Vec<BlockRecord> = blocks
            .iter()
            .filter(|record| {
                if let Some(start) = start_timestamp {
                    if record.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_timestamp {
                    if record.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    async fn get_channel_blocks(&self, channel_id: &str) -> StorageResult<Vec<BlockRecord>> {
        let blocks = self.block_records.read().await;
        let mut filtered: Vec<BlockRecord> = blocks
            .iter()
            .filter(|record| record.channel_id == channel_id)
            .cloned()
            .collect();

        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(filtered)
    }

    async fn store_batch_acknowledgment(
        &mut self,
        record: &BatchAcknowledgmentRecord,
    ) -> StorageResult<()> {
        self.update_last_operation_timestamp().await;
        let mut batches = self.batch_records.write().await;
        batches
            .entry(record.channel_id.clone())
            .or_default()
            .push(record.clone());
        Ok(())
    }

    async fn get_batch_acknowledgments(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<BatchAcknowledgmentRecord>> {
        let batches = self.batch_records.read().await;
        let empty_vec = Vec::new();
        let channel_batches = batches.get(channel_id).unwrap_or(&empty_vec);

        let mut filtered: Vec<BatchAcknowledgmentRecord> = channel_batches
            .iter()
            .filter(|record| {
                if let Some(start) = start_timestamp {
                    if record.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_timestamp {
                    if record.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if let Some(limit) = limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    async fn get_total_shares(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<u64> {
        let records = self.share_records.read().await;
        let mut total = 0u64;

        for channel_records in records.values() {
            total += channel_records
                .iter()
                .filter(|record| {
                    if let Some(start) = start_timestamp {
                        if record.timestamp < start {
                            return false;
                        }
                    }
                    if let Some(end) = end_timestamp {
                        if record.timestamp > end {
                            return false;
                        }
                    }
                    true
                })
                .count() as u64;
        }

        Ok(total)
    }

    async fn get_total_work(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<u64> {
        let records = self.share_records.read().await;
        let mut total_work = 0u64;

        for channel_records in records.values() {
            total_work += channel_records
                .iter()
                .filter(|record| {
                    if !record.accepted {
                        return false;
                    }
                    if let Some(start) = start_timestamp {
                        if record.timestamp < start {
                            return false;
                        }
                    }
                    if let Some(end) = end_timestamp {
                        if record.timestamp > end {
                            return false;
                        }
                    }
                    true
                })
                .map(|record| record.share_work)
                .sum::<u64>();
        }

        Ok(total_work)
    }

    async fn get_channel_stats(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<ChannelStats> {
        let records = self.share_records.read().await;
        let empty_vec = Vec::new();
        let channel_records = records.get(channel_id).unwrap_or(&empty_vec);
        
        let filtered_records: Vec<&ShareRecord> = channel_records
            .iter()
            .filter(|record| {
                if let Some(start) = start_timestamp {
                    if record.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_timestamp {
                    if record.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .collect();

        let total_shares = filtered_records.len() as u64;
        let accepted_shares = filtered_records.iter().filter(|r| r.accepted).count() as u64;
        let rejected_shares = total_shares - accepted_shares;
        
        let total_work = filtered_records
            .iter()
            .filter(|r| r.accepted)
            .map(|r| r.share_work)
            .sum::<u64>();

        let best_difficulty = filtered_records
            .iter()
            .map(|r| r.difficulty)
            .fold(0.0, f64::max);

        let blocks_found = {
            let blocks = self.block_records.read().await;
            blocks.iter().filter(|b| {
                b.channel_id == channel_id &&
                start_timestamp.map_or(true, |start| b.timestamp >= start) &&
                end_timestamp.map_or(true, |end| b.timestamp <= end)
            }).count() as u64
        };

        let first_share_timestamp = filtered_records.iter().map(|r| r.timestamp).min();
        let last_share_timestamp = filtered_records.iter().map(|r| r.timestamp).max();

        Ok(ChannelStats {
            channel_id: channel_id.to_string(),
            total_shares,
            accepted_shares,
            rejected_shares,
            total_work,
            blocks_found,
            best_difficulty,
            first_share_timestamp,
            last_share_timestamp,
        })
    }

    async fn health_check(&self) -> StorageResult<StorageHealth> {
        let last_operation = *self.last_operation_timestamp.read().await;
        
        Ok(StorageHealth {
            is_healthy: true,
            backend_type: "memory".to_string(),
            connection_status: "connected".to_string(),
            last_operation_timestamp: last_operation,
            error_message: None,
        })
    }
}