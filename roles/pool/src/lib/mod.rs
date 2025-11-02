use std::sync::Arc;

use async_channel::unbounded;
use ehash_integration::{config::MintConfig, mint::MintHandler, types::EHashMintData};
use stratum_apps::stratum_core::{
    bitcoin::consensus::Encodable, parsers_sv2::TemplateDistribution,
};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use crate::{
    channel_manager::ChannelManager,
    config::PoolConfig,
    error::PoolResult,
    status::{State, Status},
    task_manager::TaskManager,
    template_receiver::TemplateReceiver,
    utils::ShutdownMessage,
};

pub mod channel_manager;
pub mod config;
pub mod downstream;
pub mod error;
pub mod status;
pub mod task_manager;
pub mod template_receiver;
pub mod utils;

#[derive(Debug, Clone)]
pub struct PoolSv2 {
    config: PoolConfig,
    notify_shutdown: broadcast::Sender<ShutdownMessage>,
}

impl PoolSv2 {
    pub fn new(config: PoolConfig) -> Self {
        let (notify_shutdown, _) = tokio::sync::broadcast::channel::<ShutdownMessage>(100);
        Self {
            config,
            notify_shutdown,
        }
    }

    /// Starts the Pool main loop.
    pub async fn start(&self) -> PoolResult<()> {
        let coinbase_outputs = vec![self.config.get_txout()];
        let mut encoded_outputs = vec![];

        coinbase_outputs
            .consensus_encode(&mut encoded_outputs)
            .expect("Invalid coinbase output in config");

        let notify_shutdown = self.notify_shutdown.clone();

        let task_manager = Arc::new(TaskManager::new());

        let (status_sender, status_receiver) = async_channel::unbounded::<Status>();

        let (channel_manager_to_downstream_sender, _channel_manager_to_downstream_receiver) =
            broadcast::channel(10);
        let (downstream_to_channel_manager_sender, downstream_to_channel_manager_receiver) =
            unbounded();

        let (channel_manager_to_tp_sender, channel_manager_to_tp_receiver) =
            unbounded::<TemplateDistribution<'static>>();
        let (tp_to_channel_manager_sender, tp_to_channel_manager_receiver) =
            unbounded::<TemplateDistribution<'static>>();

        debug!("Channels initialized.");

        // Spawn mint thread - eHash configuration is required for this daemon
        let mint_config = self.config.ehash_mint().ok_or_else(|| {
            crate::error::PoolError::Custom(
                "eHash mint configuration is required. Add [ehash_mint] section to your pool config.".to_string()
            )
        })?;

        info!("Spawning eHash mint thread...");
        let mint_sender = spawn_mint_thread(
            task_manager.clone(),
            mint_config.clone(),
            notify_shutdown.subscribe(),
        )
        .await?;
        info!("eHash mint thread spawned successfully");

        let channel_manager = ChannelManager::new(
            self.config.clone(),
            channel_manager_to_tp_sender,
            tp_to_channel_manager_receiver,
            channel_manager_to_downstream_sender.clone(),
            downstream_to_channel_manager_receiver,
            encoded_outputs.clone(),
            mint_sender,
        )
        .await?;

        let channel_manager_clone = channel_manager.clone();

        // Initialize the template Receiver
        let tp_address = self.config.tp_address().to_string();
        let tp_pubkey = self.config.tp_authority_public_key().copied();

        let template_receiver = TemplateReceiver::new(
            tp_address.clone(),
            tp_pubkey,
            channel_manager_to_tp_receiver,
            tp_to_channel_manager_sender,
            notify_shutdown.clone(),
            task_manager.clone(),
            status_sender.clone(),
        )
        .await?;

        info!("Template provider setup done");

        template_receiver
            .start(
                tp_address,
                notify_shutdown.clone(),
                status_sender.clone(),
                task_manager.clone(),
                encoded_outputs,
            )
            .await?;

        channel_manager
            .start(
                notify_shutdown.clone(),
                status_sender.clone(),
                task_manager.clone(),
            )
            .await?;

        channel_manager_clone
            .start_downstream_server(
                *self.config.authority_public_key(),
                *self.config.authority_secret_key(),
                self.config.cert_validity_sec(),
                *self.config.listen_address(),
                task_manager.clone(),
                notify_shutdown.clone(),
                status_sender,
                downstream_to_channel_manager_sender,
                channel_manager_to_downstream_sender,
            )
            .await?;

        info!("Spawning status listener task...");
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Ctrl+C received — initiating graceful shutdown...");
                    let _ = notify_shutdown.send(ShutdownMessage::ShutdownAll);
                    break;
                }
                message = status_receiver.recv() => {
                    if let Ok(status) = message {
                        match status.state {
                            State::DownstreamShutdown{downstream_id,..} => {
                                warn!("Downstream {downstream_id:?} disconnected — Channel manager.");
                                let _ = notify_shutdown.send(ShutdownMessage::DownstreamShutdown(downstream_id));
                            }
                            State::TemplateReceiverShutdown(_) => {
                                warn!("Template Receiver shutdown requested — initiating full shutdown.");
                                let _ = notify_shutdown.send(ShutdownMessage::ShutdownAll);
                                break;
                            }
                            State::ChannelManagerShutdown(_) => {
                                warn!("Channel Manager shutdown requested — initiating full shutdown.");
                                let _ = notify_shutdown.send(ShutdownMessage::ShutdownAll);
                                break;
                            }
                        }
                    }
                }
            }
        }

        warn!("Graceful shutdown initiated");

        // Wait for tasks to complete gracefully with a timeout
        info!("Waiting for tasks to complete gracefully...");
        let graceful_shutdown = tokio::time::timeout(
            tokio::time::Duration::from_secs(10),
            task_manager.join_all()
        );

        match graceful_shutdown.await {
            Ok(_) => {
                info!("All tasks completed gracefully");
            }
            Err(_) => {
                warn!("Graceful shutdown timeout exceeded, aborting remaining tasks...");
                task_manager.abort_all().await;
                info!("Joining aborted tasks...");
                task_manager.join_all().await;
            }
        }

        info!("Pool shutdown complete.");
        Ok(())
    }
}

impl Drop for PoolSv2 {
    fn drop(&mut self) {
        info!("PoolSv2 dropped");
        let _ = self.notify_shutdown.send(ShutdownMessage::ShutdownAll);
    }
}

/// Spawns a mint thread for eHash token minting with optional HTTP API server
///
/// Creates a MintHandler instance and spawns it as a dedicated async task managed by
/// the TaskManager. The mint thread runs independently of mining operations and
/// processes EHashMintData events sent via the returned async channel.
///
/// If HTTP API is enabled in the config, this function also starts an HTTP server
/// running concurrently with the mint handler in the same task using tokio::select!.
/// This ensures the HTTP server shares the same CDK Mint instance and doesn't affect
/// mining operations if it fails.
///
/// # Arguments
/// * `task_manager` - TaskManager to register the mint thread with
/// * `config` - MintConfig containing mint settings and HTTP API configuration
/// * `shutdown_rx` - Shutdown signal receiver for graceful shutdown
///
/// # Returns
/// Returns the sender channel for EHashMintData events on success
pub async fn spawn_mint_thread(
    task_manager: Arc<TaskManager>,
    config: MintConfig,
    mut shutdown_rx: broadcast::Receiver<ShutdownMessage>,
) -> PoolResult<async_channel::Sender<EHashMintData>> {
    info!("Initializing eHash mint handler...");

    let mut mint_handler = MintHandler::new(config.clone()).await.map_err(|e| {
        crate::error::PoolError::Custom(format!("Failed to initialize mint handler: {}", e))
    })?;

    let sender = mint_handler.get_sender();

    // Get reference to the Mint instance for HTTP server
    let mint = mint_handler.mint();

    // Create an async_channel for shutdown signal conversion
    let (shutdown_tx, shutdown_rx_async) = async_channel::bounded::<()>(1);

    // Spawn a task to convert broadcast shutdown to async_channel
    task_manager.spawn(async move {
        if shutdown_rx.recv().await.is_ok() {
            let _ = shutdown_tx.send(()).await;
        }
    });

    // Create HTTP API server - always required for eHash
    let bind_address = config.http_api.bind_address;
    info!("Starting HTTP API server on {}", bind_address);

    // Create the CDK Axum router with eHash NUT-20 extensions
    let router = cdk_axum::create_mint_router(mint, false)
        .await
        .map_err(|e| {
            crate::error::PoolError::Custom(format!("Failed to create HTTP router: {}", e))
        })?;

    // Create TCP listener
    let listener = tokio::net::TcpListener::bind(bind_address)
        .await
        .map_err(|e| {
            crate::error::PoolError::Custom(format!(
                "Failed to bind HTTP server to {}: {}",
                bind_address, e
            ))
        })?;

    info!("HTTP API server listening on {}", bind_address);

    info!("Spawning mint handler task with HTTP server...");
    task_manager.spawn(async move {
        // Run both mint handler and HTTP server concurrently
        tokio::select! {
            result = mint_handler.run_with_shutdown(shutdown_rx_async) => {
                if let Err(e) = result {
                    warn!("Mint handler error: {}", e);
                }
                info!("Mint handler task completed");
            }
            result = axum::serve(listener, router) => {
                if let Err(e) = result {
                    warn!("HTTP server error: {}", e);
                }
                info!("HTTP server task completed");
            }
        }
    });

    info!("eHash mint handler initialized successfully");
    Ok(sender)
}
