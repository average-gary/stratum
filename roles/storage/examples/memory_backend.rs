//! Memory Storage Backend Example
//! 
//! Demonstrates the in-memory storage backend with comprehensive mock data.
//! This example shows all storage operations and is useful for:
//! - Development and testing
//! - Understanding the storage API
//! - Performance benchmarking

use storage_sv2::{
    backends::memory::MemoryStorage,
    share_accounting_storage::ShareAccountingStorage,
    StorageResult,
};
use std::time::Instant;
use tokio;

mod mock_data;
use mock_data::MockDataGenerator;

#[tokio::main]
async fn main() -> StorageResult<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🧠 Memory Storage Backend Example");
    println!("==================================\n");

    // Initialize storage
    let mut storage = MemoryStorage::new();
    println!("📝 Initializing memory storage...");
    storage.initialize().await?;

    // Generate mock data
    println!("🎲 Generating mock data...");
    let generator = MockDataGenerator::new();
    let mock_data = generator.generate_all_data();
    mock_data.print_summary();
    println!();

    // Benchmark data insertion
    let start_time = Instant::now();
    
    // Store share accounting data
    println!("💾 Storing share accounting data...");
    for accounting_data in &mock_data.share_accounting {
        storage.store_share_accounting(accounting_data).await?;
    }
    println!("   ✓ Stored {} share accounting records", mock_data.share_accounting.len());

    // Store share records
    println!("📊 Storing share records...");
    for share_record in &mock_data.share_records {
        storage.store_share_record(share_record).await?;
    }
    println!("   ✓ Stored {} share records", mock_data.share_records.len());

    // Store block records
    println!("🏆 Storing block records...");
    for block_record in &mock_data.block_records {
        storage.store_block_record(block_record).await?;
    }
    println!("   ✓ Stored {} block records", mock_data.block_records.len());

    // Store batch acknowledgments
    println!("📦 Storing batch acknowledgment records...");
    for batch_record in &mock_data.batch_acknowledgments {
        storage.store_batch_acknowledgment(batch_record).await?;
    }
    println!("   ✓ Stored {} batch acknowledgment records", mock_data.batch_acknowledgments.len());

    let insertion_time = start_time.elapsed();
    println!("\n⏱️  Data insertion completed in: {:?}\n", insertion_time);

    // Demonstrate queries and analytics
    println!("🔍 Running Queries and Analytics");
    println!("================================\n");

    // List all channels
    let channels = storage.list_channels().await?;
    println!("📋 Active channels ({}):", channels.len());
    for channel in &channels {
        println!("   • {}", channel);
    }
    println!();

    // Get detailed stats for each channel
    for channel_id in &channels {
        println!("📈 Channel Stats: {}", channel_id);
        let stats = storage.get_channel_stats(channel_id, None, None).await?;
        
        println!("   Total Shares: {}", stats.total_shares);
        println!("   Accepted: {} ({:.1}%)", 
            stats.accepted_shares,
            (stats.accepted_shares as f64 / stats.total_shares as f64) * 100.0
        );
        println!("   Rejected: {}", stats.rejected_shares);
        println!("   Total Work: {}", stats.total_work);
        println!("   Best Difficulty: {:.2}", stats.best_difficulty);
        println!("   Blocks Found: {}", stats.blocks_found);
        
        if let (Some(first), Some(last)) = (stats.first_share_timestamp, stats.last_share_timestamp) {
            let duration = last - first;
            println!("   Active Duration: {}s", duration);
            if duration > 0 {
                println!("   Shares/sec: {:.2}", stats.total_shares as f64 / duration as f64);
            }
        }
        println!();
    }

    // Global analytics
    println!("🌍 Global Analytics");
    let total_shares = storage.get_total_shares(None, None).await?;
    let total_work = storage.get_total_work(None, None).await?;
    println!("   Total Shares Across All Channels: {}", total_shares);
    println!("   Total Work Across All Channels: {}", total_work);
    println!();

    // Recent activity (last 30 minutes)
    let recent_cutoff = mock_data.share_records.iter()
        .map(|r| r.timestamp)
        .max()
        .unwrap_or(0)
        .saturating_sub(1800); // 30 minutes ago

    let recent_shares = storage.get_total_shares(Some(recent_cutoff), None).await?;
    let recent_work = storage.get_total_work(Some(recent_cutoff), None).await?;
    
    println!("⏰ Recent Activity (last 30 mins):");
    println!("   Recent Shares: {}", recent_shares);
    println!("   Recent Work: {}", recent_work);
    println!();

    // Demonstrate duplicate detection
    println!("🔍 Duplicate Detection Test");
    if let Some(first_share) = mock_data.share_records.first() {
        let is_duplicate = storage.is_share_seen(&first_share.channel_id, &first_share.share_hash).await?;
        println!("   Share {} is{} a duplicate", 
            first_share.id,
            if is_duplicate { "" } else { " not" }
        );
    }
    println!();

    // Query recent share records for a specific channel
    if let Some(channel_id) = channels.first() {
        println!("📊 Recent Share Records for: {}", channel_id);
        let recent_shares = storage.get_share_records(channel_id, None, None, Some(5)).await?;
        
        for (i, share) in recent_shares.iter().enumerate() {
            println!("   {}. Seq #{}: {} (work: {}, diff: {:.1}, {})", 
                i + 1,
                share.sequence_number,
                if share.accepted { "✓ Accepted" } else { "✗ Rejected" },
                share.share_work,
                share.difficulty,
                if let Some(ref result) = share.validation_result {
                    match result {
                        storage_sv2::types::ShareValidationOutcome::Valid => "Valid",
                        storage_sv2::types::ShareValidationOutcome::ValidWithAcknowledgement { .. } => "Valid+Ack",
                        storage_sv2::types::ShareValidationOutcome::BlockFound { .. } => "Block Found!",
                        storage_sv2::types::ShareValidationOutcome::Failed { .. } => "Failed",
                    }
                } else {
                    "No result"
                }
            );
        }
        println!();
    }

    // Show all block discoveries
    println!("🏆 Block Discoveries");
    let blocks = storage.get_block_records(None, None, None).await?;
    if blocks.is_empty() {
        println!("   No blocks found in this dataset");
    } else {
        for (i, block) in blocks.iter().enumerate() {
            println!("   {}. Channel: {} | Template: {} | Difficulty: {:.1} | Coinbase: {} bytes",
                i + 1,
                block.channel_id,
                block.template_id.unwrap_or(0),
                block.difficulty,
                block.coinbase.len()
            );
        }
    }
    println!();

    // Health check
    println!("🏥 Health Check");
    let health = storage.health_check().await?;
    println!("   Status: {}", if health.is_healthy { "✓ Healthy" } else { "✗ Unhealthy" });
    println!("   Backend: {}", health.backend_type);
    println!("   Connection: {}", health.connection_status);
    if let Some(last_op) = health.last_operation_timestamp {
        println!("   Last Operation: {}", last_op);
    }
    println!();

    // Performance summary
    println!("⚡ Performance Summary");
    println!("   Total Records: {}", 
        mock_data.share_accounting.len() + 
        mock_data.share_records.len() + 
        mock_data.block_records.len() + 
        mock_data.batch_acknowledgments.len()
    );
    println!("   Insertion Time: {:?}", insertion_time);
    println!("   Memory Backend: Zero persistence (data lost on shutdown)");
    println!("   Thread Safety: Full concurrent read/write support");
    println!();

    // Cleanup demo
    println!("🧹 Cleanup Operations");
    let old_cutoff = mock_data.share_records.iter()
        .map(|r| r.timestamp)
        .min()
        .unwrap_or(0) + 900; // Remove shares older than 15 minutes from oldest
    
    let cleaned_count = storage.cleanup_old_shares(old_cutoff).await?;
    println!("   Cleaned up {} old share records", cleaned_count);

    // Close storage
    storage.close().await?;
    println!("✅ Memory storage example completed successfully!");

    Ok(())
}