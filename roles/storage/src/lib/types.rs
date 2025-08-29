//! Data types for share accounting and validation storage.

use bitcoin::hashes::sha256d::Hash;
use serde::{Deserialize, Serialize};

/// Persistent share accounting data for a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareAccountingData {
    /// Channel identifier (could be channel_id or connection_id)
    pub channel_id: String,
    /// Sequence number of the last accepted share
    pub last_share_sequence_number: u32,
    /// Total number of shares accepted
    pub shares_accepted: u32,
    /// Cumulative work contributed by all accepted shares
    pub share_work_sum: u64,
    /// Highest difficulty found among accepted shares
    pub best_diff: f64,
    /// Timestamp of last update (Unix timestamp)
    pub last_updated: u64,
}

/// Share submission record for historical tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRecord {
    /// Unique identifier for this share record
    pub id: String,
    /// Channel identifier
    pub channel_id: String,
    /// Share hash for duplicate detection
    pub share_hash: Hash,
    /// Share sequence number
    pub sequence_number: u32,
    /// Work contributed by this share
    pub share_work: u64,
    /// Difficulty of this share
    pub difficulty: f64,
    /// Timestamp when share was submitted (Unix timestamp)
    pub timestamp: u64,
    /// Whether this share was accepted
    pub accepted: bool,
    /// Validation result (if applicable)
    pub validation_result: Option<ShareValidationOutcome>,
}

/// Outcome of share validation for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShareValidationOutcome {
    /// Share was valid and accepted
    Valid,
    /// Share was valid and triggered batch acknowledgment
    ValidWithAcknowledgement {
        last_sequence_number: u32,
        new_submits_accepted_count: u32,
        new_shares_sum: u64,
    },
    /// Share found a block
    BlockFound {
        template_id: Option<u64>,
        coinbase: Vec<u8>,
    },
    /// Share validation failed
    Failed {
        error: ShareValidationErrorType,
    },
}

/// Share validation error types for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShareValidationErrorType {
    Invalid,
    Stale,
    InvalidJobId,
    DoesNotMeetTarget,
    VersionRollingNotAllowed,
    DuplicateShare,
    InvalidCoinbase,
    NoChainTip,
}

/// Block discovery record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRecord {
    /// Unique identifier for this block record
    pub id: String,
    /// Channel identifier that found the block
    pub channel_id: String,
    /// Share hash that found the block
    pub share_hash: Hash,
    /// Template ID (None for custom jobs)
    pub template_id: Option<u64>,
    /// Serialized coinbase transaction
    pub coinbase: Vec<u8>,
    /// Block difficulty
    pub difficulty: f64,
    /// Timestamp when block was found (Unix timestamp)
    pub timestamp: u64,
}

/// Batch acknowledgment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAcknowledgmentRecord {
    /// Unique identifier for this batch
    pub id: String,
    /// Channel identifier
    pub channel_id: String,
    /// Last sequence number in the batch
    pub last_sequence_number: u32,
    /// Number of new shares accepted in this batch
    pub new_submits_accepted_count: u32,
    /// Total work contributed by shares in this batch
    pub new_shares_sum: u64,
    /// Timestamp of the acknowledgment (Unix timestamp)
    pub timestamp: u64,
}