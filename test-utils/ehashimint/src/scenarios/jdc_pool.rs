use std::path::PathBuf;

use anyhow::Result;
use tracing::info;

use crate::config::{self, defaults, PoolConfig, JdcConfig, JdsConfig};
use crate::process::ProcessManager;
use crate::scenarios::{ScenarioContext, find_binary};

/// Run JDC with Pool Mint and JDS configuration
///
/// In this setup:
/// - Pool mints eHash tokens
/// - JDC acts as proxy (Wallet mode) and tracks correlation
/// - JDS provides job declaration service
/// - SV2 miners connect directly to JDC
pub async fn run(
    test_dir: PathBuf,
    with_miner: bool,
    pool_config_path: Option<PathBuf>,
    jdc_config_path: Option<PathBuf>,
    jds_config_path: Option<PathBuf>,
) -> Result<()> {
    info!("═══════════════════════════════════════════════════════");
    info!("  JDC with Pool Mint and JDS - eHash Testing Environment");
    info!("═══════════════════════════════════════════════════════");
    info!("");
    info!("Configuration:");
    info!("  • Pool: Mints eHash tokens");
    info!("  • JDS: Provides job declaration service");
    info!("  • JDC: Acts as proxy (Wallet mode) + correlation tracking");
    info!("  • Protocol: SV2 miners → JDC → Pool");
    info!("");

    let ctx = ScenarioContext::new(test_dir.clone(), with_miner).await?;

    // Generate or load configs
    let pool_config = if let Some(path) = pool_config_path {
        config::read_config::<PoolConfig>(&path).await?
    } else {
        defaults::pool_config(true) // mint_enabled = true
    };

    let jdc_config = if let Some(path) = jdc_config_path {
        config::read_config::<JdcConfig>(&path).await?
    } else {
        defaults::jdc_wallet_config()
    };

    let jds_config = if let Some(path) = jds_config_path {
        config::read_config::<JdsConfig>(&path).await?
    } else {
        defaults::jds_config()
    };

    // Write configs
    let pool_config_file = ctx.config_dir.join("pool.toml");
    config::write_config(&pool_config, &pool_config_file).await?;

    let jdc_config_file = ctx.config_dir.join("jdc.toml");
    config::write_config(&jdc_config, &jdc_config_file).await?;

    let jds_config_file = ctx.config_dir.join("jds.toml");
    config::write_config(&jds_config, &jds_config_file).await?;

    info!("Configuration files:");
    info!("  • Pool: {}", pool_config_file.display());
    info!("  • JDS:  {}", jds_config_file.display());
    info!("  • JDC:  {}", jdc_config_file.display());
    info!("");

    info!("Starting services...");
    info!("");

    let mut pm = ProcessManager::new(test_dir.clone());

    // Start Pool
    let pool_binary = find_binary("pool_sv2")?;
    pm.spawn("pool", &pool_binary, &[], &pool_config_file)
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Start JDS
    let jds_binary = find_binary("jd_server")?;
    pm.spawn("jds", &jds_binary, &[], &jds_config_file)
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Start JDC
    let jdc_binary = find_binary("jd_client_sv2")?;
    pm.spawn("jdc", &jdc_binary, &[], &jdc_config_file)
        .await?;

    info!("");
    info!("Services started successfully!");
    info!("");
    info!("Connection details:");
    info!("  • Pool: {}", pool_config.listen_address);
    info!("  • JDS:  {}", jds_config.listen_address);
    info!("  • JDC:  {}", jdc_config.listen_mining_address);
    info!("");

    // Start miner if requested
    if ctx.with_miner {
        info!("Starting CPU miner...");
        start_cpu_miner(&mut pm, &ctx, &jdc_config.listen_mining_address).await?;
        info!("");
    }

    info!("Logs:");
    info!("  • Pool: {}", ctx.log_dir.join("pool.log").display());
    info!("  • JDS:  {}", ctx.log_dir.join("jds.log").display());
    info!("  • JDC:  {}", ctx.log_dir.join("jdc.log").display());
    if ctx.with_miner {
        info!("  • Miner: {}", ctx.log_dir.join("miner.log").display());
    }
    info!("");

    if let Some(ehash) = &pool_config.ehash {
        info!("eHash Mint (Pool):");
        info!("  • URL: {}", ehash.mint_url);
        info!("  • Database: {}", ctx.db_dir.join("ehash_mint.db").display());
        info!("");
    }

    info!("Use 'ehashimint status' to check process status");
    info!("Use 'ehashimint stop' to stop all services");
    info!("Use Ctrl+C to exit and leave services running");
    info!("");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Received shutdown signal, services will continue running in background");

    Ok(())
}

async fn start_cpu_miner(
    pm: &mut ProcessManager,
    ctx: &ScenarioContext,
    upstream: &str,
) -> Result<()> {
    let miner_binary = find_binary("mining_device")?;

    let args = vec![
        "--pool-address".to_string(),
        upstream.to_string(),
    ];

    let dummy_config = ctx.config_dir.join("miner.toml");
    tokio::fs::write(&dummy_config, "# Miner configuration\n").await?;

    pm.spawn("miner", &miner_binary, &args, &dummy_config)
        .await?;

    Ok(())
}
