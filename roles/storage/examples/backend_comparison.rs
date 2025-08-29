//! Backend Comparison Example
//! 
//! Demonstrates and compares different storage backends:
//! - Performance characteristics
//! - Feature differences
//! - Use case recommendations

use storage_sv2::{
    backends::{memory::MemoryStorage},
    share_accounting_storage::ShareAccountingStorage,
    StorageResult,
};

#[cfg(feature = "sqlite-backend")]
use storage_sv2::backends::sqlite::SqliteStorage;

use std::{time::Instant};
use tokio;

mod mock_data;
use mock_data::MockDataGenerator;

#[tokio::main]
async fn main() -> StorageResult<()> {
    tracing_subscriber::fmt::init();

    println!("⚖️  Storage Backend Comparison");
    println!("==============================\n");

    // Generate test data once
    let generator = MockDataGenerator::new();
    let mock_data = generator.generate_all_data();
    
    println!("📊 Test Dataset:");
    mock_data.print_summary();
    println!();

    // Test Memory Backend
    println!("🧠 Testing Memory Backend");
    println!("-------------------------");
    let memory_perf = test_backend_performance(&mut MemoryStorage::new(), &mock_data, "Memory").await?;

    #[cfg(feature = "sqlite-backend")]
    {
        // Test SQLite Backend
        println!("\n💾 Testing SQLite Backend");
        println!("-------------------------");
        let sqlite_path = "./comparison_test.db";
        
        // Clean up existing database
        let _ = std::fs::remove_file(sqlite_path);
        
        let mut sqlite_storage = SqliteStorage::new(sqlite_path);
        let sqlite_perf = test_backend_performance(&mut sqlite_storage, &mock_data, "SQLite").await?;

        // Performance Comparison
        println!("\n📈 Performance Comparison");
        println!("=========================");
        println!("| Metric                | Memory     | SQLite     | Winner  |");
        println!("|-----------------------|------------|------------|---------|");
        
        println!("| Initialization       | {:>8.2}ms | {:>8.2}ms | {} |",
            memory_perf.init_time.as_secs_f64() * 1000.0,
            sqlite_perf.init_time.as_secs_f64() * 1000.0,
            if memory_perf.init_time < sqlite_perf.init_time { "Memory " } else { "SQLite " }
        );
        
        println!("| Data Insertion        | {:>8.2}ms | {:>8.2}ms | {} |",
            memory_perf.insert_time.as_secs_f64() * 1000.0,
            sqlite_perf.insert_time.as_secs_f64() * 1000.0,
            if memory_perf.insert_time < sqlite_perf.insert_time { "Memory " } else { "SQLite " }
        );
        
        println!("| Query Performance     | {:>8.2}ms | {:>8.2}ms | {} |",
            memory_perf.query_time.as_secs_f64() * 1000.0,
            sqlite_perf.query_time.as_secs_f64() * 1000.0,
            if memory_perf.query_time < sqlite_perf.query_time { "Memory " } else { "SQLite " }
        );

        println!("\n🎯 Use Case Recommendations");
        println!("===========================");
        
        println!("🧠 **Memory Backend** - Best for:");
        println!("   • Development and testing");
        println!("   • High-performance scenarios with no persistence needs");
        println!("   • Temporary data processing");
        println!("   • Unit tests and benchmarks");
        println!("   • When disk I/O should be avoided");
        
        println!("\n💾 **SQLite Backend** - Best for:");
        println!("   • Production deployments requiring persistence");
        println!("   • Single-node applications");
        println!("   • Data analysis and historical queries");
        println!("   • Backup and recovery requirements");
        println!("   • Audit trails and compliance");

        println!("\n📊 Feature Matrix");
        println!("=================");
        println!("| Feature           | Memory | SQLite | Notes                    |");
        println!("|--------------------|--------|--------|--------------------------|");
        println!("| Persistence        | ❌     | ✅     | Data survives restarts   |");
        println!("| ACID Transactions  | ❌     | ✅     | Consistency guarantees   |");
        println!("| SQL Queries        | ❌     | ✅     | Complex analytics        |");
        println!("| Memory Usage       | High   | Low    | Memory vs disk storage   |");
        println!("| Startup Time       | Fast   | Medium | Schema creation overhead |");
        println!("| Query Performance  | Fast   | Medium | In-memory vs disk I/O    |");
        println!("| Concurrent Access  | ✅     | ✅     | Thread-safe operations   |");
        println!("| Data Export        | ❌     | ✅     | Standard SQLite tools    |");
        println!("| File Size          | N/A    | ~{}KB  | Compact binary format    |",
            std::fs::metadata(sqlite_path).map_or(0, |m| m.len()) / 1024
        );

        // Clean up
        let _ = std::fs::remove_file(sqlite_path);
    }

    #[cfg(not(feature = "sqlite-backend"))]
    {
        println!("\n💡 SQLite backend not available");
        println!("   Run with: cargo run --example backend_comparison --features sqlite-backend");
    }

    println!("\n✅ Backend comparison completed!");

    Ok(())
}

struct BackendPerformance {
    init_time: std::time::Duration,
    insert_time: std::time::Duration,
    query_time: std::time::Duration,
}

async fn test_backend_performance(
    storage: &mut dyn ShareAccountingStorage,
    mock_data: &mock_data::MockDataSet,
    backend_name: &str,
) -> StorageResult<BackendPerformance> {
    // Initialize
    let init_start = Instant::now();
    storage.initialize().await?;
    let init_time = init_start.elapsed();
    
    println!("   Initialization: {:?}", init_time);

    // Insert data
    let insert_start = Instant::now();
    
    for data in &mock_data.share_accounting {
        storage.store_share_accounting(data).await?;
    }
    
    for record in &mock_data.share_records {
        storage.store_share_record(record).await?;
    }
    
    for record in &mock_data.block_records {
        storage.store_block_record(record).await?;
    }
    
    for record in &mock_data.batch_acknowledgments {
        storage.store_batch_acknowledgment(record).await?;
    }
    
    let insert_time = insert_start.elapsed();
    println!("   Data Insertion: {:?}", insert_time);

    // Query operations
    let query_start = Instant::now();
    
    // Run a series of typical queries
    let _channels = storage.list_channels().await?;
    let _total_shares = storage.get_total_shares(None, None).await?;
    let _total_work = storage.get_total_work(None, None).await?;
    
    // Query each channel's stats
    for data in &mock_data.share_accounting {
        let _stats = storage.get_channel_stats(&data.channel_id, None, None).await?;
    }
    
    // Test duplicate detection
    if let Some(share) = mock_data.share_records.first() {
        let _is_dup = storage.is_share_seen(&share.channel_id, &share.share_hash).await?;
    }
    
    let query_time = query_start.elapsed();
    println!("   Query Operations: {:?}", query_time);

    // Health check
    let health = storage.health_check().await?;
    println!("   Health Status: {}", if health.is_healthy { "✅ Healthy" } else { "❌ Unhealthy" });

    storage.close().await?;

    Ok(BackendPerformance {
        init_time,
        insert_time,
        query_time,
    })
}