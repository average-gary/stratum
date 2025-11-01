use std::path::PathBuf;

use anyhow::Result;
use tracing::info;

use crate::config::{self, defaults, PoolConfig, TProxyConfig};
use crate::process::ProcessManager;
use crate::scenarios::{ScenarioContext, find_binary};

/// Run TProxy with Pool Mint configuration
///
/// This is the simplest eHash setup:
/// - Pool mints eHash tokens
/// - TProxy translates SV1→SV2 and tracks correlation
/// - SV1 miners connect to TProxy
pub async fn run(
    test_dir: PathBuf,
    with_miner: bool,
    pool_config_path: Option<PathBuf>,
    tproxy_config_path: Option<PathBuf>,
) -> Result<()> {
    info!("═══════════════════════════════════════════════════════");
    info!("  TProxy with Pool Mint - eHash Testing Environment");
    info!("═══════════════════════════════════════════════════════");
    info!("");
    info!("Configuration:");
    info!("  • Pool: Mints eHash tokens");
    info!("  • TProxy: SV1→SV2 translation + correlation tracking");
    info!("  • Protocol: SV1 miners → TProxy → Pool");
    info!("");

    let ctx = ScenarioContext::new(test_dir.clone(), with_miner).await?;

    // Generate or load Pool config
    let pool_config = if let Some(path) = pool_config_path {
        info!("Loading Pool config from: {}", path.display());
        config::read_config::<PoolConfig>(&path).await?
    } else {
        defaults::pool_config(true) // mint_enabled = true
    };

    let pool_config_file = ctx.config_dir.join("pool.toml");
    config::write_config(&pool_config, &pool_config_file).await?;
    info!("Pool config: {}", pool_config_file.display());

    // Generate or load TProxy config
    let tproxy_config = if let Some(path) = tproxy_config_path {
        info!("Loading TProxy config from: {}", path.display());
        config::read_config::<TProxyConfig>(&path).await?
    } else {
        defaults::tproxy_config()
    };

    let tproxy_config_file = ctx.config_dir.join("tproxy.toml");
    config::write_config(&tproxy_config, &tproxy_config_file).await?;
    info!("TProxy config: {}", tproxy_config_file.display());

    info!("");
    info!("Starting services...");
    info!("");

    let mut pm = ProcessManager::new(test_dir.clone());

    // Start Pool
    let pool_binary = find_binary("pool_sv2")?;
    pm.spawn("pool", &pool_binary, &[], &pool_config_file)
        .await?;

    // Wait a bit for Pool to start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Start TProxy
    let tproxy_binary = find_binary("translator_sv2")?;
    pm.spawn("tproxy", &tproxy_binary, &[], &tproxy_config_file)
        .await?;

    info!("");
    info!("Services started successfully!");
    info!("");
    info!("Connection details:");
    info!("  • Pool:   {}", pool_config.listen_address);
    info!("  • TProxy: {}", tproxy_config.listening_address);
    info!("");

    // Start miner if requested
    if ctx.with_miner {
        info!("Starting CPU miner...");
        start_cpu_miner(&mut pm, &ctx, &tproxy_config.listening_address).await?;
        info!("");
    }

    info!("Logs:");
    info!("  • Pool:   {}", ctx.log_dir.join("pool.log").display());
    info!("  • TProxy: {}", ctx.log_dir.join("tproxy.log").display());
    if ctx.with_miner {
        info!("  • Miner:  {}", ctx.log_dir.join("miner.log").display());
    }
    info!("");

    if let Some(ehash) = &pool_config.ehash {
        info!("eHash Mint:");
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

    // Parse upstream address
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
