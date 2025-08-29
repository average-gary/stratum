//! Mock data generator for storage examples.
//! 
//! Provides realistic test data for demonstrating storage backends.

use bitcoin::hashes::{sha256d::Hash, Hash as HashTrait};
use std::time::{SystemTime, UNIX_EPOCH};
use storage_sv2::types::*;

pub struct MockDataGenerator {
    pub channel_ids: Vec<String>,
    pub current_timestamp: u64,
}

impl MockDataGenerator {
    pub fn new() -> Self {
        Self {
            channel_ids: vec![
                "channel_mining_001".to_string(),
                "channel_mining_002".to_string(),
                "channel_extended_001".to_string(),
                "channel_standard_001".to_string(),
                "channel_standard_002".to_string(),
            ],
            current_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Generate mock share accounting data for all channels
    pub fn generate_share_accounting_data(&self) -> Vec<ShareAccountingData> {
        let mut data = Vec::new();
        let base_time = self.current_timestamp - 3600; // 1 hour ago

        for (i, channel_id) in self.channel_ids.iter().enumerate() {
            let multiplier = (i + 1) as u64;
            data.push(ShareAccountingData {
                channel_id: channel_id.clone(),
                last_share_sequence_number: (1000 + i * 500) as u32,
                shares_accepted: (50 * multiplier) as u32,
                share_work_sum: 1000000 * multiplier,
                best_diff: 2048.0 * multiplier as f64,
                last_updated: base_time + (i as u64 * 60), // Spread over time
            });
        }

        data
    }

    /// Generate mock share records
    pub fn generate_share_records(&self) -> Vec<ShareRecord> {
        let mut records = Vec::new();
        let base_time = self.current_timestamp - 1800; // 30 minutes ago

        for (channel_idx, channel_id) in self.channel_ids.iter().enumerate() {
            // Generate 10-15 shares per channel
            let share_count = 10 + (channel_idx * 2);
            
            for i in 0..share_count {
                let sequence_number = (i + 1) as u32;
                let timestamp = base_time + (i as u64 * 30) + (channel_idx as u64 * 300);
                
                // Create a deterministic but varied hash
                let hash_input = format!("{}_{}_share", channel_id, sequence_number);
                let share_hash = Hash::hash(hash_input.as_bytes());
                
                let share_work = match channel_idx {
                    0 => 1000 + (i as u64 * 100),  // Lower difficulty shares
                    1 => 2000 + (i as u64 * 150),  // Medium difficulty
                    _ => 5000 + (i as u64 * 200),  // Higher difficulty
                };

                let difficulty = share_work as f64 / 1000.0;
                let accepted = i % 7 != 0; // ~85% acceptance rate (reject every 7th)

                let validation_result = if accepted {
                    if i % 20 == 0 && i > 0 {
                        // Every 20th share triggers batch acknowledgment
                        Some(ShareValidationOutcome::ValidWithAcknowledgement {
                            last_sequence_number: sequence_number,
                            new_submits_accepted_count: 20,
                            new_shares_sum: share_work * 20,
                        })
                    } else if i % 50 == 0 && i > 0 {
                        // Rare block finds
                        Some(ShareValidationOutcome::BlockFound {
                            template_id: Some(12345 + i as u64),
                            coinbase: vec![0x01, 0x02, 0x03, 0x04], // Mock coinbase
                        })
                    } else {
                        Some(ShareValidationOutcome::Valid)
                    }
                } else {
                    // Failed validation with various error types
                    let error_type = match i % 4 {
                        0 => ShareValidationErrorType::DoesNotMeetTarget,
                        1 => ShareValidationErrorType::Stale,
                        2 => ShareValidationErrorType::DuplicateShare,
                        _ => ShareValidationErrorType::Invalid,
                    };
                    Some(ShareValidationOutcome::Failed { error: error_type })
                };

                records.push(ShareRecord {
                    id: format!("share_{}_{}", channel_id, sequence_number),
                    channel_id: channel_id.clone(),
                    share_hash,
                    sequence_number,
                    share_work,
                    difficulty,
                    timestamp,
                    accepted,
                    validation_result,
                });
            }
        }

        records
    }

    /// Generate mock block records
    pub fn generate_block_records(&self) -> Vec<BlockRecord> {
        let mut records = Vec::new();
        let base_time = self.current_timestamp - 7200; // 2 hours ago

        // Generate 3-5 blocks found across different channels
        let block_data = [
            ("channel_mining_001", 12345u64, 2048.0),
            ("channel_extended_001", 12346u64, 4096.0),
            ("channel_mining_002", 12347u64, 1024.0),
        ];

        for (i, (channel_id, template_id, difficulty)) in block_data.iter().enumerate() {
            let timestamp = base_time + (i as u64 * 1800); // 30 minutes apart
            let hash_input = format!("block_{}_template_{}", channel_id, template_id);
            let share_hash = Hash::hash(hash_input.as_bytes());

            // Mock coinbase transaction (simplified)
            let coinbase = vec![
                0x01, 0x00, 0x00, 0x00, // version
                0x01, // input count
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // prev hash (null)
                0xff, 0xff, 0xff, 0xff, // prev index (coinbase)
            ];

            records.push(BlockRecord {
                id: format!("block_{}", template_id),
                channel_id: channel_id.to_string(),
                share_hash,
                template_id: Some(*template_id),
                coinbase,
                difficulty: *difficulty,
                timestamp,
            });
        }

        records
    }

    /// Generate mock batch acknowledgment records
    pub fn generate_batch_acknowledgment_records(&self) -> Vec<BatchAcknowledgmentRecord> {
        let mut records = Vec::new();
        let base_time = self.current_timestamp - 900; // 15 minutes ago

        for (channel_idx, channel_id) in self.channel_ids.iter().enumerate() {
            // Generate 2-3 batch acknowledgments per channel
            for batch in 0..3 {
                let timestamp = base_time + (batch as u64 * 300) + (channel_idx as u64 * 60);
                let batch_size = 20 + (batch * 5); // Varying batch sizes
                
                records.push(BatchAcknowledgmentRecord {
                    id: format!("batch_{}_{}", channel_id, batch),
                    channel_id: channel_id.clone(),
                    last_sequence_number: ((batch + 1) * batch_size) as u32,
                    new_submits_accepted_count: batch_size as u32,
                    new_shares_sum: (batch_size as u64) * (1000 + channel_idx as u64 * 500),
                    timestamp,
                });
            }
        }

        records
    }

    /// Generate all mock data at once
    pub fn generate_all_data(&self) -> MockDataSet {
        MockDataSet {
            share_accounting: self.generate_share_accounting_data(),
            share_records: self.generate_share_records(),
            block_records: self.generate_block_records(),
            batch_acknowledgments: self.generate_batch_acknowledgment_records(),
        }
    }
}

pub struct MockDataSet {
    pub share_accounting: Vec<ShareAccountingData>,
    pub share_records: Vec<ShareRecord>,
    pub block_records: Vec<BlockRecord>,
    pub batch_acknowledgments: Vec<BatchAcknowledgmentRecord>,
}

impl MockDataSet {
    pub fn print_summary(&self) {
        println!("📊 Generated Mock Data Summary:");
        println!("  • Share Accounting Records: {}", self.share_accounting.len());
        println!("  • Share Records: {}", self.share_records.len());
        println!("  • Block Records: {}", self.block_records.len());
        println!("  • Batch Acknowledgment Records: {}", self.batch_acknowledgments.len());
        
        // Calculate some statistics
        let total_accepted_shares: u64 = self.share_records.iter()
            .filter(|r| r.accepted)
            .count() as u64;
        let total_work: u64 = self.share_records.iter()
            .filter(|r| r.accepted)
            .map(|r| r.share_work)
            .sum();
        
        println!("  • Total Accepted Shares: {}", total_accepted_shares);
        println!("  • Total Work Contributed: {}", total_work);
        println!("  • Acceptance Rate: {:.1}%", 
            (total_accepted_shares as f64 / self.share_records.len() as f64) * 100.0);
    }
}