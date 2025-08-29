//! SQLite storage backend implementation.
//!
//! This backend provides persistent storage using SQLite database.
//! Features:
//! - ACID transactions
//! - SQL queries for analytics
//! - File-based persistence
//! - Embedded database (no separate server needed)

use async_trait::async_trait;
use bitcoin::hashes::{sha256d::Hash, Hash as HashTrait};
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};
use std::{path::Path, str::FromStr, time::{SystemTime, UNIX_EPOCH}};

use crate::{
    error::{StorageError, StorageResult},
    share_accounting_storage::{ChannelStats, ShareAccountingStorage, StorageHealth},
    types::{
        BatchAcknowledgmentRecord, BlockRecord, ShareAccountingData, ShareRecord,
    },
};

/// SQLite implementation of ShareAccountingStorage.
/// 
/// Uses SQLite for persistent storage with full ACID guarantees.
/// Suitable for production use in single-node deployments.
pub struct SqliteStorage {
    pool: Option<SqlitePool>,
    database_path: String,
}

impl SqliteStorage {
    /// Create a new SqliteStorage instance
    pub fn new(database_path: impl Into<String>) -> Self {
        Self {
            pool: None,
            database_path: database_path.into(),
        }
    }

    /// Get database connection pool
    fn get_pool(&self) -> StorageResult<&SqlitePool> {
        self.pool.as_ref().ok_or(StorageError::BackendUnavailable)
    }

    /// Initialize database schema
    async fn create_tables(&self) -> StorageResult<()> {
        let pool = self.get_pool()?;

        // Share accounting table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS share_accounting (
                channel_id TEXT PRIMARY KEY,
                last_share_sequence_number INTEGER NOT NULL,
                shares_accepted INTEGER NOT NULL,
                share_work_sum INTEGER NOT NULL,
                best_diff REAL NOT NULL,
                last_updated INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        // Share records table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS share_records (
                id TEXT PRIMARY KEY,
                channel_id TEXT NOT NULL,
                share_hash BLOB NOT NULL,
                sequence_number INTEGER NOT NULL,
                share_work INTEGER NOT NULL,
                difficulty REAL NOT NULL,
                timestamp INTEGER NOT NULL,
                accepted BOOLEAN NOT NULL,
                validation_result TEXT,
                FOREIGN KEY (channel_id) REFERENCES share_accounting(channel_id)
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        // Block records table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS block_records (
                id TEXT PRIMARY KEY,
                channel_id TEXT NOT NULL,
                share_hash BLOB NOT NULL,
                template_id INTEGER,
                coinbase BLOB NOT NULL,
                difficulty REAL NOT NULL,
                timestamp INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        // Batch acknowledgment records table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS batch_acknowledgments (
                id TEXT PRIMARY KEY,
                channel_id TEXT NOT NULL,
                last_sequence_number INTEGER NOT NULL,
                new_submits_accepted_count INTEGER NOT NULL,
                new_shares_sum INTEGER NOT NULL,
                timestamp INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        // Create indexes for performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_share_records_channel_timestamp ON share_records(channel_id, timestamp)")
            .execute(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_share_records_hash ON share_records(channel_id, share_hash)")
            .execute(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_block_records_timestamp ON block_records(timestamp)")
            .execute(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(())
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[async_trait]
impl ShareAccountingStorage for SqliteStorage {
    async fn initialize(&mut self) -> StorageResult<()> {
        tracing::info!("Initializing SQLite storage backend at: {}", self.database_path);

        // Create database directory if it doesn't exist
        if let Some(parent) = Path::new(&self.database_path).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| StorageError::BackendError(format!("Failed to create database directory: {}", e)))?;
        }

        // Connect to database
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", self.database_path))
            .map_err(|e| StorageError::ConfigError(e.to_string()))?
            .create_if_missing(true);

        self.pool = Some(
            SqlitePool::connect_with(options)
                .await
                .map_err(|e| StorageError::BackendError(e.to_string()))?,
        );

        // Create tables
        self.create_tables().await?;

        tracing::info!("SQLite storage initialized successfully");
        Ok(())
    }

    async fn close(&mut self) -> StorageResult<()> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
            tracing::info!("SQLite storage closed");
        }
        Ok(())
    }

    async fn store_share_accounting(&mut self, data: &ShareAccountingData) -> StorageResult<()> {
        let pool = self.get_pool()?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO share_accounting 
            (channel_id, last_share_sequence_number, shares_accepted, share_work_sum, best_diff, last_updated)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&data.channel_id)
        .bind(data.last_share_sequence_number as i64)
        .bind(data.shares_accepted as i64)
        .bind(data.share_work_sum as i64)
        .bind(data.best_diff)
        .bind(data.last_updated as i64)
        .execute(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(())
    }

    async fn get_share_accounting(&self, channel_id: &str) -> StorageResult<Option<ShareAccountingData>> {
        let pool = self.get_pool()?;

        let row = sqlx::query(
            "SELECT channel_id, last_share_sequence_number, shares_accepted, share_work_sum, best_diff, last_updated 
             FROM share_accounting WHERE channel_id = ?1"
        )
        .bind(channel_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        if let Some(row) = row {
            Ok(Some(ShareAccountingData {
                channel_id: row.get("channel_id"),
                last_share_sequence_number: row.get::<i64, _>("last_share_sequence_number") as u32,
                shares_accepted: row.get::<i64, _>("shares_accepted") as u32,
                share_work_sum: row.get::<i64, _>("share_work_sum") as u64,
                best_diff: row.get("best_diff"),
                last_updated: row.get::<i64, _>("last_updated") as u64,
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_share_accounting(&mut self, channel_id: &str) -> StorageResult<()> {
        let pool = self.get_pool()?;

        sqlx::query("DELETE FROM share_accounting WHERE channel_id = ?1")
            .bind(channel_id)
            .execute(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(())
    }

    async fn list_channels(&self) -> StorageResult<Vec<String>> {
        let pool = self.get_pool()?;

        let rows = sqlx::query("SELECT channel_id FROM share_accounting ORDER BY channel_id")
            .fetch_all(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(rows.into_iter().map(|row| row.get("channel_id")).collect())
    }

    async fn store_share_record(&mut self, record: &ShareRecord) -> StorageResult<()> {
        let pool = self.get_pool()?;

        let validation_result_json = if let Some(ref result) = record.validation_result {
            Some(serde_json::to_string(result)
                .map_err(|e| StorageError::SerializationError(e.to_string()))?)
        } else {
            None
        };

        // Convert Hash to bytes
        let share_hash_bytes = record.share_hash.as_byte_array();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO share_records 
            (id, channel_id, share_hash, sequence_number, share_work, difficulty, timestamp, accepted, validation_result)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&record.id)
        .bind(&record.channel_id)
        .bind(&share_hash_bytes[..])
        .bind(record.sequence_number as i64)
        .bind(record.share_work as i64)
        .bind(record.difficulty)
        .bind(record.timestamp as i64)
        .bind(record.accepted)
        .bind(validation_result_json)
        .execute(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(())
    }

    async fn get_share_records(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<ShareRecord>> {
        let pool = self.get_pool()?;

        let mut query = "SELECT id, channel_id, share_hash, sequence_number, share_work, difficulty, timestamp, accepted, validation_result FROM share_records WHERE channel_id = ?1".to_string();
        let mut bind_count = 1;

        if start_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp >= ?{}", bind_count));
        }
        if end_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp <= ?{}", bind_count));
        }

        query.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = limit {
            bind_count += 1;
            query.push_str(&format!(" LIMIT ?{}", bind_count));
        }

        let mut sql_query = sqlx::query(&query).bind(channel_id);

        if let Some(start) = start_timestamp {
            sql_query = sql_query.bind(start as i64);
        }
        if let Some(end) = end_timestamp {
            sql_query = sql_query.bind(end as i64);
        }
        if let Some(limit) = limit {
            sql_query = sql_query.bind(limit as i64);
        }

        let rows = sql_query
            .fetch_all(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            let share_hash_bytes: Vec<u8> = row.get("share_hash");
            let share_hash_array: [u8; 32] = share_hash_bytes.try_into()
                .map_err(|_| StorageError::InvalidData("Invalid share hash length".to_string()))?;
            let share_hash = Hash::from_byte_array(share_hash_array);

            let validation_result = if let Some(json_str) = row.get::<Option<String>, _>("validation_result") {
                Some(serde_json::from_str(&json_str)
                    .map_err(|e| StorageError::SerializationError(e.to_string()))?)
            } else {
                None
            };

            records.push(ShareRecord {
                id: row.get("id"),
                channel_id: row.get("channel_id"),
                share_hash,
                sequence_number: row.get::<i64, _>("sequence_number") as u32,
                share_work: row.get::<i64, _>("share_work") as u64,
                difficulty: row.get("difficulty"),
                timestamp: row.get::<i64, _>("timestamp") as u64,
                accepted: row.get("accepted"),
                validation_result,
            });
        }

        Ok(records)
    }

    async fn is_share_seen(&self, channel_id: &str, share_hash: &Hash) -> StorageResult<bool> {
        let pool = self.get_pool()?;
        let share_hash_bytes = share_hash.as_byte_array();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM share_records WHERE channel_id = ?1 AND share_hash = ?2 AND accepted = true"
        )
        .bind(channel_id)
        .bind(&share_hash_bytes[..])
        .fetch_one(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(count > 0)
    }

    async fn cleanup_old_shares(&mut self, older_than: u64) -> StorageResult<usize> {
        let pool = self.get_pool()?;

        let result = sqlx::query("DELETE FROM share_records WHERE timestamp < ?1")
            .bind(older_than as i64)
            .execute(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(result.rows_affected() as usize)
    }

    async fn store_block_record(&mut self, record: &BlockRecord) -> StorageResult<()> {
        let pool = self.get_pool()?;
        let share_hash_bytes = record.share_hash.as_byte_array();

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO block_records 
            (id, channel_id, share_hash, template_id, coinbase, difficulty, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(&record.id)
        .bind(&record.channel_id)
        .bind(&share_hash_bytes[..])
        .bind(record.template_id.map(|id| id as i64))
        .bind(&record.coinbase)
        .bind(record.difficulty)
        .bind(record.timestamp as i64)
        .execute(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(())
    }

    async fn get_block_records(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<BlockRecord>> {
        let pool = self.get_pool()?;

        let mut query = "SELECT id, channel_id, share_hash, template_id, coinbase, difficulty, timestamp FROM block_records WHERE 1=1".to_string();
        let mut bind_count = 0;

        if start_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp >= ?{}", bind_count));
        }
        if end_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp <= ?{}", bind_count));
        }

        query.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = limit {
            bind_count += 1;
            query.push_str(&format!(" LIMIT ?{}", bind_count));
        }

        let mut sql_query = sqlx::query(&query);

        if let Some(start) = start_timestamp {
            sql_query = sql_query.bind(start as i64);
        }
        if let Some(end) = end_timestamp {
            sql_query = sql_query.bind(end as i64);
        }
        if let Some(limit) = limit {
            sql_query = sql_query.bind(limit as i64);
        }

        let rows = sql_query
            .fetch_all(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            let share_hash_bytes: Vec<u8> = row.get("share_hash");
            let share_hash_array: [u8; 32] = share_hash_bytes.try_into()
                .map_err(|_| StorageError::InvalidData("Invalid share hash length".to_string()))?;
            let share_hash = Hash::from_byte_array(share_hash_array);

            records.push(BlockRecord {
                id: row.get("id"),
                channel_id: row.get("channel_id"),
                share_hash,
                template_id: row.get::<Option<i64>, _>("template_id").map(|id| id as u64),
                coinbase: row.get("coinbase"),
                difficulty: row.get("difficulty"),
                timestamp: row.get::<i64, _>("timestamp") as u64,
            });
        }

        Ok(records)
    }

    async fn get_channel_blocks(&self, channel_id: &str) -> StorageResult<Vec<BlockRecord>> {
        let pool = self.get_pool()?;

        let rows = sqlx::query(
            "SELECT id, channel_id, share_hash, template_id, coinbase, difficulty, timestamp 
             FROM block_records WHERE channel_id = ?1 ORDER BY timestamp DESC"
        )
        .bind(channel_id)
        .fetch_all(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            let share_hash_bytes: Vec<u8> = row.get("share_hash");
            let share_hash_array: [u8; 32] = share_hash_bytes.try_into()
                .map_err(|_| StorageError::InvalidData("Invalid share hash length".to_string()))?;
            let share_hash = Hash::from_byte_array(share_hash_array);

            records.push(BlockRecord {
                id: row.get("id"),
                channel_id: row.get("channel_id"),
                share_hash,
                template_id: row.get::<Option<i64>, _>("template_id").map(|id| id as u64),
                coinbase: row.get("coinbase"),
                difficulty: row.get("difficulty"),
                timestamp: row.get::<i64, _>("timestamp") as u64,
            });
        }

        Ok(records)
    }

    async fn store_batch_acknowledgment(&mut self, record: &BatchAcknowledgmentRecord) -> StorageResult<()> {
        let pool = self.get_pool()?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO batch_acknowledgments 
            (id, channel_id, last_sequence_number, new_submits_accepted_count, new_shares_sum, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&record.id)
        .bind(&record.channel_id)
        .bind(record.last_sequence_number as i64)
        .bind(record.new_submits_accepted_count as i64)
        .bind(record.new_shares_sum as i64)
        .bind(record.timestamp as i64)
        .execute(pool)
        .await
        .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(())
    }

    async fn get_batch_acknowledgments(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
        limit: Option<usize>,
    ) -> StorageResult<Vec<BatchAcknowledgmentRecord>> {
        let pool = self.get_pool()?;

        let mut query = "SELECT id, channel_id, last_sequence_number, new_submits_accepted_count, new_shares_sum, timestamp FROM batch_acknowledgments WHERE channel_id = ?1".to_string();
        let mut bind_count = 1;

        if start_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp >= ?{}", bind_count));
        }
        if end_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp <= ?{}", bind_count));
        }

        query.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = limit {
            bind_count += 1;
            query.push_str(&format!(" LIMIT ?{}", bind_count));
        }

        let mut sql_query = sqlx::query(&query).bind(channel_id);

        if let Some(start) = start_timestamp {
            sql_query = sql_query.bind(start as i64);
        }
        if let Some(end) = end_timestamp {
            sql_query = sql_query.bind(end as i64);
        }
        if let Some(limit) = limit {
            sql_query = sql_query.bind(limit as i64);
        }

        let rows = sql_query
            .fetch_all(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        let mut records = Vec::new();
        for row in rows {
            records.push(BatchAcknowledgmentRecord {
                id: row.get("id"),
                channel_id: row.get("channel_id"),
                last_sequence_number: row.get::<i64, _>("last_sequence_number") as u32,
                new_submits_accepted_count: row.get::<i64, _>("new_submits_accepted_count") as u32,
                new_shares_sum: row.get::<i64, _>("new_shares_sum") as u64,
                timestamp: row.get::<i64, _>("timestamp") as u64,
            });
        }

        Ok(records)
    }

    async fn get_total_shares(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<u64> {
        let pool = self.get_pool()?;

        let mut query = "SELECT COUNT(*) FROM share_records WHERE 1=1".to_string();
        let mut bind_count = 0;

        if start_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp >= ?{}", bind_count));
        }
        if end_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp <= ?{}", bind_count));
        }

        let mut sql_query = sqlx::query_scalar(&query);

        if let Some(start) = start_timestamp {
            sql_query = sql_query.bind(start as i64);
        }
        if let Some(end) = end_timestamp {
            sql_query = sql_query.bind(end as i64);
        }

        let count: i64 = sql_query
            .fetch_one(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(count as u64)
    }

    async fn get_total_work(
        &self,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<u64> {
        let pool = self.get_pool()?;

        let mut query = "SELECT COALESCE(SUM(share_work), 0) FROM share_records WHERE accepted = true".to_string();
        let mut bind_count = 0;

        if start_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp >= ?{}", bind_count));
        }
        if end_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp <= ?{}", bind_count));
        }

        let mut sql_query = sqlx::query_scalar(&query);

        if let Some(start) = start_timestamp {
            sql_query = sql_query.bind(start as i64);
        }
        if let Some(end) = end_timestamp {
            sql_query = sql_query.bind(end as i64);
        }

        let total: i64 = sql_query
            .fetch_one(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        Ok(total as u64)
    }

    async fn get_channel_stats(
        &self,
        channel_id: &str,
        start_timestamp: Option<u64>,
        end_timestamp: Option<u64>,
    ) -> StorageResult<ChannelStats> {
        let pool = self.get_pool()?;

        let mut query = r#"
            SELECT 
                COUNT(*) as total_shares,
                SUM(CASE WHEN accepted THEN 1 ELSE 0 END) as accepted_shares,
                SUM(CASE WHEN NOT accepted THEN 1 ELSE 0 END) as rejected_shares,
                COALESCE(SUM(CASE WHEN accepted THEN share_work ELSE 0 END), 0) as total_work,
                MAX(difficulty) as best_difficulty,
                MIN(timestamp) as first_share_timestamp,
                MAX(timestamp) as last_share_timestamp
            FROM share_records 
            WHERE channel_id = ?1
        "#.to_string();
        let mut bind_count = 1;

        if start_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp >= ?{}", bind_count));
        }
        if end_timestamp.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND timestamp <= ?{}", bind_count));
        }

        let mut sql_query = sqlx::query(&query).bind(channel_id);

        if let Some(start) = start_timestamp {
            sql_query = sql_query.bind(start as i64);
        }
        if let Some(end) = end_timestamp {
            sql_query = sql_query.bind(end as i64);
        }

        let row = sql_query
            .fetch_one(pool)
            .await
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        // Get blocks found count
        let blocks_found: i64 = if start_timestamp.is_some() || end_timestamp.is_some() {
            let mut blocks_query = "SELECT COUNT(*) FROM block_records WHERE channel_id = ?1".to_string();
            let mut blocks_bind_count = 1;

            if start_timestamp.is_some() {
                blocks_bind_count += 1;
                blocks_query.push_str(&format!(" AND timestamp >= ?{}", blocks_bind_count));
            }
            if end_timestamp.is_some() {
                blocks_bind_count += 1;
                blocks_query.push_str(&format!(" AND timestamp <= ?{}", blocks_bind_count));
            }

            let mut sql_query = sqlx::query_scalar(&blocks_query).bind(channel_id);
            if let Some(start) = start_timestamp {
                sql_query = sql_query.bind(start as i64);
            }
            if let Some(end) = end_timestamp {
                sql_query = sql_query.bind(end as i64);
            }
            sql_query.fetch_one(pool).await.map_err(|e| StorageError::BackendError(e.to_string()))?
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM block_records WHERE channel_id = ?1")
                .bind(channel_id)
                .fetch_one(pool)
                .await
                .map_err(|e| StorageError::BackendError(e.to_string()))?
        };

        Ok(ChannelStats {
            channel_id: channel_id.to_string(),
            total_shares: row.get::<i64, _>("total_shares") as u64,
            accepted_shares: row.get::<i64, _>("accepted_shares") as u64,
            rejected_shares: row.get::<i64, _>("rejected_shares") as u64,
            total_work: row.get::<i64, _>("total_work") as u64,
            blocks_found: blocks_found as u64,
            best_difficulty: row.get::<Option<f64>, _>("best_difficulty").unwrap_or(0.0),
            first_share_timestamp: row.get::<Option<i64>, _>("first_share_timestamp").map(|t| t as u64),
            last_share_timestamp: row.get::<Option<i64>, _>("last_share_timestamp").map(|t| t as u64),
        })
    }

    async fn health_check(&self) -> StorageResult<StorageHealth> {
        if let Some(pool) = &self.pool {
            // Try a simple query to test the connection
            match sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(pool).await {
                Ok(_) => Ok(StorageHealth {
                    is_healthy: true,
                    backend_type: "sqlite".to_string(),
                    connection_status: "connected".to_string(),
                    last_operation_timestamp: Some(Self::current_timestamp()),
                    error_message: None,
                }),
                Err(e) => Ok(StorageHealth {
                    is_healthy: false,
                    backend_type: "sqlite".to_string(),
                    connection_status: "error".to_string(),
                    last_operation_timestamp: Some(Self::current_timestamp()),
                    error_message: Some(e.to_string()),
                }),
            }
        } else {
            Ok(StorageHealth {
                is_healthy: false,
                backend_type: "sqlite".to_string(),
                connection_status: "disconnected".to_string(),
                last_operation_timestamp: Some(Self::current_timestamp()),
                error_message: Some("Not initialized".to_string()),
            })
        }
    }
}