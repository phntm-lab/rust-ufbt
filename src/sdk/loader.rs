use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use indexmap::IndexMap;

use super::SdkError;
use super::deploy_task::{LOCAL_MODE_KEY, URL_MODE_KEY};
use crate::log::Logger;
use crate::net::FileFetcher;

/// Version recorded for an SDK whose real version could not be determined.
///
/// It is the first entry of
/// [`state::ALWAYS_UPDATE_VERSIONS`](crate::state::ALWAYS_UPDATE_VERSIONS), so a deployment
/// carrying it is always replaced rather than compared.
pub const VERSION_UNKNOWN: &str = "unknown";

pub(super) const MODE_METADATA_KEY: &str = "mode";
pub(super) const VERSION_METADATA_KEY: &str = "version";
const URL_METADATA_KEY: &str = "url";
const FILE_PATH_METADATA_KEY: &str = "file_path";

/// Boxed future returned by the asynchronous methods of [`SdkLoader`].
///
/// [`SdkLoader`] is used as a trait object, which an `async fn` in a trait cannot be, so the
/// futures are boxed explicitly.
pub type SdkFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, SdkError>> + Send + 'a>>;

/// Source an SDK bundle is obtained from.
///
/// A loader is resolved once through [`load`](SdkLoader::load) and then asked for the bundle
/// of a hardware target through [`get_sdk_component`](SdkLoader::get_sdk_component). Loaders
/// that resolve nothing up front implement `load` as a no-op, so calling it is always
/// correct and never optional.
pub trait SdkLoader: Send + Sync {
    /// Key of the deploy task mode this loader serves.
    fn mode_key(&self) -> &'static str;

    /// Resolves which SDK version this loader will hand out.
    ///
    /// # Errors
    ///
    /// Returns an error when the index describing the available versions cannot be read or
    /// does not describe a usable version.
    fn load(&mut self) -> SdkFuture<'_, ()>;

    /// Obtains the SDK bundle built for `target`, downloading it if necessary.
    ///
    /// # Errors
    ///
    /// Returns an error when no bundle is published for `target`, or when the bundle cannot
    /// be downloaded.
    fn get_sdk_component<'a>(&'a self, target: &'a str) -> SdkFuture<'a, PathBuf>;

    /// Description of the deployment this loader performs, recorded in the deployment state
    /// next to the unpacked SDK.
    fn metadata(&self) -> IndexMap<String, String>;
}

/// Loads an SDK bundle from an explicit address.
///
/// The address names the bundle directly, so there is nothing to resolve and the version is
/// always [`VERSION_UNKNOWN`].
pub struct UrlLoader {
    logger: Arc<Logger>,
    fetcher: Arc<FileFetcher>,
    download_dir: PathBuf,
    url: String,
}

impl UrlLoader {
    /// Key of the deploy task mode this loader serves.
    pub const MODE_KEY: &'static str = URL_MODE_KEY;

    /// Creates a loader that downloads the bundle at `url` into `download_dir`.
    pub fn new(
        logger: Arc<Logger>,
        fetcher: Arc<FileFetcher>,
        download_dir: impl AsRef<Path>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            logger,
            fetcher,
            download_dir: download_dir.as_ref().to_path_buf(),
            url: url.into(),
        }
    }

    /// Address the bundle is downloaded from.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl SdkLoader for UrlLoader {
    fn mode_key(&self) -> &'static str {
        Self::MODE_KEY
    }

    fn load(&mut self) -> SdkFuture<'_, ()> {
        Box::pin(std::future::ready(Ok(())))
    }

    fn get_sdk_component<'a>(&'a self, _target: &'a str) -> SdkFuture<'a, PathBuf> {
        Box::pin(async move {
            self.logger.info(&format!("Fetching SDK from {}", self.url));
            Ok(self
                .fetcher
                .fetch_file(&self.url, &self.download_dir, "", false)
                .await?)
        })
    }

    fn metadata(&self) -> IndexMap<String, String> {
        IndexMap::from([
            (MODE_METADATA_KEY.to_string(), Self::MODE_KEY.to_string()),
            (URL_METADATA_KEY.to_string(), self.url.clone()),
            (
                VERSION_METADATA_KEY.to_string(),
                VERSION_UNKNOWN.to_string(),
            ),
        ])
    }
}

/// Loads an SDK bundle already present on this machine.
///
/// The file is used where it lies — nothing is downloaded and nothing is copied — and the
/// version is always [`VERSION_UNKNOWN`].
pub struct LocalLoader {
    logger: Arc<Logger>,
    file_path: PathBuf,
}

impl LocalLoader {
    /// Key of the deploy task mode this loader serves.
    pub const MODE_KEY: &'static str = LOCAL_MODE_KEY;

    /// Creates a loader that hands out the bundle stored at `file_path`.
    pub fn new(logger: Arc<Logger>, file_path: impl AsRef<Path>) -> Self {
        Self {
            logger,
            file_path: file_path.as_ref().to_path_buf(),
        }
    }

    /// Location of the bundle.
    #[must_use]
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

impl SdkLoader for LocalLoader {
    fn mode_key(&self) -> &'static str {
        Self::MODE_KEY
    }

    fn load(&mut self) -> SdkFuture<'_, ()> {
        Box::pin(std::future::ready(Ok(())))
    }

    fn get_sdk_component<'a>(&'a self, _target: &'a str) -> SdkFuture<'a, PathBuf> {
        Box::pin(async move {
            self.logger
                .info(&format!("Loading SDK from {}", self.file_path.display()));
            Ok(self.file_path.clone())
        })
    }

    fn metadata(&self) -> IndexMap<String, String> {
        IndexMap::from([
            (MODE_METADATA_KEY.to_string(), Self::MODE_KEY.to_string()),
            (
                FILE_PATH_METADATA_KEY.to_string(),
                self.file_path.to_string_lossy().into_owned(),
            ),
            (
                VERSION_METADATA_KEY.to_string(),
                VERSION_UNKNOWN.to_string(),
            ),
        ])
    }
}
