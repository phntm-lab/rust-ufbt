use std::io;

/// Failure of ARM GCC toolchain deployment.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ToolchainError {
    /// An external tool could not be started.
    #[error("Failed to run {program}")]
    Process {
        /// Program that could not be started.
        program: String,
        /// Underlying operating system error.
        #[source]
        source: io::Error,
    },
    /// A file system operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}
