use std::fs;
use std::io;
use std::os::windows::fs::{FileTypeExt as _, symlink_dir};
use std::path::{Path, PathBuf};

use tokio::sync::mpsc;

use super::{ToolchainDeployer, ToolchainError, ToolchainInfo};
use crate::archive::extract_archive;
use crate::paths::join;

impl ToolchainDeployer {
    pub(super) async fn deploy_platform(
        &self,
        info: &ToolchainInfo,
    ) -> Result<bool, ToolchainError> {
        let archive_name = archive_name(info.url());
        let version_suffix = format!("-{}.zip", info.version());
        let dist_dir_name = archive_name.replace(version_suffix.as_str(), "");
        let archive_dir = self.archive_dir();
        let archive_file = join(&archive_dir, &archive_name);
        let arch_dir = info.arch_dir();
        let current_link = self.paths.toolchain_current_link();

        if arch_dir.is_dir() {
            self.logger
                .raw_no_newline("Removing old Windows toolchain..");
            fs::remove_dir_all(arch_dir)?;
            self.logger.raw("done!");
        }

        if current_link.symlink_metadata().is_ok() {
            self.logger.raw_no_newline("Unlinking 'current'..");
            remove_link(&current_link)?;
            self.logger.raw("done!");
        }

        if !archive_file.is_file() {
            self.logger
                .raw_no_newline("Downloading Windows toolchain..");
            if let Err(error) = self
                .fetcher
                .fetch_file(info.url(), &archive_dir, "", false)
                .await
            {
                self.logger.raw("An error occurred");
                self.logger.raw(&error.to_string());
                return Ok(false);
            }
            self.logger.raw("done!");
        }

        fs::create_dir_all(self.paths.toolchain_dir())?;

        let dist_dir = join(&archive_dir, &dist_dir_name);
        if dist_dir.is_dir() {
            self.logger.raw("Cleaning up temp toolchain path..");
            fs::remove_dir_all(&dist_dir)?;
        }

        self.logger.raw_no_newline("Extracting Windows toolchain..");
        if !self.unpack_zip(&archive_file).await? {
            return Ok(false);
        }

        self.logger.raw_no_newline("moving..");
        if !dist_dir.is_dir() {
            return Ok(false);
        }
        fs::rename(&dist_dir, arch_dir)?;

        self.logger.raw_no_newline("linking to 'current'..");
        symlink_dir(arch_dir, &current_link)?;
        self.logger.raw("done!");

        self.logger.raw_no_newline("Cleaning up temporary files..");
        if archive_file.is_file() {
            fs::remove_file(&archive_file)?;
        }
        self.logger.raw("done!");
        Ok(true)
    }

    fn archive_dir(&self) -> PathBuf {
        self.paths.current_sdk_dir()
    }

    async fn unpack_zip(&self, archive: &Path) -> Result<bool, ToolchainError> {
        let mut task = self.logger.progress("", None);

        let (sender, mut receiver) = mpsc::unbounded_channel();
        let source = archive.to_path_buf();
        let target_dir = self.archive_dir();
        let worker =
            tokio::task::spawn_blocking(move || extract_archive(&source, &target_dir, &sender));
        while receiver.recv().await.is_some() {}

        match worker.await.map_err(io::Error::other)? {
            Ok(()) => {
                task.finish();
                Ok(true)
            }
            Err(error) => {
                task.fail_with_message(&error.to_string());
                Ok(false)
            }
        }
    }
}

fn archive_name(url: &str) -> String {
    url.rsplit('/').next().unwrap_or_default().to_owned()
}

fn remove_link(link: &Path) -> io::Result<()> {
    if link.symlink_metadata()?.file_type().is_symlink_dir() {
        fs::remove_dir(link)
    } else {
        fs::remove_file(link)
    }
}
