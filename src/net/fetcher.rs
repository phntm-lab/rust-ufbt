use std::borrow::Cow;
use std::fmt;
use std::io;
use std::path::{MAIN_SEPARATOR_STR, Path, PathBuf};
use std::sync::Arc;

use reqwest::{Client, Response, StatusCode, Url, header};
use rustls::ClientConfig;
use rustls_platform_verifier::BuilderVerifierExt;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::FetchError;
use crate::log::{Logger, ProgressTask};

/// Value of the `User-Agent` header sent with every request.
pub const USER_AGENT: &str = "uFBT SDKLoader/0.2";

/// Downloads files over HTTP, reporting progress through a [`Logger`].
pub struct FileFetcher {
    logger: Arc<Logger>,
    client: Client,
}

impl FileFetcher {
    /// Creates a fetcher with a client backed by `rustls` and the platform certificate
    /// store.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::Tls`] when the TLS configuration cannot be assembled and
    /// [`FetchError::Request`] when the client itself cannot be built.
    pub fn new(logger: Arc<Logger>) -> Result<Self, FetchError> {
        Ok(Self::with_client(logger, default_client()?))
    }

    /// Creates a fetcher over a caller-supplied client.
    ///
    /// The `User-Agent` header is set on every request regardless of the client
    /// configuration.
    ///
    /// # Panics
    ///
    /// Nothing here panics, but note that building a [`Client`] through
    /// [`Client::new`] does when no process-wide `rustls` crypto provider has been
    /// installed. Prefer [`FileFetcher::new`], which configures the provider itself.
    pub fn with_client(logger: Arc<Logger>, client: Client) -> Self {
        Self { logger, client }
    }

    /// Logger the fetcher reports to.
    pub fn logger(&self) -> &Arc<Logger> {
        &self.logger
    }

    /// Client used for every request.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Releases the underlying client and its connection pool.
    pub fn close(self) {}

    /// Reads the body served at `url` and decodes it as UTF-8.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::Http`] for any status other than `200 OK`,
    /// [`FetchError::Request`] when the request itself fails and [`FetchError::Io`] with
    /// [`io::ErrorKind::InvalidData`] when the body is not valid UTF-8.
    pub async fn read_as_string(&self, url: &str) -> Result<String, FetchError> {
        let body = self.open(url).await?.bytes().await?;
        String::from_utf8(body.into())
            .map_err(|e| FetchError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))
    }

    /// Downloads `url` into `download_dir`, reporting progress under `progress_title`.
    ///
    /// The file is named after the last path segment of the URL, percent-decoded; a URL
    /// without path segments yields an empty name. `download_dir` is created if missing.
    ///
    /// The task reports `content_length` as its total when the server announces one, and is
    /// indeterminate otherwise.
    ///
    /// With `use_part_file` the body is written to `<target>.part` and moved onto the target
    /// once the transfer completes, replacing an existing target. Without it the target is
    /// written in place.
    ///
    /// # Errors
    ///
    /// Returns [`FetchError::InvalidUrl`] when `url` cannot be parsed,
    /// [`FetchError::Http`] for any status other than `200 OK`, [`FetchError::Request`]
    /// when the transfer is interrupted and [`FetchError::Io`] when the file cannot be
    /// written or moved. A failure during the transfer aborts the progress task with the
    /// error text before it is returned.
    pub async fn fetch_file(
        &self,
        url: &str,
        download_dir: impl AsRef<Path>,
        progress_title: &str,
        use_part_file: bool,
    ) -> Result<PathBuf, FetchError> {
        self.logger.debug(&format!("Fetching {url}"));

        let parsed = Url::parse(url).map_err(|_| FetchError::InvalidUrl {
            url: url.to_owned(),
        })?;
        let file_name = file_name_from_url(&parsed);
        let download_dir = download_dir.as_ref();
        fs::create_dir_all(download_dir).await?;

        let target = join_file_name(download_dir, &file_name);
        let sink = if use_part_file {
            with_part_suffix(&target)
        } else {
            target.clone()
        };

        let mut response = self.open(url).await?;
        let total = response.content_length().filter(|&length| length > 0);
        let mut task = self.logger.progress(progress_title, total);

        let mut file = fs::File::create(&sink).await?;
        match write_body(&mut response, &mut file, &mut task).await {
            Ok(()) => {
                drop(file);
                task.finish();
            }
            Err(error) => {
                drop(file);
                task.fail_with_message(&error.to_string());
                return Err(error);
            }
        }

        if use_part_file {
            if fs::symlink_metadata(&target).await.is_ok() {
                fs::remove_file(&target).await?;
            }
            fs::rename(&sink, &target).await?;
        }
        Ok(target)
    }

    async fn open(&self, url: &str) -> Result<Response, FetchError> {
        let response = self
            .client
            .get(url)
            .header(header::USER_AGENT, USER_AGENT)
            .send()
            .await?;
        let status = response.status();
        if status != StatusCode::OK {
            return Err(FetchError::Http {
                status: status.as_u16(),
                reason: status.canonical_reason().unwrap_or_default().to_owned(),
            });
        }
        Ok(response)
    }
}

impl fmt::Debug for FileFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileFetcher").finish_non_exhaustive()
    }
}

async fn write_body(
    response: &mut Response,
    file: &mut fs::File,
    task: &mut ProgressTask<'_>,
) -> Result<(), FetchError> {
    let mut received = 0u64;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        task.update(Some(received), None, None);
    }
    file.flush().await?;
    Ok(())
}

fn file_name_from_url(url: &Url) -> String {
    let Some(last) = url.path_segments().and_then(Iterator::last) else {
        return String::new();
    };
    decode_component(&decode_component(last)).into_owned()
}

fn decode_component(text: &str) -> Cow<'_, str> {
    percent_encoding::percent_decode_str(text)
        .decode_utf8()
        .unwrap_or(Cow::Borrowed(text))
}

fn join_file_name(dir: &Path, file_name: &str) -> PathBuf {
    let mut path = dir.as_os_str().to_owned();
    path.push(MAIN_SEPARATOR_STR);
    path.push(file_name);
    PathBuf::from(path)
}

fn with_part_suffix(target: &Path) -> PathBuf {
    let mut path = target.as_os_str().to_owned();
    path.push(".part");
    PathBuf::from(path)
}

fn default_client() -> Result<Client, FetchError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| FetchError::Tls(e.to_string()))?
        .with_platform_verifier()
        .map_err(|e| FetchError::Tls(e.to_string()))?
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Client::builder().use_preconfigured_tls(config).build()?)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tempfile::TempDir;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::log::{LogEvent, LogLevel, LogSink, Progress, ProgressState};

    #[derive(Default)]
    struct Capture(Mutex<Vec<LogEvent>>);

    impl Capture {
        fn events(&self) -> Vec<LogEvent> {
            self.0.lock().unwrap().clone()
        }

        fn progress(&self) -> Vec<Progress> {
            self.events()
                .into_iter()
                .filter_map(|event| match event {
                    LogEvent::Progress { progress, .. } => Some(progress),
                    _ => None,
                })
                .collect()
        }

        fn messages(&self) -> Vec<String> {
            self.events()
                .into_iter()
                .filter_map(|event| match event {
                    LogEvent::Message { message, .. } => Some(message),
                    _ => None,
                })
                .collect()
        }
    }

    impl LogSink for Capture {
        fn on_event(&self, event: &LogEvent) {
            self.0.lock().unwrap().push(event.clone());
        }
    }

    fn fetcher() -> FileFetcher {
        FileFetcher::new(Arc::new(Logger::new())).expect("client builds")
    }

    fn watched_fetcher() -> (FileFetcher, Arc<Capture>) {
        let capture = Arc::new(Capture::default());
        let logger = Logger::builder()
            .level(LogLevel::Debug)
            .sink(capture.clone())
            .build();
        (
            FileFetcher::new(Arc::new(logger)).expect("client builds"),
            capture,
        )
    }

    async fn serving(body: &str) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        server
    }

    async fn truncated_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let address = listener.local_addr().expect("has an address");
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut request = [0u8; 1024];
                let _ = socket.read(&mut request).await;
                let _ = socket
                    .write_all(
                        b"HTTP/1.1 200 OK
Content-Length: 1000

",
                    )
                    .await;
                let _ = socket.write_all(&[b'x'; 10]).await;
            }
        });
        format!("http://{address}/big.bin")
    }

    #[tokio::test]
    async fn opening_a_url_that_answers_with_ok_yields_the_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/file.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_string("payload"))
            .mount(&server)
            .await;

        let response = fetcher()
            .open(&format!("{}/file.bin", server.uri()))
            .await
            .expect("request succeeds");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "payload");
    }

    #[tokio::test]
    async fn opening_a_url_that_answers_with_not_found_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let error = fetcher()
            .open(&format!("{}/missing", server.uri()))
            .await
            .expect_err("request fails");

        assert!(matches!(
            error,
            FetchError::Http { status: 404, ref reason } if reason == "Not Found"
        ));
        assert_eq!(error.to_string(), "HTTP Error 404: Not Found");
    }

    #[tokio::test]
    async fn opening_a_url_that_answers_with_server_error_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let error = fetcher()
            .open(&format!("{}/boom", server.uri()))
            .await
            .expect_err("request fails");

        assert_eq!(error.to_string(), "HTTP Error 500: Internal Server Error");
    }

    #[tokio::test]
    async fn a_status_without_a_canonical_reason_yields_an_empty_reason() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(599))
            .mount(&server)
            .await;

        let error = fetcher()
            .open(&format!("{}/odd", server.uri()))
            .await
            .expect_err("request fails");

        assert_eq!(error.to_string(), "HTTP Error 599: ");
    }

    #[tokio::test]
    async fn every_request_carries_the_user_agent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("user-agent", USER_AGENT))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        fetcher()
            .open(&format!("{}/agent", server.uri()))
            .await
            .expect("request matches the user agent");
    }

    #[test]
    fn the_user_agent_is_the_one_the_sdk_loader_announces() {
        assert_eq!(USER_AGENT, "uFBT SDKLoader/0.2");
    }

    #[tokio::test]
    async fn an_unreachable_host_fails_with_a_request_error() {
        let error = fetcher()
            .open("http://127.0.0.1:1/gone")
            .await
            .expect_err("request fails");

        assert!(matches!(error, FetchError::Request(_)));
    }

    #[tokio::test]
    async fn reading_a_body_as_string_decodes_utf8() {
        let server = serving("привет — ok").await;

        let body = fetcher()
            .read_as_string(&format!("{}/text", server.uri()))
            .await
            .expect("body is read");

        assert_eq!(body, "привет — ok");
    }

    #[tokio::test]
    async fn reading_a_body_that_is_not_utf8_fails_as_invalid_data() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0xff, 0xfe]))
            .mount(&server)
            .await;

        let error = fetcher()
            .read_as_string(&format!("{}/bytes", server.uri()))
            .await
            .expect_err("decoding fails");

        match error {
            FetchError::Io(e) => assert_eq!(e.kind(), io::ErrorKind::InvalidData),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test]
    async fn fetching_a_file_writes_it_and_returns_its_path() {
        let server = serving("payload").await;
        let dir = TempDir::new().expect("temp dir");
        let target = dir.path().join("sub").join("file.bin");

        let path = fetcher()
            .fetch_file(
                &format!("{}/file.bin", server.uri()),
                dir.path().join("sub"),
                "Downloading",
                false,
            )
            .await
            .expect("file is fetched");

        assert_eq!(path, target);
        assert_eq!(fs::read_to_string(&path).await.unwrap(), "payload");
        assert!(!with_part_suffix(&target).exists());
    }

    #[tokio::test]
    async fn fetching_a_file_logs_the_url_it_reads() {
        let server = serving("payload").await;
        let dir = TempDir::new().expect("temp dir");
        let (fetcher, capture) = watched_fetcher();
        let url = format!("{}/file.bin", server.uri());

        fetcher
            .fetch_file(&url, dir.path(), "Downloading", false)
            .await
            .expect("file is fetched");

        assert_eq!(capture.messages(), vec![format!("Fetching {url}")]);
    }

    #[tokio::test]
    async fn a_known_content_length_drives_a_determinate_task() {
        let server = serving("payload").await;
        let dir = TempDir::new().expect("temp dir");
        let (fetcher, capture) = watched_fetcher();

        fetcher
            .fetch_file(
                &format!("{}/file.bin", server.uri()),
                dir.path(),
                "Downloading",
                false,
            )
            .await
            .expect("file is fetched");

        let progress = capture.progress();
        let states: Vec<_> = progress.iter().map(Progress::state).collect();
        assert_eq!(
            states,
            vec![
                ProgressState::Started,
                ProgressState::Running,
                ProgressState::Finished
            ]
        );
        assert!(progress.iter().all(|p| p.title() == "Downloading"));
        assert!(progress.iter().all(|p| p.total() == Some(7)));
        let counters: Vec<_> = progress.iter().map(Progress::current).collect();
        assert_eq!(counters, vec![0, 7, 7]);
    }

    #[tokio::test]
    async fn a_missing_content_length_leaves_the_task_indeterminate() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("payload")
                    .insert_header("transfer-encoding", "chunked"),
            )
            .mount(&server)
            .await;
        let dir = TempDir::new().expect("temp dir");
        let (fetcher, capture) = watched_fetcher();

        fetcher
            .fetch_file(
                &format!("{}/file.bin", server.uri()),
                dir.path(),
                "Downloading",
                false,
            )
            .await
            .expect("file is fetched");

        let progress = capture.progress();
        assert!(progress.iter().all(Progress::indeterminate));
        assert_eq!(progress.first().unwrap().state(), ProgressState::Started);
        assert_eq!(progress.last().unwrap().state(), ProgressState::Finished);
    }

    #[tokio::test]
    async fn a_part_file_replaces_an_existing_target() {
        let server = serving("fresh").await;
        let dir = TempDir::new().expect("temp dir");
        let target = dir.path().join("file.bin");
        fs::write(&target, "stale").await.expect("target is seeded");

        let path = fetcher()
            .fetch_file(
                &format!("{}/file.bin", server.uri()),
                dir.path(),
                "Downloading",
                true,
            )
            .await
            .expect("file is fetched");

        assert_eq!(path, target);
        assert_eq!(fs::read_to_string(&target).await.unwrap(), "fresh");
        assert!(!with_part_suffix(&target).exists());
    }

    #[tokio::test]
    async fn an_interrupted_transfer_fails_the_task_and_returns_the_error() {
        let dir = TempDir::new().expect("temp dir");
        let (fetcher, capture) = watched_fetcher();

        let error = fetcher
            .fetch_file(&truncated_server().await, dir.path(), "Downloading", true)
            .await
            .expect_err("transfer is interrupted");

        let progress = capture.progress();
        let last = progress.last().expect("a task was started");
        assert_eq!(last.state(), ProgressState::Failed);
        assert_eq!(last.message(), Some(error.to_string().as_str()));
        assert!(!dir.path().join("big.bin").exists());
        assert!(with_part_suffix(&dir.path().join("big.bin")).exists());
    }

    #[tokio::test]
    async fn an_unparsable_url_is_rejected_before_anything_is_written() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path().join("downloads");

        let error = fetcher()
            .fetch_file("not a url", &root, "Downloading", false)
            .await
            .expect_err("the url is rejected");

        assert_eq!(error.to_string(), "Invalid URL: not a url");
        assert!(!root.exists());
    }

    #[test]
    fn a_file_name_comes_from_the_last_path_segment() {
        assert_eq!(name_of("http://host/dir/file.bin"), "file.bin");
        assert_eq!(name_of("http://host/dir/file.bin?v=1#frag"), "file.bin");
    }

    #[test]
    fn a_file_name_is_percent_decoded() {
        assert_eq!(name_of("http://host/dir/my%20file.bin"), "my file.bin");
        assert_eq!(name_of("http://host/%D1%84%D0%B0%D0%B9%D0%BB"), "файл");
    }

    #[test]
    fn a_file_name_is_percent_decoded_twice() {
        assert_eq!(name_of("http://host/a%2520b.bin"), "a b.bin");
    }

    #[test]
    fn a_url_without_path_segments_yields_an_empty_file_name() {
        assert_eq!(name_of("http://host"), "");
        assert_eq!(name_of("http://host/"), "");
        assert_eq!(name_of("http://host/dir/"), "");
    }

    fn name_of(url: &str) -> String {
        file_name_from_url(&Url::parse(url).expect("a valid url"))
    }
}
