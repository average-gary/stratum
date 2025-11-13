//! ehashimint - eHash testing environment library
//!
//! This library provides process management and configuration utilities
//! for setting up local eHash testing environments.

pub mod process;
pub mod config;
mod scenarios;

// Re-export commonly used types
pub use process::{ProcessManager, ManagedProcess, ProcessStatus};
pub use config::{
    PoolConfig, TProxyConfig, JdcConfig, JdsConfig,
    EHashMintConfig, EHashWalletConfig,
    write_config, read_config,
};

// Re-export scenario helpers
pub use scenarios::{ScenarioContext, find_binary};
