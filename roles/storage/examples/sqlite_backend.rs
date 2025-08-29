//! SQLite Storage Backend Example
//! 
//! Demonstrates the SQLite storage backend with comprehensive mock data.
//! This example shows all storage operations including:
//! - Database schema creation
//! - Data persistence across restarts
//! - SQL-based analytics and queries
//! - Transaction safety

use storage_sv2::{
    backends::sqlite::SqliteStorage,
    share_accounting_storage::ShareAccountingStorage,
    StorageResult,
};
use std::{fs, time::Instant};
use tokio;

mod mock_data;
use mock_data::MockDataGenerator;

const DB_PATH: &str = "./example_storage.db";

#[tokio::main]
async fn main() -> StorageResult<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("💾 SQLite Storage Backend Example");
    println!("==================================\n");

    // Clean up any existing database for a fresh start
    if fs::metadata(DB_PATH).is_ok() {
        println!("🧹 Removing existing database for clean demo...");
        fs::remove_file(DB_PATH).map_err(|e| {
            storage_sv2::StorageError::BackendError(format!("Failed to remove existing database: {}", e))
        })?;
    }

    // Initialize storage
    let mut storage = SqliteStorage::new(DB_PATH);
    println!("📝 Initializing SQLite storage at: {}", DB_PATH);
    storage.initialize().await?;
    println!("   ✓ Database created and tables initialized");
    println!();

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

    // Store share records (this will be the bulk of the data)
    println!("📊 Storing share records...");
    for (i, share_record) in mock_data.share_records.iter().enumerate() {
        storage.store_share_record(share_record).await?;
        if (i + 1) % 10 == 0 || i == mock_data.share_records.len() - 1 {
            print!("   Progress: {}/{} records\r", i + 1, mock_data.share_records.len());
        }
    }
    println!("\n   ✓ Stored {} share records", mock_data.share_records.len());

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
    println!("\n⏱️  Data insertion completed in: {:?}", insertion_time);
    
    // Check database file size
    if let Ok(metadata) = fs::metadata(DB_PATH) {
        println!("💿 Database file size: {:.2} KB", metadata.len() as f64 / 1024.0);
    }
    println!();

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
            if stats.total_shares > 0 {
                (stats.accepted_shares as f64 / stats.total_shares as f64) * 100.0
            } else { 0.0 }
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

    // Time-based queries
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
    if total_shares > 0 {
        println!("   Recent Activity: {:.1}% of total", (recent_shares as f64 / total_shares as f64) * 100.0);
    }
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

    // Query recent share records with SQL-like precision
    if let Some(channel_id) = channels.first() {
        println!("📊 Recent Share Records for: {}", channel_id);
        let recent_shares = storage.get_share_records(channel_id, None, None, Some(10)).await?;
        
        println!("   Most Recent Shares (limit 10):");
        for (i, share) in recent_shares.iter().enumerate() {
            println!("   {}. Seq #{}: {} (work: {}, diff: {:.1}) - {}", 
                i + 1,
                share.sequence_number,
                if share.accepted { "✓ Accepted" } else { "✗ Rejected" },
                share.share_work,
                share.difficulty,
                if let Some(ref result) = share.validation_result {
                    match result {
                        storage_sv2::types::ShareValidationOutcome::Valid => "Valid",
                        storage_sv2::types::ShareValidationOutcome::ValidWithAcknowledgement { .. } => "Valid+Batch",
                        storage_sv2::types::ShareValidationOutcome::BlockFound { .. } => "🎉 Block Found!",
                        storage_sv2::types::ShareValidationOutcome::Failed { .. } => "Failed",
                    }
                } else {
                    "No result"
                }
            );
        }
        println!();
    }

    // Show all block discoveries with detailed info
    println!("🏆 Block Discoveries");
    let blocks = storage.get_block_records(None, None, None).await?;
    if blocks.is_empty() {
        println!("   No blocks found in this dataset");
    } else {
        for (i, block) in blocks.iter().enumerate() {
            println!("   {}. 🏆 Channel: {} | Template: {:?} | Difficulty: {:.1}", 
                i + 1,
                block.channel_id,
                block.template_id,
                block.difficulty
            );
            println!("      Hash: {:x}", block.share_hash);
            println!("      Coinbase: {} bytes", block.coinbase.len());
            
            // Show timestamp in human readable format
            let datetime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(block.timestamp);
            println!("      Time: {:?}", datetime);
        }
    }
    println!();

    // Show batch acknowledgments
    if let Some(channel_id) = channels.first() {
        println!("📦 Batch Acknowledgments for: {}", channel_id);
        let batches = storage.get_batch_acknowledgments(channel_id, None, None, Some(5)).await?;
        
        for (i, batch) in batches.iter().enumerate() {
            println!("   {}. Batch {} - Last Seq: {}, Count: {}, Work: {}", 
                i + 1,
                batch.id,
                batch.last_sequence_number,
                batch.new_submits_accepted_count,
                batch.new_shares_sum
            );
        }
        println!();
    }

    // Persistence demonstration - close and reopen
    println!("🔄 Testing Persistence (Close & Reopen)");
    storage.close().await?;
    println!("   ✓ Database closed");

    // Reopen the database
    let mut storage = SqliteStorage::new(DB_PATH);
    storage.initialize().await?;
    println!("   ✓ Database reopened");

    // Verify data persisted
    let channels_after_reopen = storage.list_channels().await?;
    let total_shares_after_reopen = storage.get_total_shares(None, None).await?;
    
    println!("   Channels after reopen: {} (expected: {})", 
        channels_after_reopen.len(), channels.len());
    println!("   Total shares after reopen: {} (expected: {})", 
        total_shares_after_reopen, total_shares);
    
    if channels_after_reopen.len() == channels.len() && total_shares_after_reopen == total_shares {
        println!("   ✅ Data persistence verified!");
    } else {
        println!("   ❌ Data persistence failed!");
    }
    println!();

    // Performance and storage info
    println!("⚡ SQLite Backend Performance");
    println!("   Total Records: {}", 
        mock_data.share_accounting.len() + 
        mock_data.share_records.len() + 
        mock_data.block_records.len() + 
        mock_data.batch_acknowledgments.len()
    );
    println!("   Insertion Time: {:?}", insertion_time);
    println!("   Database File: {}", DB_PATH);
    if let Ok(metadata) = fs::metadata(DB_PATH) {
        println!("   File Size: {:.2} KB", metadata.len() as f64 / 1024.0);
    }
    println!("   Features: ACID transactions, SQL queries, persistence");
    println!("   Thread Safety: Full concurrent read/write support");
    println!();

    // Health check
    println!("🏥 Health Check");
    let health = storage.health_check().await?;
    println!("   Status: {}", if health.is_healthy { "✅ Healthy" } else { "❌ Unhealthy" });
    println!("   Backend: {}", health.backend_type);
    println!("   Connection: {}", health.connection_status);
    if let Some(last_op) = health.last_operation_timestamp {
        println!("   Last Operation: {}", last_op);
    }
    if let Some(ref error) = health.error_message {
        println!("   Error: {}", error);
    }
    println!();

    // Cleanup demo
    println!("🧹 Cleanup Operations");
    let old_cutoff = mock_data.share_records.iter()
        .map(|r| r.timestamp)
        .min()
        .unwrap_or(0) + 900; // Remove shares older than 15 minutes from oldest
    
    let cleaned_count = storage.cleanup_old_shares(old_cutoff).await?;
    println!("   Cleaned up {} old share records", cleaned_count);

    // Final verification
    let final_share_count = storage.get_total_shares(None, None).await?;
    println!("   Remaining shares: {} (removed: {})", 
        final_share_count, total_shares_after_reopen - final_share_count);

    // Close storage
    storage.close().await?;
    
    println!("\n✅ SQLite storage example completed successfully!");
    println!("💡 Database file '{}' contains all the data and can be inspected with SQLite tools", DB_PATH);

    Ok(())
}