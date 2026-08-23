use std::path::{Path, PathBuf};

/// State of the ARM GCC toolchain in the uFBT home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainInfo {
    arch_dir: PathBuf,
    version: String,
    url: String,
    installed_version: Option<String>,
    is_deployed: bool,
}

impl ToolchainInfo {
    /// Describes a toolchain by the directory it lives in, the version that is wanted, the
    /// URL it is published under, the version that is actually installed and whether the
    /// directory exists at all.
    #[must_use]
    pub fn new(
        arch_dir: impl AsRef<Path>,
        version: impl Into<String>,
        url: impl Into<String>,
        installed_version: Option<String>,
        is_deployed: bool,
    ) -> Self {
        Self {
            arch_dir: arch_dir.as_ref().to_path_buf(),
            version: version.into(),
            url: url.into(),
            installed_version,
            is_deployed,
        }
    }

    /// Directory the toolchain for the current architecture is unpacked into.
    #[must_use]
    pub fn arch_dir(&self) -> &Path {
        &self.arch_dir
    }

    /// Version the SDK asks for.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// URL the wanted version is downloaded from.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Version recorded in the `VERSION` file of the deployed toolchain, or [`None`] when
    /// that file is absent or unreadable.
    #[must_use]
    pub fn installed_version(&self) -> Option<&str> {
        self.installed_version.as_deref()
    }

    /// Whether the toolchain directory exists.
    #[must_use]
    pub fn is_deployed(&self) -> bool {
        self.is_deployed
    }

    /// Whether the deployed toolchain is the one the SDK asks for.
    #[must_use]
    pub fn is_up_to_date(&self) -> bool {
        self.is_deployed && self.installed_version.as_deref() == Some(self.version.as_str())
    }
}
