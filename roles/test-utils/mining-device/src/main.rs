#![allow(special_module_name)]
#![allow(clippy::option_map_unit_fn)]
use key_utils::Secp256k1PublicKey;

use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        help = "Pool pub key, when left empty the pool certificate is not checked"
    )]
    pubkey_pool: Option<Secp256k1PublicKey>,
    #[arg(
        short,
        long,
        help = "Sometimes used by the pool to identify the device"
    )]
    id_device: Option<String>,
    #[arg(
        short,
        long,
        help = "Address of the pool in this format ip:port or domain:port (TCP connection)",
        conflicts_with = "pool_iroh_node_id"
    )]
    address_pool: Option<String>,
    #[arg(
        long,
        help = "This value is used to slow down the cpu miner, it represents the number of micro-seconds that are awaited between hashes",
        default_value = "0"
    )]
    handicap: u32,
    #[arg(
        long,
        help = "User id, used when a new channel is opened, it can be used by the pool to identify the miner"
    )]
    id_user: Option<String>,
    #[arg(
        long,
        help = "This floating point number is used to modify the advertised nominal hashrate when opening a channel with the upstream.\
         \nIf 0.0 < nominal_hashrate_multiplier < 1.0, the CPU miner will advertise a nominal hashrate that is smaller than its real capacity.\
         \nIf nominal_hashrate_multiplier > 1.0, the CPU miner will advertise a nominal hashrate that is bigger than its real capacity.\
         \nIf empty, the CPU miner will simply advertise its real capacity."
    )]
    nominal_hashrate_multiplier: Option<f32>,
    #[arg(
        long,
        help = "Number of nonces to try per mining loop iteration when fast hashing is available (micro-batching)",
        default_value = "32"
    )]
    nonces_per_call: u32,
    #[arg(
        long,
        help = "Number of worker threads to use for mining. Defaults to logical CPUs minus one (leaves one core free)."
    )]
    cores: Option<u32>,

    // Iroh P2P transport options (requires --features iroh)
    #[cfg(feature = "iroh")]
    #[arg(
        long,
        help = "Pool's Iroh NodeId (base32-encoded, for P2P connection)",
        conflicts_with = "address_pool"
    )]
    pool_iroh_node_id: Option<String>,
    #[cfg(feature = "iroh")]
    #[arg(
        long,
        help = "ALPN protocol identifier for Iroh connection",
        default_value = "sv2-m",
        requires = "pool_iroh_node_id"
    )]
    pool_iroh_alpn: String,
    #[cfg(feature = "iroh")]
    #[arg(
        long,
        help = "Path to save/load Iroh secret key (for stable NodeId across restarts)"
    )]
    iroh_secret_key_path: Option<std::path::PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    tracing_subscriber::fmt::init();
    info!("start");

    // Configure micro-batch size
    mining_device::set_nonces_per_call(args.nonces_per_call);
    // Optional override of worker threads
    if let Some(n) = args.cores {
        mining_device::set_cores(n);
    }
    // Log worker usage (after applying overrides)
    let used = mining_device::effective_worker_count();
    let total = mining_device::total_logical_cpus();
    info!(
        "Using {} worker threads out of {} logical CPUs",
        used, total
    );

    // Determine transport type and connect
    #[cfg(feature = "iroh")]
    {
        if let Some(node_id) = args.pool_iroh_node_id {
            info!("Using Iroh P2P transport to connect to Pool");
            let _ = mining_device::connect_iroh(
                node_id,
                args.pool_iroh_alpn,
                args.iroh_secret_key_path,
                args.pubkey_pool,
                args.id_device,
                args.id_user,
                args.handicap,
                args.nominal_hashrate_multiplier,
                false,
            )
            .await;
            return;
        }
    }

    // TCP connection (default or when --address-pool is provided)
    let address = args.address_pool.expect("Either --address-pool or --pool-iroh-node-id must be provided");
    info!("Using TCP transport to connect to Pool");
    let _ = mining_device::connect(
        address,
        args.pubkey_pool,
        args.id_device,
        args.id_user,
        args.handicap,
        args.nominal_hashrate_multiplier,
        false,
    )
    .await;
}
