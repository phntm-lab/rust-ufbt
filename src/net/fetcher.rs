use std::fmt;
use std::sync::Arc;

use reqwest::{Client, Response, StatusCode, header};
use rustls::ClientConfig;
use rustls_platform_verifier::BuilderVerifierExt;

use super::FetchError;
use crate::log::Logger;

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

    #[allow(dead_code)]
    pub(crate) async fn open(&self, url: &str) -> Result<Response, FetchError> {
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
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn fetcher() -> FileFetcher {
        FileFetcher::new(Arc::new(Logger::new())).expect("client builds")
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
}
