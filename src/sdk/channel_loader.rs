use std::path::{Path, PathBuf};
use std::sync::Arc;

use indexmap::IndexMap;

use super::deploy_task::CHANNEL_MODE_KEY;
use super::loader::{
    MODE_METADATA_KEY, SdkFuture, SdkLoader, VERSION_METADATA_KEY, VERSION_UNKNOWN,
};
use super::{DirectoryIndex, FileType, IndexVersion, SdkError, UpdateChannel};
use crate::log::Logger;
use crate::net::FileFetcher;

const CHANNEL_METADATA_KEY: &str = "channel";
const JSON_INDEX_METADATA_KEY: &str = "json_index";

/// Loads an SDK bundle from an update channel of the directory index.
///
/// [`load`](SdkLoader::load) reads the index and picks the **first** version the channel
/// lists, which is the newest one the update server publishes. Until it has run,
/// [`version_info`](UpdateChannelLoader::version_info) and
/// [`get_sdk_component`](SdkLoader::get_sdk_component) report
/// [`SdkError::LoaderNotInitialized`].
pub struct UpdateChannelLoader {
    logger: Arc<Logger>,
    fetcher: Arc<FileFetcher>,
    download_dir: PathBuf,
    channel: UpdateChannel,
    json_index_url: String,
    version_info: Option<IndexVersion>,
}

impl UpdateChannelLoader {
    /// Key of the deploy task mode this loader serves.
    pub const MODE_KEY: &'static str = CHANNEL_MODE_KEY;

    /// Address of the directory index published by the official update server.
    pub const OFFICIAL_INDEX_URL: &'static str =
        "https://update.flipperzero.one/firmware/directory.json";

    /// Creates a loader that reads
    /// [`OFFICIAL_INDEX_URL`](UpdateChannelLoader::OFFICIAL_INDEX_URL).
    pub fn new(
        logger: Arc<Logger>,
        fetcher: Arc<FileFetcher>,
        download_dir: impl AsRef<Path>,
        channel: UpdateChannel,
    ) -> Self {
        Self {
            logger,
            fetcher,
            download_dir: download_dir.as_ref().to_path_buf(),
            channel,
            json_index_url: Self::OFFICIAL_INDEX_URL.to_string(),
            version_info: None,
        }
    }

    /// Reads the index from the given address instead of the official update server.
    #[must_use]
    pub fn with_index_url(mut self, json_index_url: impl Into<String>) -> Self {
        self.json_index_url = json_index_url.into();
        self
    }

    /// Channel the SDK is taken from.
    #[must_use]
    pub fn channel(&self) -> UpdateChannel {
        self.channel
    }

    /// Address the directory index is read from.
    #[must_use]
    pub fn json_index_url(&self) -> &str {
        &self.json_index_url
    }

    /// Version [`load`](SdkLoader::load) settled on.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError::LoaderNotInitialized`] when [`load`](SdkLoader::load) has not run
    /// yet.
    pub fn version_info(&self) -> Result<&IndexVersion, SdkError> {
        self.version_info
            .as_ref()
            .ok_or(SdkError::LoaderNotInitialized)
    }

    fn channel_repr(&self) -> String {
        format!("UpdateChannel.{}", self.channel.python_name())
    }

    async fn resolve_version(&self) -> Result<IndexVersion, SdkError> {
        self.logger.info(&format!(
            "Fetching version info for {} from {}",
            self.channel_repr(),
            self.json_index_url
        ));

        let body = self.fetcher.read_as_string(&self.json_index_url).await?;
        let index: DirectoryIndex =
            serde_json::from_str(&body).map_err(|error| SdkError::InvalidJson {
                message: error.to_string(),
            })?;

        if index.channels().is_empty() {
            return Err(SdkError::InvalidChannel {
                channel: self.channel_repr(),
            });
        }
        let channel =
            index
                .find_channel(self.channel.id())
                .ok_or_else(|| SdkError::InvalidChannel {
                    channel: self.channel_repr(),
                })?;
        let version = channel
            .versions()
            .first()
            .ok_or_else(|| SdkError::EmptyChannel {
                channel: self.channel_repr(),
            })?;

        self.logger
            .info(&format!("Using version: {}", version.version()));
        self.logger.debug(&format!(
            "Changelog: {}",
            version.changelog().unwrap_or("None")
        ));

        Ok(version.clone())
    }
}

impl SdkLoader for UpdateChannelLoader {
    fn mode_key(&self) -> &'static str {
        Self::MODE_KEY
    }

    fn load(&mut self) -> SdkFuture<'_, ()> {
        Box::pin(async move {
            self.version_info = Some(self.resolve_version().await?);
            Ok(())
        })
    }

    fn get_sdk_component<'a>(&'a self, target: &'a str) -> SdkFuture<'a, PathBuf> {
        Box::pin(async move {
            let version = self.version_info()?;
            if version.files().is_empty() {
                return Err(SdkError::EmptyFilesList);
            }

            let file = version.find_file(FileType::SdkZip, target).ok_or_else(|| {
                SdkError::InvalidFileType {
                    file_type: FileType::SdkZip.id().to_uppercase(),
                }
            })?;
            if file.url().is_empty() {
                return Err(SdkError::InvalidFileUrl);
            }

            Ok(self
                .fetcher
                .fetch_file(file.url(), &self.download_dir, "", false)
                .await?)
        })
    }

    fn metadata(&self) -> IndexMap<String, String> {
        IndexMap::from([
            (MODE_METADATA_KEY.to_string(), Self::MODE_KEY.to_string()),
            (
                CHANNEL_METADATA_KEY.to_string(),
                self.channel.key().to_string(),
            ),
            (
                JSON_INDEX_METADATA_KEY.to_string(),
                self.json_index_url.clone(),
            ),
            (
                VERSION_METADATA_KEY.to_string(),
                self.version_info
                    .as_ref()
                    .map_or_else(|| VERSION_UNKNOWN.to_string(), |v| v.version().to_string()),
            ),
        ])
    }
}
