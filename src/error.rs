use std::io;

use crate::build::{BuildError, BundleError, ElfError, FamParseError, IconError, ManifestError};
use crate::net::FetchError;
use crate::sdk::SdkError;
use crate::toolchain::ToolchainError;

/// Any failure the crate can report.
///
/// Individual operations return the narrowest error type that fits; this aggregate exists
/// for callers that drive several of them and want a single error type to propagate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An HTTP download failed.
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// SDK resolution or deployment failed.
    #[error(transparent)]
    Sdk(#[from] SdkError),
    /// Toolchain deployment failed.
    #[error(transparent)]
    Toolchain(#[from] ToolchainError),
    /// An application manifest could not be parsed.
    #[error(transparent)]
    FamParse(#[from] FamParseError),
    /// An application manifest could not be loaded or validated.
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    /// An application build failed.
    #[error(transparent)]
    Build(#[from] BuildError),
    /// An ELF could not be parsed.
    #[error(transparent)]
    Elf(#[from] ElfError),
    /// Icon assets could not be compiled.
    #[error(transparent)]
    Icon(#[from] IconError),
    /// File assets could not be bundled.
    #[error(transparent)]
    Bundle(#[from] BundleError),
    /// A file system operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Result alias defaulting to the crate-wide [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
