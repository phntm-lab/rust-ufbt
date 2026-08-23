use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use indexmap::IndexMap;
use regex::Regex;

use super::deploy_task::BRANCH_MODE_KEY;
use super::loader::{
    MODE_METADATA_KEY, SdkFuture, SdkLoader, VERSION_METADATA_KEY, VERSION_UNKNOWN,
};
use super::{FileType, SdkError};
use crate::log::Logger;
use crate::net::FileFetcher;

const BRANCH_METADATA_KEY: &str = "branch";
const BRANCH_ROOT_METADATA_KEY: &str = "branch_root";

static HREF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)<a[^>]+href=["']([^"']+)["']"#).unwrap());

static FILE_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^flipper-z-([0-9A-Za-z_]+)-([0-9A-Za-z_]+)-(.+)\.([0-9A-Za-z_]+)").unwrap()
});

/// Loads an SDK bundle from the directory listing of a firmware branch.
///
/// [`load`](SdkLoader::load) reads the listing served at
/// [`branch_url`](BranchLoader::branch_url) and recognises the published bundles by their
/// file names. All of them are expected to belong to one version; a file name announcing a
/// different one makes the load fail with [`SdkError::MultipleVersions`].
pub struct BranchLoader {
    logger: Arc<Logger>,
    fetcher: Arc<FileFetcher>,
    download_dir: PathBuf,
    branch: String,
    branch_root: String,
    branch_files: HashMap<String, String>,
    version: Option<String>,
}

impl BranchLoader {
    /// Key of the deploy task mode this loader serves.
    pub const MODE_KEY: &'static str = BRANCH_MODE_KEY;

    /// Address the branch listings of the official update server are served under.
    pub const UPDATE_SERVER_BRANCH_ROOT: &'static str =
        "https://update.flipperzero.one/builds/firmware";

    /// Creates a loader that reads the branch listings of the official update server.
    pub fn new(
        logger: Arc<Logger>,
        fetcher: Arc<FileFetcher>,
        download_dir: impl AsRef<Path>,
        branch: impl Into<String>,
    ) -> Self {
        Self {
            logger,
            fetcher,
            download_dir: download_dir.as_ref().to_path_buf(),
            branch: branch.into(),
            branch_root: Self::UPDATE_SERVER_BRANCH_ROOT.to_string(),
            branch_files: HashMap::new(),
            version: None,
        }
    }

    /// Reads the branch listings from the given address instead of the official update
    /// server.
    #[must_use]
    pub fn with_branch_root(mut self, branch_root: impl Into<String>) -> Self {
        self.branch_root = branch_root.into();
        self
    }

    /// Branch the SDK is taken from.
    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Address the branch listings are served under.
    #[must_use]
    pub fn branch_root(&self) -> &str {
        &self.branch_root
    }

    /// Address of this branch's listing, which is also the base every published file name is
    /// resolved against.
    #[must_use]
    pub fn branch_url(&self) -> String {
        format!("{}/{}/", self.branch_root, self.branch)
    }

    /// Version [`load`](SdkLoader::load) found in the listing.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    fn record(&mut self, href: &str) -> Result<(), SdkError> {
        if href.contains(".map") {
            return Ok(());
        }
        let Some(captures) = FILE_NAME_RE.captures(href) else {
            return Ok(());
        };
        let target = &captures[1];
        let file_type = &captures[2];
        let version = &captures[3];
        let extension = &captures[4];

        let type_id = format!("{file_type}_{extension}").to_lowercase();
        if let Some(known) = FileType::by_id(&type_id) {
            self.branch_files
                .insert(format!("{}|{target}", known.id()), href.to_string());
        }

        match &self.version {
            None => self.version = Some(version.to_string()),
            Some(first) if !version.starts_with(first.as_str()) => {
                return Err(SdkError::MultipleVersions {
                    first: first.clone(),
                    second: version.to_string(),
                });
            }
            Some(_) => {}
        }
        Ok(())
    }
}

impl SdkLoader for BranchLoader {
    fn mode_key(&self) -> &'static str {
        Self::MODE_KEY
    }

    fn load(&mut self) -> SdkFuture<'_, ()> {
        Box::pin(async move {
            let branch_url = self.branch_url();
            self.logger
                .info(&format!("Fetching branch index {branch_url}"));
            let html = self.fetcher.read_as_string(&branch_url).await?;

            let hrefs: Vec<String> = HREF_RE
                .captures_iter(&html)
                .map(|captures| captures[1].to_string())
                .collect();
            for href in hrefs {
                self.record(&href)?;
            }

            self.logger.info(&format!(
                "Found version {}",
                self.version.as_deref().unwrap_or("null")
            ));
            Ok(())
        })
    }

    fn get_sdk_component<'a>(&'a self, target: &'a str) -> SdkFuture<'a, PathBuf> {
        Box::pin(async move {
            let key = format!("{}|{target}", FileType::SdkZip.id());
            let file_name =
                self.branch_files
                    .get(&key)
                    .ok_or_else(|| SdkError::SdkBundleNotFound {
                        target: target.to_string(),
                    })?;

            let url = format!("{}{file_name}", self.branch_url());
            Ok(self
                .fetcher
                .fetch_file(&url, &self.download_dir, "", false)
                .await?)
        })
    }

    fn metadata(&self) -> IndexMap<String, String> {
        IndexMap::from([
            (MODE_METADATA_KEY.to_string(), Self::MODE_KEY.to_string()),
            (BRANCH_METADATA_KEY.to_string(), self.branch.clone()),
            (
                VERSION_METADATA_KEY.to_string(),
                self.version
                    .clone()
                    .unwrap_or_else(|| VERSION_UNKNOWN.to_string()),
            ),
            (
                BRANCH_ROOT_METADATA_KEY.to_string(),
                self.branch_root.clone(),
            ),
        ])
    }
}
