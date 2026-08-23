use std::sync::{Arc, Mutex};

use rust_ufbt::log::{LogEvent, LogLevel, LogSink, Logger};
use rust_ufbt::net::FileFetcher;
use rust_ufbt::sdk::{BranchLoader, SdkError, SdkLoader};
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

    async fn serving_listing(listing: &str) -> Self {
        let harness = Self::new().await;
        harness
            .serve(ResponseTemplate::new(200).set_body_string(listing.to_string()))
            .await;
        harness
    }

    async fn serve(&self, response: ResponseTemplate) {
        Mock::given(method("GET"))
            .and(path("/dev/"))
            .respond_with(response)
            .mount(&self.server)
            .await;
    }

    fn loader(&self) -> BranchLoader {
        BranchLoader::new(
            self.logger.clone(),
            self.fetcher.clone(),
            self.temp.path(),
            "dev",
        )
        .with_branch_root(self.server.uri())
    }

    async fn loaded(&self) -> BranchLoader {
        let mut loader = self.loader();
        loader.load().await.unwrap();
        loader
    }
}

const APACHE_LISTING: &str = r#"
<html><head><title>Index of /builds/firmware/dev</title></head>
<body><h1>Index of /builds/firmware/dev</h1><pre>
<a href="../">../</a>
<A HREF='flipper-z-f7-sdk-1.3.4.zip'>flipper-z-f7-sdk-1.3.4.zip</A>
<a HrEf="flipper-z-f7-update-1.3.4.tgz">flipper-z-f7-update-1.3.4.tgz</a>
<a href="flipper-z-f7-firmware-1.3.4.elf.map">map</a>
<a href="flipper-z-f7-full-1.3.4.bin">bin</a>
<a href="flipper-z-f7-mystery-1.3.4.xyz">unknown</a>
<a href="flipper-z-f18-sdk-1.3.4.zip">f18 sdk</a>
<a href='some-other-file.txt'>other</a>
<a href="flipper-z-f7-resources-1.3.4.tgz">res</a>
</pre></body></html>
"#;

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
async fn the_branch_root_and_the_mode_key_match_the_original() {
    assert_eq!(
        BranchLoader::UPDATE_SERVER_BRANCH_ROOT,
        "https://update.flipperzero.one/builds/firmware"
    );
    assert_eq!(BranchLoader::MODE_KEY, "branch");
}

#[tokio::test]
async fn the_branch_url_is_the_root_the_branch_and_a_trailing_separator() {
    let harness = Harness::new().await;
    let loader = BranchLoader::new(
        harness.logger.clone(),
        harness.fetcher.clone(),
        harness.temp.path(),
        "feature/x",
    );

    assert_eq!(loader.branch(), "feature/x");
    assert_eq!(
        loader.branch_root(),
        BranchLoader::UPDATE_SERVER_BRANCH_ROOT
    );
    assert_eq!(
        loader.branch_url(),
        "https://update.flipperzero.one/builds/firmware/feature/x/"
    );
    assert_eq!(loader.mode_key(), "branch");
}

#[tokio::test]
async fn load_reads_a_typical_listing() {
    let harness = Harness::serving_listing(APACHE_LISTING).await;
    let loader = harness.loaded().await;

    assert_eq!(loader.version(), Some("1.3.4"));
    assert!(harness.sink.messages().contains(&format!(
        "Fetching branch index {}/dev/",
        harness.server.uri()
    )));
    assert!(
        harness
            .sink
            .messages()
            .contains(&"Found version 1.3.4".to_string())
    );
}

#[tokio::test]
async fn load_accepts_both_quote_styles_and_any_tag_case() {
    let harness = Harness::serving_listing(APACHE_LISTING).await;
    for name in ["flipper-z-f7-sdk-1.3.4.zip", "flipper-z-f18-sdk-1.3.4.zip"] {
        Mock::given(method("GET"))
            .and(path(format!("/dev/{name}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(name.as_bytes().to_vec()))
            .mount(&harness.server)
            .await;
    }
    let loader = harness.loaded().await;

    let upper_case_single_quoted = loader.get_sdk_component("f7").await.unwrap();
    let plain = loader.get_sdk_component("f18").await.unwrap();

    assert_eq!(
        upper_case_single_quoted,
        harness.temp.path().join("flipper-z-f7-sdk-1.3.4.zip")
    );
    assert_eq!(
        plain,
        harness.temp.path().join("flipper-z-f18-sdk-1.3.4.zip")
    );
}

#[tokio::test]
async fn load_skips_map_files() {
    let harness = Harness::serving_listing(
        r#"<a href="flipper-z-f7-firmware-1.3.4.elf.map">map</a>
           <a href="flipper-z-f7-sdk-1.3.4.zip">sdk</a>"#,
    )
    .await;
    let loader = harness.loaded().await;

    assert_eq!(loader.version(), Some("1.3.4"));
}

#[tokio::test]
async fn a_map_file_alone_leaves_the_version_unknown() {
    let harness =
        Harness::serving_listing(r#"<a href="flipper-z-f7-firmware-1.3.4.elf.map">map</a>"#).await;
    let loader = harness.loaded().await;

    assert_eq!(loader.version(), None);
    assert!(
        harness
            .sink
            .messages()
            .contains(&"Found version null".to_string())
    );
    assert_eq!(loader.metadata()["version"], "unknown");
}

#[tokio::test]
async fn a_file_of_an_unknown_type_still_settles_the_version() {
    let harness =
        Harness::serving_listing(r#"<a href="flipper-z-f7-mystery-1.3.4.xyz">x</a>"#).await;
    let loader = harness.loaded().await;

    assert_eq!(loader.version(), Some("1.3.4"));

    let error = loader.get_sdk_component("f7").await.unwrap_err();
    assert!(matches!(error, SdkError::SdkBundleNotFound { .. }));
}

#[tokio::test]
async fn a_listing_without_recognisable_names_leaves_the_version_unknown() {
    let harness = Harness::serving_listing(
        r#"<a href="../">up</a><a href="readme.txt">readme</a>
           <a href="prefixed-flipper-z-f7-sdk-1.3.4.zip">not a prefix match</a>"#,
    )
    .await;
    let loader = harness.loaded().await;

    assert_eq!(loader.version(), None);
}

#[tokio::test]
async fn load_reports_two_different_versions() {
    let harness = Harness::serving_listing(
        r#"<a href="flipper-z-f7-sdk-1.3.4.zip">a</a>
           <a href="flipper-z-f7-update-1.4.0.tgz">b</a>"#,
    )
    .await;
    let mut loader = harness.loader();

    let error = loader.load().await.unwrap_err();

    assert!(matches!(error, SdkError::MultipleVersions { .. }));
    assert_eq!(
        error.to_string(),
        "Found multiple versions: 1.3.4 and 1.4.0"
    );
}

#[tokio::test]
async fn a_version_that_extends_the_first_one_is_accepted() {
    let harness = Harness::serving_listing(
        r#"<a href="flipper-z-f7-sdk-1.0.zip">a</a>
           <a href="flipper-z-f7-update-1.0.1.tgz">b</a>"#,
    )
    .await;
    let loader = harness.loaded().await;

    assert_eq!(loader.version(), Some("1.0"));
}

#[tokio::test]
async fn a_version_the_first_one_extends_is_rejected() {
    let harness = Harness::serving_listing(
        r#"<a href="flipper-z-f7-sdk-1.0.1.zip">a</a>
           <a href="flipper-z-f7-update-1.0.tgz">b</a>"#,
    )
    .await;
    let mut loader = harness.loader();

    let error = loader.load().await.unwrap_err();

    assert_eq!(error.to_string(), "Found multiple versions: 1.0.1 and 1.0");
}

#[tokio::test]
async fn get_sdk_component_reports_a_target_the_listing_does_not_carry() {
    let harness = Harness::serving_listing(APACHE_LISTING).await;
    let loader = harness.loaded().await;

    let error = loader.get_sdk_component("f42").await.unwrap_err();

    assert!(matches!(error, SdkError::SdkBundleNotFound { .. }));
    assert_eq!(error.to_string(), "SDK bundle not found for f42");
}

#[tokio::test]
async fn get_sdk_component_before_load_reports_a_missing_bundle() {
    let harness = Harness::new().await;
    let loader = harness.loader();

    let error = loader.get_sdk_component("f7").await.unwrap_err();

    assert_eq!(error.to_string(), "SDK bundle not found for f7");
}

#[tokio::test]
async fn get_sdk_component_downloads_the_bundle_from_the_branch_url() {
    let harness = Harness::serving_listing(APACHE_LISTING).await;
    Mock::given(method("GET"))
        .and(path("/dev/flipper-z-f7-sdk-1.3.4.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bundle".to_vec()))
        .mount(&harness.server)
        .await;
    let loader = harness.loaded().await;

    let component = loader.get_sdk_component("f7").await.unwrap();

    assert_eq!(
        component,
        harness.temp.path().join("flipper-z-f7-sdk-1.3.4.zip")
    );
    assert_eq!(std::fs::read(&component).unwrap(), b"bundle");
}

#[tokio::test]
async fn load_reports_a_failed_download_of_the_listing() {
    let harness = Harness::new().await;
    harness.serve(ResponseTemplate::new(404)).await;
    let mut loader = harness.loader();

    let error = loader.load().await.unwrap_err();

    assert!(matches!(error, SdkError::Fetch(_)));
    assert_eq!(error.to_string(), "HTTP Error 404: Not Found");
}

#[tokio::test]
async fn metadata_reports_the_branch_the_version_and_the_root() {
    let harness = Harness::serving_listing(APACHE_LISTING).await;
    let root = harness.server.uri();
    let mut loader = harness.loader();

    assert_eq!(
        metadata_pairs(&loader),
        pairs(&[
            ("mode", "branch"),
            ("branch", "dev"),
            ("version", "unknown"),
            ("branch_root", &root),
        ])
    );

    loader.load().await.unwrap();

    assert_eq!(
        metadata_pairs(&loader),
        pairs(&[
            ("mode", "branch"),
            ("branch", "dev"),
            ("version", "1.3.4"),
            ("branch_root", &root),
        ])
    );
}
