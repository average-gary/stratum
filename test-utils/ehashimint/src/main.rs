use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod cli;
mod config;
mod process;
mod scenarios;

#[derive(Parser)]
#[command(name = "ehashimint")]
#[command(version, about = "eHash testing environment manager", long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Command,

    /// Test directory for logs, configs, and databases
    #[clap(short = 'd', long, env = "EHASH_TEST_DIR")]
    test_dir: Option<PathBuf>,

    /// Verbose logging
    #[clap(short = 'v', long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run TProxy with Pool Mint configuration (simplest setup)
    TProxyPool {
        /// Automatically start a CPU miner
        #[clap(long)]
        with_miner: bool,

        /// Custom pool configuration file
        #[clap(long)]
        pool_config: Option<PathBuf>,

        /// Custom TProxy configuration file
        #[clap(long)]
        tproxy_config: Option<PathBuf>,
    },

    /// Run TProxy with JDC Mint and JDS configuration
    TProxyJdc {
        /// Automatically start a CPU miner
        #[clap(long)]
        with_miner: bool,

        /// Custom pool configuration file
        #[clap(long)]
        pool_config: Option<PathBuf>,

        /// Custom TProxy configuration file
        #[clap(long)]
        tproxy_config: Option<PathBuf>,

        /// Custom JDC configuration file
        #[clap(long)]
        jdc_config: Option<PathBuf>,

        /// Custom JDS configuration file
        #[clap(long)]
        jds_config: Option<PathBuf>,
    },

    /// Run JDC with Pool Mint and JDS configuration
    JdcPool {
        /// Automatically start a CPU miner
        #[clap(long)]
        with_miner: bool,

        /// Custom pool configuration file
        #[clap(long)]
        pool_config: Option<PathBuf>,

        /// Custom JDC configuration file
        #[clap(long)]
        jdc_config: Option<PathBuf>,

        /// Custom JDS configuration file
        #[clap(long)]
        jds_config: Option<PathBuf>,
    },

    /// Clean up test directories and stop all processes
    Clean {
        /// Force cleanup without confirmation
        #[clap(long)]
        force: bool,
    },

    /// Show status of running processes
    Status,

    /// Stop all running processes
    Stop,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Set up logging
    let log_level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    // Determine test directory
    let test_dir = args.test_dir.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("ehashimint-{}", std::process::id()))
    });

    info!("Using test directory: {}", test_dir.display());

    // Handle commands
    match args.command {
        Command::TProxyPool {
            with_miner,
            pool_config,
            tproxy_config,
        } => {
            scenarios::tproxy_pool::run(test_dir, with_miner, pool_config, tproxy_config).await?;
        }
        Command::TProxyJdc {
            with_miner,
            pool_config,
            tproxy_config,
            jdc_config,
            jds_config,
        } => {
            scenarios::tproxy_jdc::run(
                test_dir,
                with_miner,
                pool_config,
                tproxy_config,
                jdc_config,
                jds_config,
            )
            .await?;
        }
        Command::JdcPool {
            with_miner,
            pool_config,
            jdc_config,
            jds_config,
        } => {
            scenarios::jdc_pool::run(test_dir, with_miner, pool_config, jdc_config, jds_config)
                .await?;
        }
        Command::Clean { force } => {
            cli::clean(test_dir, force).await?;
        }
        Command::Status => {
            cli::status(test_dir).await?;
        }
        Command::Stop => {
            cli::stop(test_dir).await?;
        }
    }

    Ok(())
}
