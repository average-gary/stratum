//! Storage service wrapper for integrating with existing roles.
//! 
//! This module is only available when the `service-integration` feature is enabled.

use async_trait::async_trait;
use bitcoin::hashes::sha256d::Hash;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::{
    error::StorageResult,
    share_accounting_storage::ShareAccountingStorage,
    types::{ShareAccountingData, ShareRecord, ShareValidationOutcome},
};

/// A service that wraps existing ShareAccounting with persistent storage.
/// 
/// This allows existing roles to gradually adopt persistence without major refactoring.
/// It acts as a bridge between the in-memory ShareAccounting and persistent storage.
pub struct ShareAccountingStorageService {
    storage: RwLock<Box<dyn ShareAccountingStorage>>,
}

impl ShareAccountingStorageService {
    /// Create a new storage service with the specified backend
    pub fn new(storage: Box<dyn ShareAccountingStorage>) -> Self {
        Self {
            storage: RwLock::new(storage),
        }
    }

    /// Sync existing ShareAccounting data to storage
    pub async fn sync_from_share_accounting(
        &self,
        channel_id: &str,
        share_accounting: &stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareAccounting,
    ) -> StorageResult<()> {
        let storage_data = ShareAccountingData {
            channel_id: channel_id.to_string(),
            last_share_sequence_number: share_accounting.get_last_share_sequence_number(),
            shares_accepted: share_accounting.get_shares_accepted(),
            share_work_sum: share_accounting.get_share_work_sum(),
            best_diff: share_accounting.get_best_diff(),
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let mut storage = self.storage.write().await;
        storage.store_share_accounting(&storage_data).await
    }

    /// Load ShareAccounting data from storage to populate a new ShareAccounting instance
    pub async fn load_to_share_accounting(
        &self,
        channel_id: &str,
    ) -> StorageResult<Option<ShareAccountingData>> {
        let storage = self.storage.read().await;
        storage.get_share_accounting(channel_id).await
    }

    /// Store a share submission record
    pub async fn record_share_submission(
        &self,
        channel_id: &str,
        share_hash: Hash,
        sequence_number: u32,
        share_work: u64,
        difficulty: f64,
        accepted: bool,
        validation_result: Option<ShareValidationOutcome>,
    ) -> StorageResult<()> {
        let share_record = ShareRecord {
            id: format!("{}_{}", channel_id, sequence_number),
            channel_id: channel_id.to_string(),
            share_hash,
            sequence_number,
            share_work,
            difficulty,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            accepted,
            validation_result,
        };

        let mut storage = self.storage.write().await;
        storage.store_share_record(&share_record).await
    }

    /// Check if a share has been seen before (for duplicate detection)
    pub async fn is_share_duplicate(
        &self,
        channel_id: &str,
        share_hash: &Hash,
    ) -> StorageResult<bool> {
        let storage = self.storage.read().await;
        storage.is_share_seen(channel_id, share_hash).await
    }

    /// Get storage reference for advanced operations
    pub async fn get_storage(&self) -> tokio::sync::RwLockReadGuard<Box<dyn ShareAccountingStorage>> {
        self.storage.read().await
    }

    /// Get mutable storage reference for advanced operations
    pub async fn get_storage_mut(&self) -> tokio::sync::RwLockWriteGuard<Box<dyn ShareAccountingStorage>> {
        self.storage.write().await
    }
}

/// Integration helper for existing pool implementations
pub struct PoolStorageIntegration {
    storage_service: ShareAccountingStorageService,
}

impl PoolStorageIntegration {
    pub fn new(storage: Box<dyn ShareAccountingStorage>) -> Self {
        Self {
            storage_service: ShareAccountingStorageService::new(storage),
        }
    }

    /// Hook to call after share validation in existing pool code
    pub async fn on_share_validated(
        &self,
        channel_id: &str,
        share_hash: Hash,
        sequence_number: u32,
        share_work: u64,
        difficulty: f64,
        validation_result: &stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationResult,
        share_accounting: &stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareAccounting,
    ) -> StorageResult<()> {
        // Convert validation result to our storage format
        let storage_result = match validation_result {
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationResult::Valid => {
                Some(ShareValidationOutcome::Valid)
            }
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationResult::ValidWithAcknowledgement(seq, count, sum) => {
                Some(ShareValidationOutcome::ValidWithAcknowledgement {
                    last_sequence_number: *seq,
                    new_submits_accepted_count: *count,
                    new_shares_sum: *sum,
                })
            }
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationResult::BlockFound(template_id, coinbase) => {
                Some(ShareValidationOutcome::BlockFound {
                    template_id: *template_id,
                    coinbase: coinbase.clone(),
                })
            }
        };

        // Record the share
        self.storage_service
            .record_share_submission(
                channel_id,
                share_hash,
                sequence_number,
                share_work,
                difficulty,
                true, // accepted since validation succeeded
                storage_result,
            )
            .await?;

        // Sync the updated accounting data
        self.storage_service
            .sync_from_share_accounting(channel_id, share_accounting)
            .await?;

        Ok(())
    }

    /// Hook to call when share validation fails
    pub async fn on_share_validation_failed(
        &self,
        channel_id: &str,
        share_hash: Hash,
        sequence_number: u32,
        share_work: u64,
        difficulty: f64,
        error: &stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError,
    ) -> StorageResult<()> {
        use crate::types::ShareValidationErrorType;

        let error_type = match error {
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError::Invalid => ShareValidationErrorType::Invalid,
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError::Stale => ShareValidationErrorType::Stale,
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError::InvalidJobId => ShareValidationErrorType::InvalidJobId,
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError::DoesNotMeetTarget => ShareValidationErrorType::DoesNotMeetTarget,
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError::VersionRollingNotAllowed => ShareValidationErrorType::VersionRollingNotAllowed,
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError::DuplicateShare => ShareValidationErrorType::DuplicateShare,
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError::InvalidCoinbase => ShareValidationErrorType::InvalidCoinbase,
            stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareValidationError::NoChainTip => ShareValidationErrorType::NoChainTip,
        };

        self.storage_service
            .record_share_submission(
                channel_id,
                share_hash,
                sequence_number,
                share_work,
                difficulty,
                false, // not accepted
                Some(ShareValidationOutcome::Failed { error: error_type }),
            )
            .await
    }

    /// Initialize/restore ShareAccounting from storage on startup
    pub async fn restore_share_accounting(
        &self,
        channel_id: &str,
        share_batch_size: usize,
    ) -> StorageResult<stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareAccounting> {
        if let Some(stored_data) = self.storage_service.load_to_share_accounting(channel_id).await? {
            // Create a new ShareAccounting and populate it with stored data
            let mut share_accounting = stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareAccounting::new(share_batch_size);
            
            // Note: The existing ShareAccounting doesn't have setters, so we'd need to modify it
            // or use reflection/unsafe code to restore state. For now, we document this limitation.
            tracing::warn!("ShareAccounting restoration requires modifications to the existing ShareAccounting struct");
            
            Ok(share_accounting)
        } else {
            // No stored data, create fresh ShareAccounting
            Ok(stratum_common::roles_logic_sv2::channels_sv2::server::share_accounting::ShareAccounting::new(share_batch_size))
        }
    }
}