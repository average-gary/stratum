pub mod config;
pub mod downstream;
pub mod error;
pub mod job_declarator;
pub mod status;
pub mod template_receiver;
pub mod upstream_sv2;

use std::{sync::atomic::AtomicBool, time::Duration};

use async_channel::unbounded;
use config::JobDeclaratorClientConfig;
use futures::{select, FutureExt};
use job_declarator::JobDeclarator;
use roles_logic_sv2::utils::Mutex;
use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
use tokio::{sync::Notify, task::AbortHandle};

use tracing::{error, info};

/// Is used by the template receiver and the downstream. When a NewTemplate is received the context
/// that is running the template receiver set this value to false and then the message is sent to
/// the context that is running the Downstream that do something and then set it back to true.
///
/// In the meantime if the context that is running the template receiver receives a SetNewPrevHash
/// it wait until the value of this global is true before doing anything.
///
/// Acquire and Release memory ordering is used.
///
/// Memory Ordering Explanation:
/// We use Acquire-Release ordering instead of SeqCst or Relaxed for the following reasons:
/// 1. Acquire in template receiver context ensures we see all operations before the Release store
///    the downstream.
/// 2. Within the same execution context (template receiver), a Relaxed store followed by an Acquire
///    load is sufficient. This is because operations within the same context execute in the order
///    they appear in the code.
/// 3. The combination of Release in downstream and Acquire in template receiver contexts
///    establishes a happens-before relationship, guaranteeing that we handle the SetNewPrevHash
///    message after that downstream have finished handling the NewTemplate.
/// 3. SeqCst is overkill we only need to synchronize two contexts, a globally agreed-upon order
///    between all the contexts is not necessary.
pub static IS_NEW_TEMPLATE_HANDLED: AtomicBool = AtomicBool::new(true);

/// Job Declarator Client (or JDC) is the role which is Miner-side, in charge of creating new
/// mining jobs from the templates received by the Template Provider to which it is connected. It
/// declares custom jobs to the JDS, in order to start working on them.
/// JDC is also responsible for putting in action the Pool-fallback mechanism, automatically
/// switching to backup Pools in case of declared custom jobs refused by JDS (which is Pool side).
/// As a solution of last-resort, it is able to switch to Solo Mining until new safe Pools appear
/// in the market.
#[derive(Debug, Clone)]
pub struct JobDeclaratorClient {
    /// Configuration of the proxy server [`JobDeclaratorClient`] is connected to.
    config: JobDeclaratorClientConfig,
    // Used for notifying the [`JobDeclaratorClient`] to shutdown gracefully.
    shutdown: Arc<Notify>,
}

impl JobDeclaratorClient {
    /// Instantiate JDC instance.
    pub fn new(config: JobDeclaratorClientConfig) -> Self {
        Self {
            config,
            shutdown: Arc::new(Notify::new()),
        }
    }

    pub async fn start(self) {
        let mut upstream_index = 0;

        // Channel used to manage failed tasks
        let (tx_status, rx_status) = unbounded();

        let task_collector = Arc::new(Mutex::new(vec![]));

        tokio::spawn({
            let shutdown_signal = self.shutdown.clone();
            async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    info!("Interrupt received");
                    shutdown_signal.notify_one();
                }
            }
        });

        let config = self.config;
        'outer: loop {
            let task_collector = task_collector.clone();
            let tx_status = tx_status.clone();
            let config = config.clone();
            let root_handler;
            if let Some(upstream) = config.upstreams().get(upstream_index) {
                let tx_status = tx_status.clone();
                let task_collector = task_collector.clone();
                let upstream = upstream.clone();
                root_handler = tokio::spawn(async move {
                    Self::initialize_jd(config, tx_status, task_collector, upstream).await;
                });
            } else {
                let tx_status = tx_status.clone();
                let task_collector = task_collector.clone();
                root_handler = tokio::spawn(async move {
                    Self::initialize_jd_as_solo_miner(
                        config,
                        tx_status.clone(),
                        task_collector.clone(),
                    )
                    .await;
                });
            }
            // Check all tasks if is_finished() is true, if so exit
            loop {
                select! {
                    task_status = rx_status.recv().fuse() => {
                        if let Ok(task_status) = task_status {
                            match task_status.state {
                                // Should only be sent by the downstream listener
                                status::State::DownstreamShutdown(err) => {
                                    error!("SHUTDOWN from: {}", err);
                                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                    task_collector
                                        .safe_lock(|s| {
                                            for handle in s {
                                                handle.abort();
                                            }
                                        })
                                        .unwrap();
                                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                    break;
                                }
                                status::State::UpstreamShutdown(err) => {
                                    error!("SHUTDOWN from: {}", err);
                                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                    task_collector
                                        .safe_lock(|s| {
                                            for handle in s {
                                                handle.abort();
                                            }
                                        })
                                        .unwrap();
                                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                    break;
                                }
                                status::State::UpstreamRogue => {
                                    error!("Changing Pool");
                                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                    task_collector
                                        .safe_lock(|s| {
                                            for handle in s {
                                                handle.abort();
                                            }
                                        })
                                        .unwrap();
                                    upstream_index += 1;
                                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                    break;
                                }
                                status::State::Healthy(msg) => {
                                    info!("HEALTHY message: {}", msg);
                                }
                            }
                        } else {
                            info!("Received unknown task. Shutting down.");
                            task_collector
                                .safe_lock(|s| {
                                    for handle in s {
                                        handle.abort();
                                    }
                                })
                                .unwrap();
                            root_handler.abort();
                            break 'outer;
                        }
                    },
                    _ = self.shutdown.notified().fuse() => {
                        info!("Shutting down gracefully...");
                        task_collector
                            .safe_lock(|s| {
                                for handle in s {
                                    handle.abort();
                                }
                            })
                            .unwrap();
                        // root_handler.abort();
                        break 'outer;
                    }
                };
            }
        }
    }

    async fn initialize_jd_as_solo_miner(
        config: JobDeclaratorClientConfig,
        tx_status: async_channel::Sender<status::Status<'static>>,
        task_collector: Arc<Mutex<Vec<AbortHandle>>>,
    ) {
        let miner_tx_out = config.get_txout().expect("Failed to get txout");

        // Wait for downstream to connect
        let downstream_handle = tokio::spawn(downstream::listen_for_downstream_mining(
            *config.listening_address(),
            None,
            config.withhold(),
            *config.authority_public_key(),
            *config.authority_secret_key(),
            config.cert_validity_sec(),
            task_collector.clone(),
            tx_status.clone(),
            miner_tx_out.clone(),
            None,
            config.clone(),
        ));
        let _ = task_collector.safe_lock(|e| {
            e.push(downstream_handle.abort_handle());
        });
    }

    async fn initialize_jd(
        config: JobDeclaratorClientConfig,
        tx_status: async_channel::Sender<status::Status<'static>>,
        task_collector: Arc<Mutex<Vec<AbortHandle>>>,
        upstream_config: config::Upstream,
    ) {
        let timeout = config.timeout();

        // Format `Upstream` connection address
        let mut parts = upstream_config.pool_address.split(':');
        let address = parts
            .next()
            .unwrap_or_else(|| panic!("Invalid pool address {}", upstream_config.pool_address));
        let port = parts
            .next()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or_else(|| panic!("Invalid pool address {}", upstream_config.pool_address));
        let upstream_addr = SocketAddr::new(
            IpAddr::from_str(address).unwrap_or_else(|_| {
                panic!("Invalid pool address {}", upstream_config.pool_address)
            }),
            port,
        );

        // Instantiate a new `Upstream` (SV2 Pool)
        let upstream = match upstream_sv2::Upstream::new(
            upstream_addr,
            upstream_config.authority_pubkey,
            0, // TODO
            upstream_config.pool_signature.clone(),
            status::Sender::Upstream(tx_status.clone()),
            task_collector.clone(),
            Arc::new(Mutex::new(PoolChangerTrigger::new(timeout))),
        )
        .await
        {
            Ok(upstream) => upstream,
            Err(e) => {
                error!("Failed to create upstream: {}", e);
                panic!()
            }
        };

        match upstream_sv2::Upstream::setup_connection(
            upstream.clone(),
            config.min_supported_version(),
            config.max_supported_version(),
        )
        .await
        {
            Ok(_) => info!("Connected to Upstream!"),
            Err(e) => {
                error!("Failed to connect to Upstream EXITING! : {}", e);
                panic!()
            }
        }

        // Start receiving messages from the SV2 Upstream role
        if let Err(e) = upstream_sv2::Upstream::parse_incoming(upstream.clone()) {
            error!("failed to create sv2 parser: {}", e);
            panic!()
        }

        let mut parts = upstream_config.jd_address.split(':');
        let ip_jd = parts.next().unwrap().to_string();
        let port_jd = parts.next().unwrap().parse::<u16>().unwrap();
        let jd = match JobDeclarator::new(
            SocketAddr::new(IpAddr::from_str(ip_jd.as_str()).unwrap(), port_jd),
            upstream_config.authority_pubkey.into_bytes(),
            config.clone(),
            upstream.clone(),
            task_collector.clone(),
        )
        .await
        {
            Ok(c) => c,
            Err(e) => {
                let _ = tx_status
                    .send(status::Status {
                        state: status::State::UpstreamShutdown(e),
                    })
                    .await;
                return;
            }
        };

        // Wait for downstream to connect
        let downstream_handle = tokio::spawn(downstream::listen_for_downstream_mining(
            *config.listening_address(),
            Some(upstream),
            config.withhold(),
            *config.authority_public_key(),
            *config.authority_secret_key(),
            config.cert_validity_sec(),
            task_collector.clone(),
            tx_status.clone(),
            vec![],
            Some(jd),
            config.clone(),
        ));
        let _ = task_collector.safe_lock(|e| {
            e.push(downstream_handle.abort_handle());
        });
    }

    /// Closes JDC role and any open connection associated with it.
    ///
    /// Note that this method will result in a full exit of the  running
    /// jd-client and any open connection most be re-initiated upon new
    /// start.
    #[allow(dead_code)]
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }
}

#[derive(Debug)]
pub struct PoolChangerTrigger {
    timeout: Duration,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl PoolChangerTrigger {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            task: None,
        }
    }

    pub fn start(&mut self, sender: status::Sender) {
        let timeout = self.timeout;
        let task = tokio::task::spawn(async move {
            tokio::time::sleep(timeout).await;
            let _ = sender
                .send(status::Status {
                    state: status::State::UpstreamRogue,
                })
                .await;
        });
        self.task = Some(task);
    }

    pub fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use ext_config::{Config, File, FileFormat};

    use crate::*;

    #[tokio::test]
    async fn test_shutdown() {
        let config_path = "config-examples/jdc-config-hosted-example.toml";
        let config: JobDeclaratorClientConfig = match Config::builder()
            .add_source(File::new(config_path, FileFormat::Toml))
            .build()
        {
            Ok(settings) => match settings.try_deserialize::<JobDeclaratorClientConfig>() {
                Ok(c) => c,
                Err(e) => {
                    dbg!(&e);
                    return;
                }
            },
            Err(e) => {
                dbg!(&e);
                return;
            }
        };
        let jdc = JobDeclaratorClient::new(config.clone());
        let cloned = jdc.clone();
        tokio::spawn(async move {
            cloned.start().await;
        });
        jdc.shutdown();
        let ip = config.listening_address().ip();
        let port = config.listening_address().port();
        let jdc_addr = format!("{}:{}", ip, port);
        assert!(std::net::TcpListener::bind(jdc_addr).is_ok());
    }
}
