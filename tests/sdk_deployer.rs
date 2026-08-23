use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rust_ufbt::log::{LogEvent, LogLevel, LogSink, Logger, Progress, ProgressState};
use rust_ufbt::net::FileFetcher;
use rust_ufbt::paths::Paths;
use rust_ufbt::sdk::{DeployTask, SdkDeployer, SdkError, UpdateChannel};
use rust_ufbt::state::State;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use zip::write::SimpleFileOptions;

#[derive(Default)]
struct CapturingSink {
    messages: Mutex<Vec<(LogLevel, String)>>,
    progress: Mutex<Vec<Progress>>,
}

impl CapturingSink {
    fn messages(&self) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .map(|(_, message)| message.clone())
            .collect()
    }

    fn errors(&self) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter(|(level, _)| *level == LogLevel::Error)
            .map(|(_, message)| message.clone())
            .collect()
    }

    fn progress(&self) -> Vec<Progress> {
        self.progress.lock().unwrap().clone()
    }
}

impl LogSink for CapturingSink {
    fn on_event(&self, event: &LogEvent) {
        match event {
            LogEvent::Message { level, message, .. } => {
                self.messages
                    .lock()
                    .unwrap()
                    .push((*level, message.clone()));
            }
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
        logger.set_level(LogLevel::Debug);
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

    fn deployer(&self) -> SdkDeployer {
        SdkDeployer::new(
            self.logger.clone(),
            self.paths.clone(),
            self.fetcher.clone(),
        )
    }

    fn current_dir(&self) -> PathBuf {
        self.paths.current_sdk_dir()
    }

    fn state_file(&self) -> PathBuf {
        self.paths.state_file()
    }

    fn write_state(&self, value: serde_json::Value) {
        fs::create_dir_all(self.current_dir()).unwrap();
        fs::write(self.state_file(), value.to_string()).unwrap();
    }

    fn mark_current(&self) {
        fs::create_dir_all(self.current_dir()).unwrap();
        fs::write(self.current_dir().join("marker"), b"old").unwrap();
    }

    fn marker_survived(&self) -> bool {
        self.current_dir().join("marker").exists()
    }

    fn bundle(&self, entries: &[(&str, Option<&[u8]>)]) -> PathBuf {
        let target = self.home.path().join("bundle.zip");
        write_zip(&target, entries);
        target
    }
}

fn write_zip(target: &Path, entries: &[(&str, Option<&[u8]>)]) {
    let file = fs::File::create(target).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for (name, contents) in entries {
        match contents {
            Some(bytes) => {
                writer.start_file(*name, options).unwrap();
                writer.write_all(bytes).unwrap();
            }
            None => writer.add_directory(*name, options).unwrap(),
        }
    }
    writer.finish().unwrap();
}

const SDK_ENTRIES: [(&str, Option<&[u8]>); 3] = [
    ("scripts/", None),
    ("scripts/fbt.py", Some(b"print()")),
    ("sdk.opts", Some(b"opts")),
];

fn local_task(bundle: &Path, hw_target: &str) -> DeployTask {
    DeployTask::local(bundle, hw_target).build()
}

#[test]
fn previous_task_without_a_state_file_yields_nothing() {
    let harness = Harness::new();

    assert!(harness.deployer().previous_task().is_none());
}

#[test]
fn previous_task_reads_the_recorded_state() {
    let harness = Harness::new();
    harness.write_state(json!({
        "hw_target": "f18",
        "mode": "channel",
        "channel": "dev",
        "version": "0.104.0"
    }));

    let task = harness.deployer().previous_task().unwrap();

    assert_eq!(task.hw_target(), Some("f18"));
    assert_eq!(task.mode(), Some("channel"));
    assert_eq!(task.param("channel"), Some("dev"));
    assert!(
        harness
            .sink
            .messages()
            .iter()
            .any(|message| message.starts_with("get_previous_task() loaded state: {"))
    );
}

#[test]
fn previous_task_renders_the_state_the_way_the_original_does() {
    let harness = Harness::new();
    harness.write_state(json!({
        "hw_target": "f7",
        "timestamp": 17,
        "ratio": 1.5,
        "beta": true,
        "changelog": null,
        "files": ["sdk.zip", "lib.zip"],
        "details": { "nested": "yes" }
    }));

    harness.deployer().previous_task().unwrap();

    let rendered = harness
        .sink
        .messages()
        .into_iter()
        .find(|message| message.starts_with("get_previous_task() loaded state: "))
        .unwrap();

    assert_eq!(
        rendered,
        "get_previous_task() loaded state: {hw_target: f7, timestamp: 17, ratio: 1.5, \
         beta: true, changelog: null, files: [sdk.zip, lib.zip], details: {nested: yes}}"
    );
}

#[test]
fn previous_task_of_a_state_that_is_not_an_object_yields_nothing() {
    let harness = Harness::new();
    harness.write_state(json!(["f7"]));

    assert!(harness.deployer().previous_task().is_none());
}

#[test]
fn create_loader_serves_every_mode() {
    let harness = Harness::new();
    let deployer = harness.deployer();

    let cases = [
        (DeployTask::branch("dev").build(), "branch"),
        (
            DeployTask::channel(UpdateChannel::Release).build(),
            "channel",
        ),
        (
            DeployTask::url("https://example.test/sdk.zip", "f7").build(),
            "url",
        ),
        (DeployTask::local("sdk.zip", "f7").build(), "local"),
    ];

    for (task, expected) in cases {
        assert_eq!(deployer.create_loader(&task).unwrap().mode_key(), expected);
    }
}

#[test]
fn create_loader_announces_the_task_it_was_given() {
    let harness = Harness::new();
    let task = DeployTask::defaults();

    harness.deployer().create_loader(&task).unwrap();

    assert!(
        harness
            .sink
            .messages()
            .contains(&format!("SdkLoaderFactory::create_for_task task={task}"))
    );
}

#[test]
fn create_loader_rejects_a_mode_no_loader_serves() {
    let harness = Harness::new();
    let mut task = DeployTask::defaults();
    task.update_from(&DeployTask::from_state(&State::new(
        serde_json::from_value(json!({ "mode": "nightly" })).unwrap(),
    )));

    let error = harness.deployer().create_loader(&task).err().unwrap();

    assert!(matches!(error, SdkError::InvalidMode { .. }));
    assert_eq!(error.to_string(), "Invalid mode: nightly");
}

#[test]
fn create_loader_rejects_a_task_without_a_mode() {
    let harness = Harness::new();
    let task = DeployTask::from_state(&State::default());

    let error = harness.deployer().create_loader(&task).err().unwrap();

    assert_eq!(error.to_string(), "Invalid mode: null");
}

#[test]
fn create_loader_rejects_a_mode_without_its_parameter() {
    let harness = Harness::new();
    let task = DeployTask::from_state(&State::new(
        serde_json::from_value(json!({ "mode": "branch" })).unwrap(),
    ));

    let error = harness.deployer().create_loader(&task).err().unwrap();

    assert!(matches!(error, SdkError::MissingParam { .. }));
    assert_eq!(
        error.to_string(),
        "Missing branch parameter for mode branch"
    );
}

#[test]
fn create_loader_rejects_a_channel_that_does_not_exist() {
    let harness = Harness::new();
    let task = DeployTask::from_state(&State::new(
        serde_json::from_value(json!({ "mode": "channel", "channel": "nightly" })).unwrap(),
    ));

    let error = harness.deployer().create_loader(&task).err().unwrap();

    assert_eq!(error.to_string(), "Invalid channel: nightly");
}

#[tokio::test]
async fn deploy_unpacks_the_bundle_and_records_the_state() {
    let harness = Harness::new();
    let bundle = harness.bundle(&SDK_ENTRIES);

    assert!(
        harness
            .deployer()
            .deploy(&local_task(&bundle, "f7"))
            .await
            .unwrap()
    );

    assert_eq!(
        fs::read(harness.current_dir().join("scripts").join("fbt.py")).unwrap(),
        b"print()"
    );
    assert_eq!(
        fs::read(harness.current_dir().join("sdk.opts")).unwrap(),
        b"opts"
    );

    let state = State::read(harness.state_file()).unwrap().unwrap();
    assert_eq!(state.hw_target(), Some("f7"));
    assert_eq!(state.mode(), Some("local"));
    assert_eq!(state.version(), Some("unknown"));
    assert_eq!(
        state.values().keys().collect::<Vec<_>>(),
        ["hw_target", "mode", "file_path", "version"]
    );

    assert!(
        harness
            .sink
            .messages()
            .contains(&"SDK deployed.".to_string())
    );
}

#[tokio::test]
async fn deploy_records_a_hardware_target_it_was_not_given_as_null() {
    let harness = Harness::new();
    let bundle = harness.bundle(&SDK_ENTRIES);
    let task = DeployTask::from_state(&State::new(
        serde_json::from_value(json!({
            "mode": "local",
            "file_path": bundle.to_string_lossy(),
        }))
        .unwrap(),
    ));

    assert_eq!(task.hw_target(), None);
    assert!(harness.deployer().deploy(&task).await.unwrap());

    let recorded = fs::read_to_string(harness.state_file()).unwrap();
    assert!(recorded.starts_with(
        "{
    \"hw_target\": null,
"
    ));
    assert!(
        harness
            .sink
            .messages()
            .contains(&"Deploying SDK for null".to_string())
    );
}

#[tokio::test]
async fn deploy_replaces_whatever_was_there_before() {
    let harness = Harness::new();
    harness.mark_current();
    let bundle = harness.bundle(&SDK_ENTRIES);

    harness
        .deployer()
        .deploy(&local_task(&bundle, "f7"))
        .await
        .unwrap();

    assert!(!harness.marker_survived());
}

#[tokio::test]
async fn deploy_replaces_an_sdk_whose_state_is_missing() {
    let harness = Harness::new();
    harness.mark_current();
    let bundle = harness.bundle(&SDK_ENTRIES);

    assert!(
        harness
            .deployer()
            .deploy(&local_task(&bundle, "f7"))
            .await
            .unwrap()
    );

    assert!(!harness.marker_survived());
    assert!(harness.current_dir().join("sdk.opts").exists());
    assert!(
        harness
            .sink
            .messages()
            .contains(&"Cannot determine current SDK version, updating".to_string())
    );
}

#[tokio::test]
async fn deploy_replaces_an_sdk_whose_state_is_unreadable() {
    let harness = Harness::new();
    harness.mark_current();
    fs::write(harness.state_file(), b"not json at all").unwrap();
    let bundle = harness.bundle(&SDK_ENTRIES);

    assert!(
        harness
            .deployer()
            .deploy(&local_task(&bundle, "f7"))
            .await
            .unwrap()
    );

    assert!(!harness.marker_survived());
}

#[tokio::test]
async fn deploy_replaces_an_sdk_of_an_unknown_version() {
    let harness = Harness::new();
    harness.mark_current();
    harness.write_state(json!({ "hw_target": "f7", "mode": "local", "version": "unknown" }));
    let bundle = harness.bundle(&SDK_ENTRIES);

    harness
        .deployer()
        .deploy(&local_task(&bundle, "f7"))
        .await
        .unwrap();

    assert!(!harness.marker_survived());
    assert!(
        harness
            .sink
            .messages()
            .contains(&"Cannot determine current SDK version, updating".to_string())
    );
}

#[tokio::test]
async fn deploy_replaces_an_sdk_whose_state_carries_no_version() {
    let harness = Harness::new();
    harness.mark_current();
    harness.write_state(json!({ "hw_target": "f7", "mode": "local" }));
    let bundle = harness.bundle(&SDK_ENTRIES);

    harness
        .deployer()
        .deploy(&local_task(&bundle, "f7"))
        .await
        .unwrap();

    assert!(!harness.marker_survived());
    assert!(
        !harness
            .sink
            .messages()
            .contains(&"Cannot determine current SDK version, updating".to_string())
    );
}

async fn channel_harness(version: &str) -> (Harness, MockServer, DeployTask) {
    let harness = Harness::new();
    let server = MockServer::start().await;
    let bundle_url = format!("{}/sdk.zip", server.uri());

    let mut zip_bytes = Vec::new();
    let bundle = harness.bundle(&SDK_ENTRIES);
    zip_bytes.extend_from_slice(&fs::read(&bundle).unwrap());

    Mock::given(method("GET"))
        .and(path("/directory.json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                json!({
                    "channels": [{
                        "id": "release",
                        "versions": [{
                            "version": version,
                            "files": [
                            { "url": bundle_url, "target": "f7", "type": "sdk_zip" },
                            { "url": bundle_url, "target": "f18", "type": "sdk_zip" }
                        ]
                        }]
                    }]
                })
                .to_string(),
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sdk.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(zip_bytes))
        .mount(&server)
        .await;

    let task = DeployTask::channel(UpdateChannel::Release)
        .index_url(format!("{}/directory.json", server.uri()))
        .build();
    (harness, server, task)
}

#[tokio::test]
async fn deploy_leaves_an_up_to_date_sdk_alone() {
    let (harness, _server, task) = channel_harness("0.104.0").await;
    harness.deployer().deploy(&task).await.unwrap();
    harness.mark_current();

    assert!(harness.deployer().deploy(&task).await.unwrap());

    assert!(harness.marker_survived());
    assert!(
        harness
            .sink
            .messages()
            .contains(&"SDK is up-to-date".to_string())
    );
}

#[tokio::test]
async fn deploy_replaces_an_up_to_date_sdk_when_forced() {
    let (harness, server, _) = channel_harness("0.104.0").await;
    let task = DeployTask::channel(UpdateChannel::Release)
        .index_url(format!("{}/directory.json", server.uri()))
        .build();
    harness.deployer().deploy(&task).await.unwrap();
    harness.mark_current();

    let forced = DeployTask::channel(UpdateChannel::Release)
        .index_url(format!("{}/directory.json", server.uri()))
        .force(true)
        .build();
    assert!(harness.deployer().deploy(&forced).await.unwrap());

    assert!(!harness.marker_survived());
}

#[tokio::test]
async fn deploy_replaces_an_sdk_built_for_another_hardware_target() {
    let (harness, server, task) = channel_harness("0.104.0").await;
    harness.deployer().deploy(&task).await.unwrap();
    harness.mark_current();

    let other = DeployTask::channel(UpdateChannel::Release)
        .hw_target("f18")
        .index_url(format!("{}/directory.json", server.uri()))
        .build();
    assert!(harness.deployer().deploy(&other).await.unwrap());

    assert!(!harness.marker_survived());
    assert_eq!(
        State::read(harness.state_file())
            .unwrap()
            .unwrap()
            .hw_target(),
        Some("f18")
    );
}

#[tokio::test]
async fn deploy_reports_a_bundle_it_could_not_obtain() {
    let harness = Harness::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sdk.zip"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let task = DeployTask::url(format!("{}/sdk.zip", server.uri()), "f7").build();

    assert!(!harness.deployer().deploy(&task).await.unwrap());

    assert_eq!(
        harness.sink.errors(),
        vec!["Failed to fetch SDK for f7: HTTP Error 500: Internal Server Error".to_string()]
    );
    assert!(!harness.state_file().exists());
}

#[tokio::test]
async fn deploy_skips_entries_that_climb_out_of_the_target_directory() {
    let harness = Harness::new();
    let bundle = harness.bundle(&[
        ("../escaped.txt", Some(b"nope" as &[u8])),
        ("./relative.txt", Some(b"nope")),
        ("nested/../escaped.txt", Some(b"nope")),
        ("kept.txt", Some(b"yes")),
    ]);

    harness
        .deployer()
        .deploy(&local_task(&bundle, "f7"))
        .await
        .unwrap();

    assert_eq!(
        fs::read(harness.current_dir().join("kept.txt")).unwrap(),
        b"yes"
    );
    assert!(!harness.current_dir().join("escaped.txt").exists());
    assert!(!harness.current_dir().join("relative.txt").exists());
    assert!(!harness.home.path().join("escaped.txt").exists());
}

#[tokio::test]
async fn unpacking_reports_progress_over_every_entry() {
    let harness = Harness::new();
    let bundle = harness.bundle(&SDK_ENTRIES);

    harness
        .deployer()
        .deploy(&local_task(&bundle, "f7"))
        .await
        .unwrap();

    let events = harness.sink.progress();
    let started = events
        .iter()
        .find(|progress| progress.state() == ProgressState::Started)
        .unwrap();
    assert_eq!(started.total(), Some(SDK_ENTRIES.len() as u64));
    assert_eq!(started.title(), "");

    let finished = events
        .iter()
        .rfind(|progress| progress.state() == ProgressState::Finished)
        .unwrap();
    assert_eq!(finished.current(), SDK_ENTRIES.len() as u64);
    assert_eq!(finished.id(), started.id());
}

#[tokio::test]
async fn deploy_reports_a_bundle_that_is_not_a_zip_archive() {
    let harness = Harness::new();
    let bundle = harness.home.path().join("broken.zip");
    fs::write(&bundle, b"definitely not a zip").unwrap();

    let error = harness
        .deployer()
        .deploy(&local_task(&bundle, "f7"))
        .await
        .unwrap_err();

    assert!(matches!(error, SdkError::Zip(_)));
    assert!(
        harness
            .sink
            .progress()
            .iter()
            .all(|progress| progress.state() != ProgressState::Finished)
    );
}
