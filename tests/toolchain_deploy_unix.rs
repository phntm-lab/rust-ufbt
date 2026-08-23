#![cfg(unix)]

use std::env;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use rust_ufbt::log::{LogEvent, LogSink, Logger, Progress, ProgressState};
use rust_ufbt::net::FileFetcher;
use rust_ufbt::paths::Paths;
use rust_ufbt::toolchain::ToolchainDeployer;

const VERSION_VAR: &str = "FBT_TOOLCHAIN_VERSION";
const PRESERVE_VAR: &str = "FBT_PRESERVE_TAR";
const PATH_VAR: &str = "PATH";

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct EnvGuard {
    path: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn cleared() -> Self {
        let guard = Self {
            path: env::var(PATH_VAR).ok(),
            _lock: env_lock(),
        };
        unsafe {
            env::remove_var(VERSION_VAR);
            env::remove_var(PRESERVE_VAR);
        }
        guard
    }

    fn set(&self, name: &str, value: &str) {
        unsafe { env::set_var(name, value) };
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            env::remove_var(VERSION_VAR);
            env::remove_var(PRESERVE_VAR);
            match &self.path {
                Some(path) => env::set_var(PATH_VAR, path),
                None => env::remove_var(PATH_VAR),
            }
        }
    }
}

#[derive(Default)]
struct CapturingSink {
    raw: Mutex<Vec<String>>,
    progress: Mutex<Vec<Progress>>,
}

impl CapturingSink {
    fn raw(&self) -> Vec<String> {
        self.raw.lock().unwrap().clone()
    }

    fn progress(&self) -> Vec<Progress> {
        self.progress.lock().unwrap().clone()
    }
}

impl LogSink for CapturingSink {
    fn on_event(&self, event: &LogEvent) {
        match event {
            LogEvent::Raw { text, .. } => self.raw.lock().unwrap().push(text.clone()),
            LogEvent::Progress { progress, .. } => {
                self.progress.lock().unwrap().push(progress.clone());
            }
            _ => {}
        }
    }
}

struct Harness {
    logger: Arc<Logger>,
    sink: Arc<CapturingSink>,
    fetcher: Arc<FileFetcher>,
    paths: Paths,
    home: tempfile::TempDir,
}

impl Harness {
    fn new() -> Self {
        let sink = Arc::new(CapturingSink::default());
        let logger = Arc::new(Logger::new());
        logger.add_sink(sink.clone());
        let fetcher = Arc::new(FileFetcher::new(logger.clone()).unwrap());
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::new(home.path(), None::<&Path>);
        Self {
            logger,
            sink,
            fetcher,
            paths,
            home,
        }
    }

    fn deployer(&self) -> ToolchainDeployer {
        ToolchainDeployer::new(
            self.logger.clone(),
            self.paths.clone(),
            self.fetcher.clone(),
        )
    }

    fn arch_dir(&self) -> PathBuf {
        self.paths
            .toolchain_arch_dir(&self.deployer().arch_dir_name().unwrap())
    }

    fn archive_name(&self, version: &str) -> String {
        let url = self.deployer().toolchain_url(version).unwrap();
        url.rsplit('/').next().unwrap().to_owned()
    }

    fn dist_dir_name(&self, version: &str) -> String {
        self.archive_name(version)
            .replace(&format!("-{version}.tar.gz"), "")
    }

    fn install_version(&self, version: &str) {
        let arch_dir = self.arch_dir();
        fs::create_dir_all(&arch_dir).unwrap();
        fs::write(arch_dir.join("VERSION"), format!("{version}\n")).unwrap();
    }

    fn write_archive(&self, version: &str, dist_dir_name: &str) -> PathBuf {
        let staging = self.home.path().join("staging");
        let dist = staging.join(dist_dir_name);
        fs::create_dir_all(dist.join("bin")).unwrap();
        fs::write(dist.join("bin").join("arm-none-eabi-gcc"), "elf\n").unwrap();
        fs::write(dist.join("VERSION"), format!("{version}\n")).unwrap();

        let toolchain_dir = self.paths.toolchain_dir();
        fs::create_dir_all(&toolchain_dir).unwrap();
        let archive = toolchain_dir.join(self.archive_name(version));
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&staging)
            .arg(dist_dir_name)
            .status()
            .unwrap();
        assert!(status.success());
        fs::remove_dir_all(&staging).unwrap();
        archive
    }
}

fn real_uname() -> &'static Path {
    ["/usr/bin/uname", "/bin/uname"]
        .into_iter()
        .map(Path::new)
        .find(|candidate| candidate.exists())
        .expect("uname is expected to exist")
}

#[tokio::test]
async fn a_downloaded_archive_is_unpacked_linked_and_removed() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let archive = harness.write_archive("39", &harness.dist_dir_name("39"));

    assert!(harness.deployer().deploy(true).await.unwrap());

    let arch_dir = harness.arch_dir();
    assert!(arch_dir.join("bin").join("arm-none-eabi-gcc").is_file());
    let current = harness.paths.toolchain_current_link();
    assert!(current.symlink_metadata().is_ok());
    assert_eq!(fs::read_link(&current).unwrap(), arch_dir);
    assert!(!archive.exists());

    assert_eq!(
        harness.sink.raw(),
        vec![
            "Checking for tar..".to_owned(),
            "yes".to_owned(),
            "Checking if downloaded toolchain tgz exists..".to_owned(),
            "yes".to_owned(),
            "Removing old toolchain..".to_owned(),
            "done".to_owned(),
            format!(
                "Unpacking toolchain to '{}':",
                harness.paths.toolchain_dir().display()
            ),
            "linking toolchain to 'current'..".to_owned(),
            "done".to_owned(),
            "Cleaning up..".to_owned(),
            "done".to_owned(),
        ]
    );

    let progress = harness.sink.progress();
    assert_eq!(progress.first().unwrap().state(), ProgressState::Started);
    assert_eq!(progress.last().unwrap().state(), ProgressState::Finished);
    assert!(progress.last().unwrap().current() > 0);
}

#[tokio::test]
async fn an_old_toolchain_and_its_link_are_replaced() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.install_version("38");
    let stale = harness.arch_dir().join("stale");
    fs::write(&stale, "old\n").unwrap();
    symlink(harness.arch_dir(), harness.paths.toolchain_current_link()).unwrap();
    harness.write_archive("39", &harness.dist_dir_name("39"));

    assert!(harness.deployer().deploy(false).await.unwrap());

    assert!(!stale.exists());
    assert_eq!(
        fs::read_to_string(harness.arch_dir().join("VERSION")).unwrap(),
        "39\n"
    );
    assert_eq!(
        harness.sink.raw().first().unwrap(),
        "FBT: starting toolchain upgrade process.."
    );
}

#[tokio::test]
async fn a_toolchain_of_the_wanted_version_is_left_alone() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.install_version("39");

    assert!(harness.deployer().deploy(false).await.unwrap());

    assert!(harness.sink.raw().is_empty());
}

#[tokio::test]
async fn a_toolchain_of_the_wanted_version_is_replaced_when_forced() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.install_version("39");
    harness.write_archive("39", &harness.dist_dir_name("39"));

    assert!(harness.deployer().deploy(true).await.unwrap());

    assert!(
        harness
            .arch_dir()
            .join("bin")
            .join("arm-none-eabi-gcc")
            .is_file()
    );
    assert_eq!(harness.sink.raw().first().unwrap(), "Checking for tar..");
}

#[tokio::test]
async fn an_archive_without_the_expected_directory_fails_the_deployment() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.write_archive("39", "some-other-directory");

    assert!(!harness.deployer().deploy(true).await.unwrap());

    assert!(!harness.arch_dir().exists());
    assert!(
        !harness
            .sink
            .raw()
            .contains(&"linking toolchain to 'current'..".to_owned())
    );
}

#[tokio::test]
async fn a_deployment_without_tar_reports_it_and_stops() {
    let env = EnvGuard::cleared();
    let harness = Harness::new();
    let bin = harness.home.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    symlink(real_uname(), bin.join("uname")).unwrap();
    env.set(PATH_VAR, &bin.to_string_lossy());

    assert!(!harness.deployer().deploy(true).await.unwrap());

    assert_eq!(
        harness.sink.raw(),
        vec!["Checking for tar..".to_owned(), "no".to_owned()]
    );
}

#[tokio::test]
async fn cleaning_up_removes_part_files_and_the_archive() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let archive = harness.write_archive("39", &harness.dist_dir_name("39"));
    let part = harness.paths.toolchain_dir().join("leftover.tar.gz.part");
    fs::write(&part, "partial\n").unwrap();

    assert!(harness.deployer().deploy(true).await.unwrap());

    assert!(!part.exists());
    assert!(!archive.exists());
}

#[tokio::test]
async fn a_preserved_archive_survives_the_cleanup() {
    let env = EnvGuard::cleared();
    let harness = Harness::new();
    let archive = harness.write_archive("39", &harness.dist_dir_name("39"));
    let part = harness.paths.toolchain_dir().join("leftover.tar.gz.part");
    fs::write(&part, "partial\n").unwrap();
    env.set(PRESERVE_VAR, "1");

    assert!(harness.deployer().deploy(true).await.unwrap());

    assert!(!part.exists());
    assert!(archive.is_file());
}

#[tokio::test]
async fn an_empty_preserve_variable_does_not_preserve() {
    let env = EnvGuard::cleared();
    let harness = Harness::new();
    let archive = harness.write_archive("39", &harness.dist_dir_name("39"));
    env.set(PRESERVE_VAR, "");

    assert!(harness.deployer().deploy(true).await.unwrap());

    assert!(!archive.exists());
}
