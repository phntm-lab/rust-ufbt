//! HTTP downloads with progress reporting.

mod error;
mod fetcher;

pub use error::FetchError;
pub use fetcher::{FileFetcher, USER_AGENT};
