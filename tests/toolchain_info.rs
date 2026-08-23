use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use rust_ufbt::log::Logger;
use rust_ufbt::net::FileFetcher;
use rust_ufbt::paths::Paths;
use rust_ufbt::toolchain::{ToolchainDeployer, ToolchainInfo};

const VERSION_VAR: &str = "FBT_TOOLCHAIN_VERSION";

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn cleared() -> Self {
        let guard = EnvGuard { _lock: env_lock() };
        guard.clear();
        guard
    }

    fn set(&self, value: &str) {
        unsafe { env::set_var(VERSION_VAR, value) };
    }

    fn clear(&self) {
        unsafe { env::remove_var(VERSION_VAR) };
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        self.clear();
    }
}

struct Harness {
    paths: Paths,
    _home: tempfile::TempDir,
}

impl Harness {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::new(home.path(), None::<&Path>);
        Self { paths, _home: home }
    }

    fn deployer(&self) -> ToolchainDeployer {
        let logger = Arc::new(Logger::new());
        let fetcher = Arc::new(FileFetcher::new(logger.clone()).unwrap());
        ToolchainDeployer::new(logger, self.paths.clone(), fetcher)
    }

    fn write_script(&self, contents: &str) -> PathBuf {
        let script = if cfg!(windows) {
            self.paths.fbtenv_cmd()
        } else {
            self.paths.fbtenv_script()
        };
        fs::create_dir_all(script.parent().unwrap()).unwrap();
        fs::write(&script, contents).unwrap();
        script
    }

    fn arch_dir(&self, deployer: &ToolchainDeployer) -> PathBuf {
        self.paths
            .toolchain_arch_dir(&deployer.arch_dir_name().unwrap())
    }
}

#[test]
fn the_architecture_directory_is_named_after_the_platform() {
    let harness = Harness::new();
    let name = harness.deployer().arch_dir_name().unwrap();

    if cfg!(windows) {
        assert_eq!(name, "x86_64-windows");
    } else {
        let (machine, system) = name.split_once('-').unwrap();
        assert!(!machine.is_empty());
        assert_eq!(system, system.to_lowercase());
        assert!(!system.is_empty());
    }
}

#[test]
fn the_version_falls_back_when_no_script_is_installed() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();

    assert_eq!(harness.deployer().toolchain_version(), "39");
    assert_eq!(ToolchainDeployer::FALLBACK_VERSION, "39");
}

#[test]
fn the_environment_overrides_the_script() {
    let env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.write_script(script_naming_42());
    env.set("57");

    assert_eq!(harness.deployer().toolchain_version(), "57");
}

#[test]
fn an_empty_environment_variable_is_ignored() {
    let env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.write_script(script_naming_42());
    env.set("");

    assert_eq!(harness.deployer().toolchain_version(), "42");
}

#[test]
fn a_version_the_script_does_not_name_falls_back() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.write_script("echo nothing to see here\n");

    assert_eq!(harness.deployer().toolchain_version(), "39");
}

#[cfg(unix)]
#[test]
fn the_version_is_read_from_the_shell_script() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.write_script(concat!(
        "#!/bin/bash\n",
        "FBT_TOOLCHAIN_PATH_WARNING=0\n",
        "FBT_TOOLCHAIN_VERSION=\"${FBT_TOOLCHAIN_VERSION:-\"42\"}\"\n",
        "FBT_TOOLCHAIN_VERSION=\"99\"\n",
    ));

    assert_eq!(harness.deployer().toolchain_version(), "42");
}

#[cfg(unix)]
#[test]
fn a_shell_assignment_without_quotes_is_not_a_version() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.write_script("FBT_TOOLCHAIN_VERSION=42\n");

    assert_eq!(harness.deployer().toolchain_version(), "39");
}

#[cfg(windows)]
#[test]
fn the_version_is_read_from_the_batch_script() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.write_script(concat!(
        "@echo off\r\n",
        "set FLIPPER_TOOLCHAIN_VERSION=42\r\n",
        "set FLIPPER_TOOLCHAIN_VERSION=99\r\n",
    ));

    assert_eq!(harness.deployer().toolchain_version(), "42");
}

#[test]
fn the_url_names_the_platform_the_version_and_the_archive_format() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let deployer = harness.deployer();
    let arch = deployer.arch_dir_name().unwrap();
    let suffix = if cfg!(windows) { "zip" } else { "tar.gz" };

    assert_eq!(
        deployer.toolchain_url("39").unwrap(),
        format!(
            "https://update.flipperzero.one/builds/toolchain\
             /gcc-arm-none-eabi-12.3-{arch}-flipper-39.{suffix}"
        )
    );
    assert_eq!(
        ToolchainDeployer::URL_ROOT,
        "https://update.flipperzero.one/builds/toolchain"
    );
}

#[test]
fn a_missing_toolchain_directory_is_not_deployed() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let deployer = harness.deployer();
    let info = deployer.status().unwrap();

    assert_eq!(info.arch_dir(), harness.arch_dir(&deployer));
    assert_eq!(info.version(), "39");
    assert_eq!(info.url(), deployer.toolchain_url("39").unwrap());
    assert_eq!(info.installed_version(), None);
    assert!(!info.is_deployed());
    assert!(!info.is_up_to_date());
}

#[test]
fn a_toolchain_directory_without_a_version_file_is_deployed_but_not_current() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let deployer = harness.deployer();
    fs::create_dir_all(harness.arch_dir(&deployer)).unwrap();

    let info = deployer.status().unwrap();

    assert!(info.is_deployed());
    assert_eq!(info.installed_version(), None);
    assert!(!info.is_up_to_date());
}

#[test]
fn the_installed_version_is_read_from_the_version_file() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let deployer = harness.deployer();
    let arch_dir = harness.arch_dir(&deployer);
    fs::create_dir_all(&arch_dir).unwrap();
    fs::write(arch_dir.join("VERSION"), "  39 \n").unwrap();

    let info = deployer.status().unwrap();

    assert_eq!(info.installed_version(), Some("39"));
    assert!(info.is_up_to_date());
}

#[test]
fn an_installed_version_that_differs_is_not_up_to_date() {
    let env = EnvGuard::cleared();
    let harness = Harness::new();
    let deployer = harness.deployer();
    let arch_dir = harness.arch_dir(&deployer);
    fs::create_dir_all(&arch_dir).unwrap();
    fs::write(arch_dir.join("VERSION"), "38\n").unwrap();
    env.set("39");

    let info = deployer.status().unwrap();

    assert_eq!(info.version(), "39");
    assert_eq!(info.installed_version(), Some("38"));
    assert!(info.is_deployed());
    assert!(!info.is_up_to_date());
}

#[test]
fn a_toolchain_that_is_not_deployed_is_never_up_to_date() {
    let info = ToolchainInfo::new(
        "/toolchain/x86_64-linux",
        "39",
        "",
        Some("39".to_owned()),
        false,
    );

    assert!(!info.is_up_to_date());
    assert_eq!(info.arch_dir(), Path::new("/toolchain/x86_64-linux"));
}
fn script_naming_42() -> &'static str {
    if cfg!(windows) {
        "set FLIPPER_TOOLCHAIN_VERSION=42\r\n"
    } else {
        "FBT_TOOLCHAIN_VERSION=\"42\"\n"
    }
}
