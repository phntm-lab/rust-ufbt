use std::io;

/// Failure of an HTTP download.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FetchError {
    /// The server answered with a status other than `200 OK`.
    #[error("HTTP Error {status}: {reason}")]
    Http {
        /// Status code returned by the server.
        status: u16,
        /// Reason phrase returned by the server.
        reason: String,
    },
    /// The TLS backend could not be initialized.
    #[error("TLS initialization failed: {0}")]
    Tls(String),
    /// The request could not be performed.
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    /// The downloaded data could not be written to disk.
    #[error(transparent)]
    Io(#[from] io::Error),
}
