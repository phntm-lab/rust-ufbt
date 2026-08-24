#![cfg(windows)]

use std::env;
use std::fs;
use std::io::Write as _;
use std::os::windows::fs::symlink_dir;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use rust_ufbt::log::{LogEvent, LogSink, Logger, Progress, ProgressState};
use rust_ufbt::net::FileFetcher;
use rust_ufbt::paths::Paths;
use rust_ufbt::toolchain::{ToolchainDeployer, ToolchainError};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

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
        let guard = Self { _lock: env_lock() };
        unsafe { env::remove_var(VERSION_VAR) };
        guard
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe { env::remove_var(VERSION_VAR) };
    }
}

fn symlinks_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        let probe = tempfile::tempdir().unwrap();
        let target = probe.path().join("target");
        fs::create_dir_all(&target).unwrap();
        symlink_dir(&target, probe.path().join("link")).is_ok()
    })
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
    _home: tempfile::TempDir,
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
            _home: home,
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
        self.paths.toolchain_arch_dir("x86_64-windows")
    }

    fn archive_name(&self, version: &str) -> String {
        let url = self.deployer().toolchain_url(version).unwrap();
        url.rsplit('/').next().unwrap().to_owned()
    }

    fn dist_dir_name(&self, version: &str) -> String {
        self.archive_name(version)
            .replace(&format!("-{version}.zip"), "")
    }

    fn install_version(&self, version: &str) {
        let arch_dir = self.arch_dir();
        fs::create_dir_all(&arch_dir).unwrap();
        fs::write(arch_dir.join("VERSION"), format!("{version}\n")).unwrap();
    }

    fn archive_path(&self, version: &str) -> PathBuf {
        self.paths
            .current_sdk_dir()
            .join(self.archive_name(version))
    }

    fn write_archive(&self, version: &str, dist_dir_name: &str) -> PathBuf {
        let path = self.archive_path(version);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = ZipWriter::new(fs::File::create(&path).unwrap());
        let options = SimpleFileOptions::default();
        zip.add_directory(format!("{dist_dir_name}/"), options)
            .unwrap();
        zip.start_file(format!("{dist_dir_name}/VERSION"), options)
            .unwrap();
        zip.write_all(format!("{version}\n").as_bytes()).unwrap();
        zip.start_file(
            format!("{dist_dir_name}/bin/arm-none-eabi-gcc.exe"),
            options,
        )
        .unwrap();
        zip.write_all(b"MZ").unwrap();
        zip.finish().unwrap();
        path
    }

    fn compiler(&self) -> PathBuf {
        self.arch_dir().join("bin").join("arm-none-eabi-gcc.exe")
    }
}

fn owned(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

#[tokio::test]
async fn an_archive_in_place_is_extracted_moved_linked_and_removed() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let archive = harness.write_archive("39", &harness.dist_dir_name("39"));

    let result = harness.deployer().deploy(true).await;

    assert!(harness.compiler().is_file());
    let mut expected = owned(&[
        "Extracting Windows toolchain..",
        "moving..",
        "linking to 'current'..",
    ]);
    if symlinks_available() {
        assert!(result.unwrap());
        expected.extend(owned(&["done!", "Cleaning up temporary files..", "done!"]));
        let current = harness.paths.toolchain_current_link();
        assert_eq!(fs::read_link(&current).unwrap(), harness.arch_dir());
        assert!(!archive.exists());
    } else {
        assert!(matches!(result, Err(ToolchainError::Io(_))));
    }
    assert_eq!(harness.sink.raw(), expected);

    let progress = harness.sink.progress();
    assert_eq!(progress.first().unwrap().state(), ProgressState::Started);
    assert_eq!(progress.last().unwrap().state(), ProgressState::Finished);
}

#[tokio::test]
async fn an_old_toolchain_is_removed_before_the_new_one_is_unpacked() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.install_version("38");
    let stale = harness.arch_dir().join("stale");
    fs::write(&stale, "old\n").unwrap();
    harness.write_archive("39", &harness.dist_dir_name("39"));

    let _ = harness.deployer().deploy(false).await;

    assert!(!stale.exists());
    let raw = harness.sink.raw();
    assert_eq!(
        raw.first().unwrap(),
        "Removing old Windows toolchain..",
        "the upgrade notice belongs to the Unix branch only"
    );
    assert_eq!(raw.get(1).unwrap(), "done!");
}

#[tokio::test]
async fn an_existing_link_is_unlinked_first() {
    if !symlinks_available() {
        return;
    }
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let target = harness.arch_dir();
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(harness.paths.toolchain_dir()).unwrap();
    symlink_dir(&target, harness.paths.toolchain_current_link()).unwrap();
    harness.write_archive("39", &harness.dist_dir_name("39"));

    assert!(harness.deployer().deploy(true).await.unwrap());

    let raw = harness.sink.raw();
    assert_eq!(raw.first().unwrap(), "Removing old Windows toolchain..");
    assert_eq!(raw.get(2).unwrap(), "Unlinking 'current'..");
    assert_eq!(raw.get(3).unwrap(), "done!");
}

#[tokio::test]
async fn a_leftover_unpacked_directory_is_removed_before_extraction() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let dist_dir = harness
        .paths
        .current_sdk_dir()
        .join(harness.dist_dir_name("39"));
    fs::create_dir_all(&dist_dir).unwrap();
    let leftover = dist_dir.join("leftover");
    fs::write(&leftover, "old\n").unwrap();
    harness.write_archive("39", &harness.dist_dir_name("39"));

    let _ = harness.deployer().deploy(true).await;

    assert!(!leftover.exists());
    assert_eq!(
        harness.sink.raw().first().unwrap(),
        "Cleaning up temp toolchain path.."
    );
}

#[tokio::test]
async fn an_archive_without_the_expected_directory_fails_the_deployment() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    harness.write_archive("39", "some-other-directory");

    assert!(!harness.deployer().deploy(true).await.unwrap());

    assert!(!harness.arch_dir().exists());
    assert_eq!(
        harness.sink.raw(),
        owned(&["Extracting Windows toolchain..", "moving.."])
    );
}

#[tokio::test]
async fn an_archive_that_is_not_a_zip_fails_the_deployment() {
    let _env = EnvGuard::cleared();
    let harness = Harness::new();
    let archive = harness.archive_path("39");
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    fs::write(&archive, "not a zip at all\n").unwrap();

    assert!(!harness.deployer().deploy(true).await.unwrap());

    assert_eq!(
        harness.sink.raw(),
        owned(&["Extracting Windows toolchain.."])
    );
    assert_eq!(
        harness.sink.progress().last().unwrap().state(),
        ProgressState::Failed
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
