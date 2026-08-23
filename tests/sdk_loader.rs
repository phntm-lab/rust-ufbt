use std::sync::{Arc, Mutex};

use rust_ufbt::log::{LogEvent, LogSink, Logger};
use rust_ufbt::net::FileFetcher;
use rust_ufbt::sdk::{LocalLoader, SdkLoader, UrlLoader, VERSION_UNKNOWN};
use rust_ufbt::state::ALWAYS_UPDATE_VERSIONS;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Default)]
struct CapturingSink {
    messages: Mutex<Vec<String>>,
}

impl CapturingSink {
    fn messages(&self) -> Vec<String> {
        self.messages.lock().unwrap().clone()
    }
}

impl LogSink for CapturingSink {
    fn on_event(&self, event: &LogEvent) {
        if let LogEvent::Message { message, .. } = event {
            self.messages.lock().unwrap().push(message.clone());
        }
    }
}

fn logger_with_sink() -> (Arc<Logger>, Arc<CapturingSink>) {
    let sink = Arc::new(CapturingSink::default());
    let logger = Arc::new(Logger::new());
    logger.add_sink(sink.clone());
    (logger, sink)
}

fn fetcher(logger: Arc<Logger>) -> Arc<FileFetcher> {
    Arc::new(FileFetcher::new(logger).unwrap())
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

#[test]
fn version_unknown_is_the_first_always_updated_version() {
    assert_eq!(VERSION_UNKNOWN, "unknown");
    assert_eq!(ALWAYS_UPDATE_VERSIONS, ["unknown", "local"]);
    assert_eq!(ALWAYS_UPDATE_VERSIONS[0], VERSION_UNKNOWN);
}

#[test]
fn the_mode_keys_name_the_deploy_task_modes() {
    assert_eq!(UrlLoader::MODE_KEY, "url");
    assert_eq!(LocalLoader::MODE_KEY, "local");
}

#[tokio::test]
async fn a_url_loader_describes_itself_with_an_unknown_version() {
    let (logger, _) = logger_with_sink();
    let loader = UrlLoader::new(
        logger.clone(),
        fetcher(logger),
        "downloads",
        "https://example.test/sdk.zip",
    );

    assert_eq!(loader.mode_key(), "url");
    assert_eq!(loader.url(), "https://example.test/sdk.zip");
    assert_eq!(
        metadata_pairs(&loader),
        pairs(&[
            ("mode", "url"),
            ("url", "https://example.test/sdk.zip"),
            ("version", "unknown"),
        ])
    );
}

#[tokio::test]
async fn a_local_loader_describes_itself_with_an_unknown_version() {
    let (logger, _) = logger_with_sink();
    let loader = LocalLoader::new(logger, "bundles/sdk.zip");

    assert_eq!(loader.mode_key(), "local");
    assert_eq!(
        metadata_pairs(&loader),
        pairs(&[
            ("mode", "local"),
            ("file_path", &loader.file_path().to_string_lossy(),),
            ("version", "unknown"),
        ])
    );
}

#[tokio::test]
async fn loading_a_url_loader_resolves_nothing_and_leaves_the_metadata_alone() {
    let (logger, sink) = logger_with_sink();
    let mut loader = UrlLoader::new(
        logger.clone(),
        fetcher(logger),
        "downloads",
        "https://example.test/sdk.zip",
    );

    let before = metadata_pairs(&loader);
    loader.load().await.unwrap();

    assert_eq!(metadata_pairs(&loader), before);
    assert!(sink.messages().is_empty());
}

#[tokio::test]
async fn loading_a_local_loader_resolves_nothing_and_leaves_the_metadata_alone() {
    let (logger, sink) = logger_with_sink();
    let mut loader = LocalLoader::new(logger, "bundles/sdk.zip");

    let before = metadata_pairs(&loader);
    loader.load().await.unwrap();

    assert_eq!(metadata_pairs(&loader), before);
    assert!(sink.messages().is_empty());
}

#[tokio::test]
async fn a_local_loader_hands_out_the_path_it_was_given() {
    let (logger, sink) = logger_with_sink();
    let temp = tempfile::tempdir().unwrap();
    let bundle = temp.path().join("sdk.zip");
    std::fs::write(&bundle, b"not really a bundle").unwrap();

    let loader = LocalLoader::new(logger, &bundle);
    let component = loader.get_sdk_component("f7").await.unwrap();

    assert_eq!(component, bundle);
    assert_eq!(
        sink.messages(),
        vec![format!("Loading SDK from {}", bundle.display())]
    );
}

#[tokio::test]
async fn a_local_loader_hands_out_a_path_that_does_not_exist() {
    let (logger, _) = logger_with_sink();
    let loader = LocalLoader::new(logger, "nowhere/sdk.zip");

    let component = loader.get_sdk_component("f7").await.unwrap();

    assert_eq!(component, std::path::Path::new("nowhere/sdk.zip"));
}

#[tokio::test]
async fn a_local_loader_ignores_the_hardware_target() {
    let (logger, _) = logger_with_sink();
    let loader = LocalLoader::new(logger, "bundles/sdk.zip");

    let for_f7 = loader.get_sdk_component("f7").await.unwrap();
    let for_f18 = loader.get_sdk_component("f18").await.unwrap();

    assert_eq!(for_f7, for_f18);
}

#[tokio::test]
async fn a_url_loader_downloads_the_bundle_into_the_download_directory() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sdk.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bundle".to_vec()))
        .mount(&server)
        .await;

    let (logger, sink) = logger_with_sink();
    let temp = tempfile::tempdir().unwrap();
    let url = format!("{}/sdk.zip", server.uri());
    let loader = UrlLoader::new(logger.clone(), fetcher(logger), temp.path(), &url);

    let component = loader.get_sdk_component("f7").await.unwrap();

    assert_eq!(component, temp.path().join("sdk.zip"));
    assert_eq!(std::fs::read(&component).unwrap(), b"bundle");
    assert!(
        sink.messages()
            .contains(&format!("Fetching SDK from {url}"))
    );
}

#[tokio::test]
async fn a_url_loader_leaves_no_part_file_behind() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sdk.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bundle".to_vec()))
        .mount(&server)
        .await;

    let (logger, _) = logger_with_sink();
    let temp = tempfile::tempdir().unwrap();
    let loader = UrlLoader::new(
        logger.clone(),
        fetcher(logger),
        temp.path(),
        format!("{}/sdk.zip", server.uri()),
    );

    loader.get_sdk_component("f7").await.unwrap();

    assert!(!temp.path().join("sdk.zip.part").exists());
}

#[tokio::test]
async fn a_url_loader_reports_a_download_failure() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sdk.zip"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let (logger, _) = logger_with_sink();
    let temp = tempfile::tempdir().unwrap();
    let loader = UrlLoader::new(
        logger.clone(),
        fetcher(logger),
        temp.path(),
        format!("{}/sdk.zip", server.uri()),
    );

    let error = loader.get_sdk_component("f7").await.unwrap_err();

    assert_eq!(error.to_string(), "HTTP Error 404: Not Found");
}

#[tokio::test]
async fn a_url_loader_ignores_the_hardware_target() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/sdk.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bundle".to_vec()))
        .mount(&server)
        .await;

    let (logger, _) = logger_with_sink();
    let temp = tempfile::tempdir().unwrap();
    let loader = UrlLoader::new(
        logger.clone(),
        fetcher(logger),
        temp.path(),
        format!("{}/sdk.zip", server.uri()),
    );

    let for_f7 = loader.get_sdk_component("f7").await.unwrap();
    let for_f18 = loader.get_sdk_component("f18").await.unwrap();

    assert_eq!(for_f7, for_f18);
}

#[tokio::test]
async fn loaders_are_usable_as_trait_objects() {
    let (logger, _) = logger_with_sink();
    let loaders: Vec<Box<dyn SdkLoader>> = vec![
        Box::new(UrlLoader::new(
            logger.clone(),
            fetcher(logger.clone()),
            "downloads",
            "https://example.test/sdk.zip",
        )),
        Box::new(LocalLoader::new(logger, "bundles/sdk.zip")),
    ];

    let modes: Vec<&str> = loaders.iter().map(|loader| loader.mode_key()).collect();

    assert_eq!(modes, ["url", "local"]);
}
