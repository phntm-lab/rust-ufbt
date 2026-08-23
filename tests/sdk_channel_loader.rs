use std::sync::{Arc, Mutex};

use rust_ufbt::log::{LogEvent, LogLevel, LogSink, Logger};
use rust_ufbt::net::FileFetcher;
use rust_ufbt::sdk::{SdkError, SdkLoader, UpdateChannel, UpdateChannelLoader};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Default)]
struct CapturingSink {
    messages: Mutex<Vec<(LogLevel, String)>>,
}

impl CapturingSink {
    fn of_level(&self, wanted: LogLevel) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter(|(level, _)| *level == wanted)
            .map(|(_, message)| message.clone())
            .collect()
    }
}

impl LogSink for CapturingSink {
    fn on_event(&self, event: &LogEvent) {
        if let LogEvent::Message { level, message, .. } = event {
            self.messages
                .lock()
                .unwrap()
                .push((*level, message.clone()));
        }
    }
}

struct Harness {
    server: MockServer,
    logger: Arc<Logger>,
    fetcher: Arc<FileFetcher>,
    sink: Arc<CapturingSink>,
    temp: tempfile::TempDir,
}

impl Harness {
    async fn new() -> Self {
        let sink = Arc::new(CapturingSink::default());
        let logger = Arc::new(Logger::new());
        logger.set_level(LogLevel::Debug);
        logger.add_sink(sink.clone());
        let fetcher = Arc::new(FileFetcher::new(logger.clone()).unwrap());
        Self {
            server: MockServer::start().await,
            logger,
            fetcher,
            sink,
            temp: tempfile::tempdir().unwrap(),
        }
    }

    async fn serving_index(body: serde_json::Value) -> Self {
        let harness = Self::new().await;
        harness
            .serve(ResponseTemplate::new(200).set_body_string(body.to_string()))
            .await;
        harness
    }

    async fn serve(&self, response: ResponseTemplate) {
        Mock::given(method("GET"))
            .and(path("/directory.json"))
            .respond_with(response)
            .mount(&self.server)
            .await;
    }

    fn index_url(&self) -> String {
        format!("{}/directory.json", self.server.uri())
    }

    fn loader(&self, channel: UpdateChannel) -> UpdateChannelLoader {
        UpdateChannelLoader::new(
            self.logger.clone(),
            self.fetcher.clone(),
            self.temp.path(),
            channel,
        )
        .with_index_url(self.index_url())
    }
}

fn index_with_release_sdk(url: &str) -> serde_json::Value {
    json!({
        "channels": [
            {
                "id": "release",
                "versions": [
                    {
                        "version": "0.104.0",
                        "changelog": "Newest",
                        "files": [
                            { "url": url, "target": "f7", "type": "sdk_zip" }
                        ]
                    },
                    {
                        "version": "0.103.0",
                        "files": []
                    }
                ]
            }
        ]
    })
}

fn metadata_pairs(loader: &dyn SdkLoader) -> Vec<(String, String)> {
    loader
        .metadata()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

#[tokio::test]
async fn the_official_index_url_and_the_mode_key_match_the_original() {
    assert_eq!(
        UpdateChannelLoader::OFFICIAL_INDEX_URL,
        "https://update.flipperzero.one/firmware/directory.json"
    );
    assert_eq!(UpdateChannelLoader::MODE_KEY, "channel");
}

#[tokio::test]
async fn a_loader_reads_the_official_index_unless_told_otherwise() {
    let harness = Harness::new().await;
    let loader = UpdateChannelLoader::new(
        harness.logger.clone(),
        harness.fetcher.clone(),
        harness.temp.path(),
        UpdateChannel::Release,
    );

    assert_eq!(
        loader.json_index_url(),
        UpdateChannelLoader::OFFICIAL_INDEX_URL
    );
    assert_eq!(loader.channel(), UpdateChannel::Release);
    assert_eq!(loader.mode_key(), "channel");
}

#[tokio::test]
async fn load_settles_on_the_first_version_of_the_channel() {
    let harness =
        Harness::serving_index(index_with_release_sdk("https://example.test/sdk.zip")).await;
    let mut loader = harness.loader(UpdateChannel::Release);

    loader.load().await.unwrap();

    assert_eq!(loader.version_info().unwrap().version(), "0.104.0");
}

#[tokio::test]
async fn load_announces_the_channel_the_index_and_the_version() {
    let harness =
        Harness::serving_index(index_with_release_sdk("https://example.test/sdk.zip")).await;
    let mut loader = harness.loader(UpdateChannel::Release);

    loader.load().await.unwrap();

    let url = harness.index_url();
    assert!(harness.sink.of_level(LogLevel::Info).contains(&format!(
        "Fetching version info for UpdateChannel.RELEASE from {url}"
    )));
    assert!(
        harness
            .sink
            .of_level(LogLevel::Info)
            .contains(&"Using version: 0.104.0".to_string())
    );
    assert!(
        harness
            .sink
            .of_level(LogLevel::Debug)
            .contains(&"Changelog: Newest".to_string())
    );
}

#[tokio::test]
async fn load_writes_none_for_a_version_without_a_changelog() {
    let harness = Harness::serving_index(json!({
        "channels": [{ "id": "development", "versions": [{ "version": "0.104.0" }] }]
    }))
    .await;
    let mut loader = harness.loader(UpdateChannel::Dev);

    loader.load().await.unwrap();

    assert!(
        harness
            .sink
            .of_level(LogLevel::Debug)
            .contains(&"Changelog: None".to_string())
    );
}

#[tokio::test]
async fn version_info_before_load_reports_an_uninitialized_loader() {
    let harness = Harness::new().await;
    let loader = harness.loader(UpdateChannel::Release);

    let error = loader.version_info().unwrap_err();

    assert!(matches!(error, SdkError::LoaderNotInitialized));
    assert_eq!(error.to_string(), "Loader is not initialized");
}

#[tokio::test]
async fn get_sdk_component_before_load_reports_an_uninitialized_loader() {
    let harness = Harness::new().await;
    let loader = harness.loader(UpdateChannel::Release);

    let error = loader.get_sdk_component("f7").await.unwrap_err();

    assert_eq!(error.to_string(), "Loader is not initialized");
}

#[tokio::test]
async fn load_reports_a_body_that_is_not_json() {
    let harness = Harness::new().await;
    harness
        .serve(ResponseTemplate::new(200).set_body_string("<html>nope</html>"))
        .await;
    let mut loader = harness.loader(UpdateChannel::Release);

    let error = loader.load().await.unwrap_err();

    assert!(matches!(error, SdkError::InvalidJson { .. }));
    assert!(error.to_string().starts_with("Invalid JSON: "));
}

#[tokio::test]
async fn load_reports_an_index_without_channels() {
    let harness = Harness::serving_index(json!({ "channels": [] })).await;
    let mut loader = harness.loader(UpdateChannel::Release);

    let error = loader.load().await.unwrap_err();

    assert!(matches!(error, SdkError::InvalidChannel { .. }));
    assert_eq!(error.to_string(), "Invalid channel: UpdateChannel.RELEASE");
}

#[tokio::test]
async fn load_reports_a_channel_that_is_not_listed() {
    let harness =
        Harness::serving_index(index_with_release_sdk("https://example.test/sdk.zip")).await;
    let mut loader = harness.loader(UpdateChannel::Dev);

    let error = loader.load().await.unwrap_err();

    assert_eq!(error.to_string(), "Invalid channel: UpdateChannel.DEV");
}

#[tokio::test]
async fn load_reports_a_channel_without_versions() {
    let harness = Harness::serving_index(json!({
        "channels": [{ "id": "release-candidate", "versions": [] }]
    }))
    .await;
    let mut loader = harness.loader(UpdateChannel::Rc);

    let error = loader.load().await.unwrap_err();

    assert!(matches!(error, SdkError::EmptyChannel { .. }));
    assert_eq!(error.to_string(), "Empty channel: UpdateChannel.RC");
}

#[tokio::test]
async fn load_reports_a_failed_download_of_the_index() {
    let harness = Harness::new().await;
    harness.serve(ResponseTemplate::new(404)).await;
    let mut loader = harness.loader(UpdateChannel::Release);

    let error = loader.load().await.unwrap_err();

    assert!(matches!(error, SdkError::Fetch(_)));
    assert_eq!(error.to_string(), "HTTP Error 404: Not Found");
}

#[tokio::test]
async fn a_failed_load_leaves_the_loader_uninitialized() {
    let harness = Harness::serving_index(json!({ "channels": [] })).await;
    let mut loader = harness.loader(UpdateChannel::Release);

    loader.load().await.unwrap_err();

    assert!(loader.version_info().is_err());
    assert_eq!(loader.metadata()["version"], "unknown");
}

#[tokio::test]
async fn get_sdk_component_reports_a_version_without_files() {
    let harness = Harness::serving_index(json!({
        "channels": [{ "id": "release", "versions": [{ "version": "0.104.0", "files": [] }] }]
    }))
    .await;
    let mut loader = harness.loader(UpdateChannel::Release);
    loader.load().await.unwrap();

    let error = loader.get_sdk_component("f7").await.unwrap_err();

    assert!(matches!(error, SdkError::EmptyFilesList));
    assert_eq!(error.to_string(), "Empty files list");
}

#[tokio::test]
async fn get_sdk_component_reports_a_missing_bundle_for_the_target() {
    let harness =
        Harness::serving_index(index_with_release_sdk("https://example.test/sdk.zip")).await;
    let mut loader = harness.loader(UpdateChannel::Release);
    loader.load().await.unwrap();

    let error = loader.get_sdk_component("f18").await.unwrap_err();

    assert!(matches!(error, SdkError::InvalidFileType { .. }));
    assert_eq!(error.to_string(), "Invalid file type: FileType.SDK_ZIP");
}

#[tokio::test]
async fn get_sdk_component_reports_a_bundle_without_an_address() {
    let harness = Harness::serving_index(index_with_release_sdk("")).await;
    let mut loader = harness.loader(UpdateChannel::Release);
    loader.load().await.unwrap();

    let error = loader.get_sdk_component("f7").await.unwrap_err();

    assert!(matches!(error, SdkError::InvalidFileUrl));
    assert_eq!(error.to_string(), "Invalid file url");
}

#[tokio::test]
async fn get_sdk_component_downloads_the_bundle_of_the_chosen_version() {
    let harness = Harness::new().await;
    let bundle_url = format!("{}/sdk.zip", harness.server.uri());
    harness
        .serve(
            ResponseTemplate::new(200)
                .set_body_string(index_with_release_sdk(&bundle_url).to_string()),
        )
        .await;
    Mock::given(method("GET"))
        .and(path("/sdk.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bundle".to_vec()))
        .mount(&harness.server)
        .await;

    let mut loader = harness.loader(UpdateChannel::Release);
    loader.load().await.unwrap();

    let component = loader.get_sdk_component("f7").await.unwrap();

    assert_eq!(component, harness.temp.path().join("sdk.zip"));
    assert_eq!(std::fs::read(&component).unwrap(), b"bundle");
}

#[tokio::test]
async fn metadata_reports_an_unknown_version_before_load_and_the_real_one_after() {
    let harness =
        Harness::serving_index(index_with_release_sdk("https://example.test/sdk.zip")).await;
    let mut loader = harness.loader(UpdateChannel::Release);
    let url = harness.index_url();

    assert_eq!(
        metadata_pairs(&loader),
        pairs(&[
            ("mode", "channel"),
            ("channel", "release"),
            ("json_index", &url),
            ("version", "unknown"),
        ])
    );

    loader.load().await.unwrap();

    assert_eq!(
        metadata_pairs(&loader),
        pairs(&[
            ("mode", "channel"),
            ("channel", "release"),
            ("json_index", &url),
            ("version", "0.104.0"),
        ])
    );
}

#[tokio::test]
async fn metadata_records_the_short_channel_key() {
    let harness = Harness::new().await;

    assert_eq!(
        harness.loader(UpdateChannel::Dev).metadata()["channel"],
        "dev"
    );
    assert_eq!(
        harness.loader(UpdateChannel::Rc).metadata()["channel"],
        "rc"
    );
}

#[tokio::test]
async fn an_index_that_is_not_an_object_reads_as_a_missing_channel() {
    let harness = Harness::serving_index(json!(["release"])).await;
    let mut loader = harness.loader(UpdateChannel::Release);

    let error = loader.load().await.unwrap_err();

    assert_eq!(error.to_string(), "Invalid channel: UpdateChannel.RELEASE");
}
