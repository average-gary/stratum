use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tracing::{debug, info, warn};

/// Represents a managed process in the eHash testing environment
pub struct ManagedProcess {
    pub name: String,
    pub child: Child,
    pub log_file: PathBuf,
    pub pid_file: PathBuf,
}

impl ManagedProcess {
    /// Create a new managed process
    pub async fn spawn(
        name: &str,
        binary: &str,
        args: &[String],
        config_path: &Path,
        log_dir: &Path,
        pid_dir: &Path,
    ) -> Result<Self> {
        let log_file = log_dir.join(format!("{}.log", name));
        let pid_file = pid_dir.join(format!("{}.pid", name));

        // Create log file
        let log_handle = File::create(&log_file)
            .await
            .with_context(|| format!("Failed to create log file: {}", log_file.display()))?;

        // Convert to std::fs::File for Stdio
        let std_log = log_handle.into_std().await;
        let stdout_log = std_log
            .try_clone()
            .with_context(|| format!("Failed to clone log file handle"))?;

        // Build command
        let mut cmd = Command::new(binary);
        cmd.arg("-c")
            .arg(config_path)
            .args(args)
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(std_log))
            .stdin(Stdio::null());

        debug!("Starting {} with config: {}", name, config_path.display());

        // Spawn process
        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn {}", name))?;

        // Write PID file
        if let Some(pid) = child.id() {
            let mut pid_file_handle = File::create(&pid_file).await?;
            pid_file_handle
                .write_all(pid.to_string().as_bytes())
                .await?;
            pid_file_handle.flush().await?;
        }

        info!(
            "Started {} (PID: {:?}), logging to: {}",
            name,
            child.id(),
            log_file.display()
        );

        Ok(Self {
            name: name.to_string(),
            child,
            log_file,
            pid_file,
        })
    }

    /// Kill the process and clean up PID file
    pub async fn kill(&mut self) -> Result<()> {
        info!("Stopping {}", self.name);

        match self.child.kill().await {
            Ok(()) => {
                // Remove PID file
                if tokio::fs::remove_file(&self.pid_file).await.is_err() {
                    warn!("Failed to remove PID file: {}", self.pid_file.display());
                }
                Ok(())
            }
            Err(e) => {
                warn!("Error killing {}: {}", self.name, e);
                Err(e.into())
            }
        }
    }

    /// Wait for process to exit
    pub async fn wait(&mut self) -> Result<()> {
        let status = self
            .child
            .wait()
            .await
            .with_context(|| format!("Failed to wait for {}", self.name))?;

        if status.success() {
            info!("{} exited successfully", self.name);
            Ok(())
        } else {
            bail!("{} exited with status: {}", self.name, status);
        }
    }
}

/// Process manager for multiple services
pub struct ProcessManager {
    pub processes: Vec<ManagedProcess>,
    pub test_dir: PathBuf,
}

impl ProcessManager {
    pub fn new(test_dir: PathBuf) -> Self {
        Self {
            processes: Vec::new(),
            test_dir,
        }
    }

    /// Spawn a new managed process
    pub async fn spawn(
        &mut self,
        name: &str,
        binary: &str,
        args: &[String],
        config_path: &Path,
    ) -> Result<()> {
        let log_dir = self.test_dir.join("logs");
        let pid_dir = self.test_dir.join("pids");

        tokio::fs::create_dir_all(&log_dir).await?;
        tokio::fs::create_dir_all(&pid_dir).await?;

        let process =
            ManagedProcess::spawn(name, binary, args, config_path, &log_dir, &pid_dir).await?;

        self.processes.push(process);
        Ok(())
    }

    /// Stop all managed processes
    pub async fn stop_all(&mut self) -> Result<()> {
        info!("Stopping all {} processes", self.processes.len());

        for process in self.processes.iter_mut() {
            let _ = process.kill().await;
        }

        self.processes.clear();
        Ok(())
    }

    /// Wait for all processes to exit
    pub async fn wait_all(&mut self) -> Result<()> {
        for process in self.processes.iter_mut() {
            process.wait().await?;
        }
        Ok(())
    }

    /// Get status of all processes
    pub async fn status(&self) -> Vec<ProcessStatus> {
        let mut statuses = Vec::new();

        for process in &self.processes {
            let status = if let Some(pid) = process.child.id() {
                // Check if process is still running
                if is_process_running(pid) {
                    ProcessStatus::Running {
                        name: process.name.clone(),
                        pid,
                        log_file: process.log_file.clone(),
                    }
                } else {
                    ProcessStatus::Dead {
                        name: process.name.clone(),
                        log_file: process.log_file.clone(),
                    }
                }
            } else {
                ProcessStatus::Unknown {
                    name: process.name.clone(),
                }
            };

            statuses.push(status);
        }

        statuses
    }
}

#[derive(Debug)]
pub enum ProcessStatus {
    Running {
        name: String,
        pid: u32,
        log_file: PathBuf,
    },
    Dead {
        name: String,
        log_file: PathBuf,
    },
    Unknown {
        name: String,
    },
}

/// Check if a process is running by PID
fn is_process_running(pid: u32) -> bool {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let pid = Pid::from_raw(pid as i32);
    kill(pid, Signal::SIGCONT).is_ok() || kill(pid, None).is_ok()
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        // Best-effort cleanup
        for process in self.processes.iter_mut() {
            if let Some(pid) = process.child.id() {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
            }
        }
    }
}
