//! ARM GCC toolchain deployment.

#[cfg(not(windows))]
mod deploy_unix;
#[cfg(windows)]
mod deploy_windows;
mod deployer;
mod error;
mod info;

pub use deployer::ToolchainDeployer;
pub use error::ToolchainError;
pub use info::ToolchainInfo;
