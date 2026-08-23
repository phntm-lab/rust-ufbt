use std::env;
use std::fs;
use std::sync::{Arc, LazyLock};

use regex::Regex;

use super::{ToolchainError, ToolchainInfo};
use crate::log::Logger;
use crate::net::FileFetcher;
use crate::paths::Paths;

const TOOLCHAIN_VERSION_VAR: &str = "FBT_TOOLCHAIN_VERSION";
const VERSION_FILE_NAME: &str = "VERSION";

#[cfg(windows)]
static VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"FLIPPER_TOOLCHAIN_VERSION=([0-9]+)").unwrap());

#[cfg(not(windows))]
static VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"FBT_TOOLCHAIN_VERSION=[^\n]*?"([0-9]+)""#).unwrap());

/// Deploys the ARM GCC toolchain the SDK is built with into the uFBT home directory.
pub struct ToolchainDeployer {
    #[expect(dead_code)]
    logger: Arc<Logger>,
    paths: Paths,
    #[expect(dead_code)]
    fetcher: Arc<FileFetcher>,
}

impl ToolchainDeployer {
    /// Version used when neither the environment nor the SDK scripts name one.
    pub const FALLBACK_VERSION: &'static str = "39";

    /// Location the prebuilt toolchains are published under.
    pub const URL_ROOT: &'static str = "https://update.flipperzero.one/builds/toolchain";

    /// Creates a deployer working in the given uFBT home directory.
    pub fn new(logger: Arc<Logger>, paths: Paths, fetcher: Arc<FileFetcher>) -> Self {
        Self {
            logger,
            paths,
            fetcher,
        }
    }

    /// Name of the directory the toolchain of this machine is deployed into.
    ///
    /// On Windows this is always `x86_64-windows`. Everywhere else it is the machine name
    /// and the lowercased system name as `uname` reports them, such as `x86_64-linux` or
    /// `arm64-darwin`.
    ///
    /// # Errors
    ///
    /// Returns [`ToolchainError::Process`] when `uname` cannot be started.
    pub fn arch_dir_name(&self) -> Result<String, ToolchainError> {
        arch_dir_name()
    }

    /// Version of the toolchain the SDK asks for.
    ///
    /// The `FBT_TOOLCHAIN_VERSION` environment variable wins when it is set and not empty.
    /// Otherwise the version is taken from the toolchain script shipped with the current
    /// SDK — `fbtenv.cmd` on Windows, `fbtenv.sh` elsewhere — and falls back to
    /// [`FALLBACK_VERSION`](Self::FALLBACK_VERSION) when that script is absent or names no
    /// version.
    #[must_use]
    pub fn toolchain_version(&self) -> String {
        if let Ok(version) = env::var(TOOLCHAIN_VERSION_VAR) {
            if !version.is_empty() {
                return version;
            }
        }

        let script = if cfg!(windows) {
            self.paths.fbtenv_cmd()
        } else {
            self.paths.fbtenv_script()
        };
        if let Ok(contents) = fs::read_to_string(script) {
            if let Some(captures) = VERSION_RE.captures(&contents) {
                return captures[1].to_owned();
            }
        }
        Self::FALLBACK_VERSION.to_owned()
    }

    /// URL the given toolchain version is downloaded from.
    ///
    /// # Errors
    ///
    /// Returns [`ToolchainError::Process`] when `uname` cannot be started.
    pub fn toolchain_url(&self, version: &str) -> Result<String, ToolchainError> {
        Ok(toolchain_url(&self.arch_dir_name()?, version))
    }

    /// Reports which toolchain is wanted and which one is deployed.
    ///
    /// # Errors
    ///
    /// Returns [`ToolchainError::Process`] when `uname` cannot be started.
    pub fn status(&self) -> Result<ToolchainInfo, ToolchainError> {
        let version = self.toolchain_version();
        let arch_dir_name = self.arch_dir_name()?;
        let arch_dir = self.paths.toolchain_arch_dir(&arch_dir_name);
        let installed_version = fs::read_to_string(arch_dir.join(VERSION_FILE_NAME))
            .ok()
            .map(|contents| contents.trim().to_owned());
        let is_deployed = arch_dir.is_dir();
        Ok(ToolchainInfo::new(
            arch_dir,
            &version,
            toolchain_url(&arch_dir_name, &version),
            installed_version,
            is_deployed,
        ))
    }
}

fn toolchain_url(arch_dir_name: &str, version: &str) -> String {
    let suffix = if cfg!(windows) { "zip" } else { "tar.gz" };
    format!(
        "{}/gcc-arm-none-eabi-12.3-{arch_dir_name}-flipper-{version}.{suffix}",
        ToolchainDeployer::URL_ROOT
    )
}

#[cfg(windows)]
fn arch_dir_name() -> Result<String, ToolchainError> {
    Ok("x86_64-windows".to_owned())
}

#[cfg(not(windows))]
fn arch_dir_name() -> Result<String, ToolchainError> {
    Ok(format!("{}-{}", uname("-m")?, uname("-s")?.to_lowercase()))
}

#[cfg(not(windows))]
fn uname(flag: &str) -> Result<String, ToolchainError> {
    let output = std::process::Command::new("uname")
        .arg(flag)
        .output()
        .map_err(|source| ToolchainError::Process {
            program: "uname".to_owned(),
            source,
        })?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
