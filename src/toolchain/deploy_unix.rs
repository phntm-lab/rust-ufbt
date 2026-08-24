use std::env;
use std::fs;
use std::os::unix::fs::symlink;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncBufReadExt as _, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{self, UnboundedSender};

use super::{ToolchainDeployer, ToolchainError, ToolchainInfo};
use crate::paths::join;

const PRESERVE_TAR_VAR: &str = "FBT_PRESERVE_TAR";

impl ToolchainDeployer {
    pub(super) async fn deploy_platform(
        &self,
        info: &ToolchainInfo,
    ) -> Result<bool, ToolchainError> {
        if !self.check_tar().await {
            return Ok(false);
        }

        let archive_name = archive_name(info.url());
        let version_suffix = format!("-{}.tar.gz", info.version());
        let dist_dir_name = archive_name.replace(version_suffix.as_str(), "");
        let archive_dir = self.archive_dir();
        let archive_file = join(&archive_dir, &archive_name);

        self.logger
            .raw_no_newline("Checking if downloaded toolchain tgz exists..");
        if archive_file.is_file() {
            self.logger.raw("yes");
        } else {
            self.logger.raw("no");
            self.logger.raw("Downloading toolchain:");
            if self
                .fetcher
                .fetch_file(info.url(), &archive_dir, "", true)
                .await
                .is_err()
            {
                self.logger
                    .raw(&format!("Failed to download {}", info.url()));
                return Ok(false);
            }
            self.logger.raw("done");
        }

        let arch_dir = info.arch_dir();
        self.logger.raw_no_newline("Removing old toolchain..");
        if arch_dir.is_dir() {
            fs::remove_dir_all(arch_dir)?;
        }
        self.logger.raw("done");

        let toolchain_dir = self.paths.toolchain_dir();
        self.logger.raw(&format!(
            "Unpacking toolchain to '{}':",
            toolchain_dir.display()
        ));

        let current_link = self.paths.toolchain_current_link();
        if current_link.symlink_metadata().is_ok() {
            fs::remove_file(&current_link)?;
        }

        fs::create_dir_all(&toolchain_dir)?;
        if !self.unpack_tar(&archive_file).await? {
            return Ok(false);
        }

        let dist_dir = join(&toolchain_dir, &dist_dir_name);
        if !dist_dir.is_dir() {
            return Ok(false);
        }
        fs::rename(&dist_dir, arch_dir)?;

        self.logger
            .raw_no_newline("linking toolchain to 'current'..");
        symlink(arch_dir, &current_link)?;
        self.logger.raw("done");

        self.cleanup()?;
        Ok(true)
    }

    fn archive_dir(&self) -> PathBuf {
        self.paths.toolchain_dir()
    }

    async fn check_tar(&self) -> bool {
        self.logger.raw_no_newline("Checking for tar..");
        let ran = Command::new("tar")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        if ran.is_ok_and(|status| status.success()) {
            self.logger.raw("yes");
            true
        } else {
            self.logger.raw("no");
            false
        }
    }

    async fn unpack_tar(&self, archive: &Path) -> Result<bool, ToolchainError> {
        let mut task = self.logger.progress("", None);

        let mut child = Command::new("tar")
            .arg("-xvf")
            .arg(archive)
            .arg("-C")
            .arg(self.paths.toolchain_dir())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| ToolchainError::Process {
                program: "tar".to_owned(),
                source,
            })?;

        let (sender, mut receiver) = mpsc::unbounded_channel();
        count_lines(child.stdout.take(), sender.clone());
        count_lines(child.stderr.take(), sender);

        while receiver.recv().await.is_some() {
            task.advance(1);
        }

        let status = child.wait().await?;
        if !status.success() {
            task.fail_with_message(&format!("tar exited with {}", exit_code(&status)));
            return Ok(false);
        }
        task.finish();
        Ok(true)
    }

    fn cleanup(&self) -> Result<(), ToolchainError> {
        self.logger.raw_no_newline("Cleaning up..");
        let preserve = env::var(PRESERVE_TAR_VAR).unwrap_or_default();
        let toolchain_dir = self.paths.toolchain_dir();
        if toolchain_dir.is_dir() {
            for entry in fs::read_dir(&toolchain_dir)? {
                let path = entry?.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name() else {
                    continue;
                };
                let name = name.to_string_lossy();
                if name.ends_with(".part") || (preserve.is_empty() && name.ends_with(".tar.gz")) {
                    fs::remove_file(&path)?;
                }
            }
        }
        self.logger.raw("done");
        Ok(())
    }
}

fn archive_name(url: &str) -> String {
    url.rsplit('/').next().unwrap_or_default().to_owned()
}

fn exit_code(status: &ExitStatus) -> i32 {
    status
        .code()
        .or_else(|| status.signal().map(|signal| -signal))
        .unwrap_or(-1)
}

fn count_lines<R>(stream: Option<R>, sender: UnboundedSender<()>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let Some(stream) = stream else {
        return;
    };
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(_)) = lines.next_line().await {
            if sender.send(()).is_err() {
                break;
            }
        }
    });
}
