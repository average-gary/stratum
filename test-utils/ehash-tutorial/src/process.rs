//! Process monitoring and health checking utilities

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

/// Read the last N lines from a log file
pub fn tail_log_file(path: &Path, lines: usize) -> Result<Vec<String>> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open log file: {}", path.display()))?;

    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader
        .lines()
        .filter_map(|l| l.ok())
        .collect();

    // Return last N lines
    let start = if all_lines.len() > lines {
        all_lines.len() - lines
    } else {
        0
    };

    Ok(all_lines[start..].to_vec())
}

/// Check if a process with given PID is still running
pub fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        let pid = Pid::from_raw(pid as i32);
        // SIGCONT or Signal 0 (null signal) to check if process exists
        kill(pid, Signal::SIGCONT).is_ok() || kill(pid, None).is_ok()
    }

    #[cfg(windows)]
    {
        // TODO: Windows implementation using WinAPI
        // For now, assume it's running
        true
    }
}

/// Check if a TCP port is listening on localhost
#[cfg(unix)]
pub fn is_port_listening(port: u16) -> bool {
    use std::net::TcpStream;
    use std::time::Duration;

    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_millis(500),
    )
    .is_ok()
}

#[cfg(windows)]
pub fn is_port_listening(_port: u16) -> bool {
    // TODO: Windows implementation
    true
}

/// Check health of the Pool process
pub fn check_pool_health(pid: Option<u32>) -> PoolHealth {
    match pid {
        None => PoolHealth::NotStarted,
        Some(pid) => {
            if !is_process_running(pid) {
                PoolHealth::Dead
            } else if is_port_listening(34254) {
                PoolHealth::Healthy
            } else {
                PoolHealth::Starting
            }
        }
    }
}

/// Health status of the Pool process
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolHealth {
    NotStarted,
    Starting,
    Healthy,
    Dead,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::fs;

    #[test]
    fn test_tail_log_file() {
        // Create a temporary log file
        let temp_dir = std::env::temp_dir();
        let log_file = temp_dir.join("test_log.txt");

        {
            let mut file = File::create(&log_file).unwrap();
            for i in 1..=10 {
                writeln!(file, "Line {}", i).unwrap();
            }
        }

        // Test reading last 5 lines
        let lines = tail_log_file(&log_file, 5).unwrap();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "Line 6");
        assert_eq!(lines[4], "Line 10");

        // Test reading more lines than available
        let lines = tail_log_file(&log_file, 20).unwrap();
        assert_eq!(lines.len(), 10);

        // Cleanup
        fs::remove_file(&log_file).ok();
    }

    #[test]
    fn test_is_process_running() {
        // Current process should be running
        let current_pid = std::process::id();
        assert!(is_process_running(current_pid));

        // PID 99999 should not exist
        assert!(!is_process_running(99999));
    }
}
