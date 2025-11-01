use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::{info, warn};

/// Clean up test directories
pub async fn clean(test_dir: PathBuf, force: bool) -> Result<()> {
    if !test_dir.exists() {
        info!("Test directory does not exist: {}", test_dir.display());
        return Ok(());
    }

    if !force {
        println!("This will delete: {}", test_dir.display());
        println!("Are you sure? (y/N)");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            info!("Cleanup cancelled");
            return Ok(());
        }
    }

    // Stop any running processes first
    stop(test_dir.clone()).await?;

    // Remove test directory
    tokio::fs::remove_dir_all(&test_dir)
        .await
        .with_context(|| format!("Failed to remove test directory: {}", test_dir.display()))?;

    info!("Cleaned up test directory: {}", test_dir.display());
    Ok(())
}

/// Show status of running processes
pub async fn status(test_dir: PathBuf) -> Result<()> {
    let pid_dir = test_dir.join("pids");

    if !pid_dir.exists() {
        info!("No running processes found");
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&pid_dir).await?;
    let mut found_processes = false;

    println!("Process Status:");
    println!("═══════════════════════════════════════════════════════");

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("pid") {
            found_processes = true;
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            if let Ok(pid_str) = tokio::fs::read_to_string(&path).await {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    let status = if is_process_running(pid) {
                        format!("✓ Running (PID: {})", pid)
                    } else {
                        "✗ Dead".to_string()
                    };

                    println!("{:<20} {}", name, status);
                }
            }
        }
    }

    if !found_processes {
        info!("No running processes found");
    }

    Ok(())
}

/// Stop all running processes
pub async fn stop(test_dir: PathBuf) -> Result<()> {
    let pid_dir = test_dir.join("pids");

    if !pid_dir.exists() {
        info!("No running processes to stop");
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&pid_dir).await?;
    let mut stopped = 0;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("pid") {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            if let Ok(pid_str) = tokio::fs::read_to_string(&path).await {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    if kill_process(pid) {
                        info!("Stopped {}", name);
                        stopped += 1;
                    } else {
                        warn!("Failed to stop {} (PID: {})", name, pid);
                    }

                    // Remove PID file
                    let _ = tokio::fs::remove_file(&path).await;
                }
            }
        }
    }

    if stopped > 0 {
        info!("Stopped {} process(es)", stopped);
    } else {
        info!("No running processes to stop");
    }

    Ok(())
}

/// Check if a process is running by PID
fn is_process_running(pid: u32) -> bool {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let pid = Pid::from_raw(pid as i32);
    kill(pid, Signal::SIGCONT).is_ok() || kill(pid, None).is_ok()
}

/// Kill a process by PID
fn kill_process(pid: u32) -> bool {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let pid = Pid::from_raw(pid as i32);
    kill(pid, Signal::SIGTERM).is_ok()
}
