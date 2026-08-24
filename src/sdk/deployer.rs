use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use serde_json::{Map, Value};
use tokio::sync::mpsc;

use super::loader::SdkLoader;
use super::{
    BranchLoader, DeployTask, LocalLoader, SdkError, UpdateChannel, UpdateChannelLoader, UrlLoader,
};
use crate::archive::{ExtractEvent, extract_archive};
use crate::log::{Logger, ProgressTask};
use crate::net::FileFetcher;
use crate::paths::Paths;
use crate::state::{ALWAYS_UPDATE_VERSIONS, State};

const HW_TARGET_KEY: &str = "hw_target";
const VERSION_KEY: &str = "version";

/// Deploys a Flipper Zero SDK into the uFBT home directory.
pub struct SdkDeployer {
    logger: Arc<Logger>,
    paths: Paths,
    fetcher: Arc<FileFetcher>,
}

impl SdkDeployer {
    /// Creates a deployer working in the given uFBT home directory.
    pub fn new(logger: Arc<Logger>, paths: Paths, fetcher: Arc<FileFetcher>) -> Self {
        Self {
            logger,
            paths,
            fetcher,
        }
    }

    /// Reconstructs the task that produced the currently deployed SDK.
    ///
    /// Returns [`None`] when no deployment state has been written yet, and also when the
    /// state file cannot be read or does not hold a JSON object — in either case there is no
    /// previous task to speak of.
    #[must_use]
    pub fn previous_task(&self) -> Option<DeployTask> {
        let state = State::read(self.paths.state_file()).ok().flatten()?;
        self.logger.debug(&format!(
            "get_previous_task() loaded state: {}",
            render_values(state.values())
        ));
        Some(DeployTask::from_state(&state))
    }

    /// Builds the loader that serves the mode of `task`.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError::InvalidMode`] when no loader implements the mode,
    /// [`SdkError::MissingParam`] when the task does not carry the parameter the mode needs,
    /// and [`SdkError::InvalidChannel`] when a channel task names a channel that does not
    /// exist.
    pub fn create_loader(&self, task: &DeployTask) -> Result<Box<dyn SdkLoader>, SdkError> {
        self.logger
            .debug(&format!("SdkLoaderFactory::create_for_task task={task}"));

        let download_dir = self.paths.download_dir();
        match task.mode().unwrap_or_default() {
            BranchLoader::MODE_KEY => {
                let branch = required_param(task, BranchLoader::MODE_KEY, "branch")?;
                let mut loader = BranchLoader::new(
                    self.logger.clone(),
                    self.fetcher.clone(),
                    download_dir,
                    branch,
                );
                if let Some(branch_root) = task.param("branch_root") {
                    loader = loader.with_branch_root(branch_root);
                }
                Ok(Box::new(loader))
            }
            UpdateChannelLoader::MODE_KEY => {
                let key = required_param(task, UpdateChannelLoader::MODE_KEY, "channel")?;
                let mut loader = UpdateChannelLoader::new(
                    self.logger.clone(),
                    self.fetcher.clone(),
                    download_dir,
                    UpdateChannel::by_key(key)?,
                );
                if let Some(json_index) = task.param("json_index") {
                    loader = loader.with_index_url(json_index);
                }
                Ok(Box::new(loader))
            }
            UrlLoader::MODE_KEY => {
                let url = required_param(task, UrlLoader::MODE_KEY, "url")?;
                Ok(Box::new(UrlLoader::new(
                    self.logger.clone(),
                    self.fetcher.clone(),
                    download_dir,
                    url,
                )))
            }
            LocalLoader::MODE_KEY => {
                let file_path = required_param(task, LocalLoader::MODE_KEY, "file_path")?;
                Ok(Box::new(LocalLoader::new(self.logger.clone(), file_path)))
            }
            _ => Err(SdkError::InvalidMode {
                mode: or_null(task.mode()).to_string(),
            }),
        }
    }

    /// Deploys the SDK the task describes.
    ///
    /// Returns `Ok(true)` when the SDK is in place — either freshly unpacked, or already
    /// up to date — and `Ok(false)` when the bundle could not be obtained, which is logged
    /// at [`LogLevel::Error`](crate::log::LogLevel::Error) rather than returned.
    ///
    /// An SDK is left alone when the task does not force a deployment, the SDK directory
    /// exists, and the recorded version and hardware target both match what the loader
    /// offers. A recorded version of `unknown` or `local` never matches.
    ///
    /// Unlike the reference implementation, an existing SDK directory whose deployment state
    /// is missing or unreadable is treated as an SDK of undeterminable version and simply
    /// replaced, instead of failing the deployment and requiring the directory to be removed
    /// by hand.
    ///
    /// # Errors
    ///
    /// Returns an error when the loader cannot be built or resolved, when the SDK directory
    /// cannot be replaced, when the bundle is not a readable zip archive, or when the
    /// deployment state cannot be written.
    pub async fn deploy(&self, task: &DeployTask) -> Result<bool, SdkError> {
        self.logger
            .info(&format!("Deploying SDK for {}", or_null(task.hw_target())));

        let mut loader = self.create_loader(task)?;
        loader.load().await?;

        let sdk_target_dir = self.paths.current_sdk_dir();
        self.logger
            .info(&format!("uFBT SDK dir: {}", sdk_target_dir.display()));

        if !task.force() && sdk_target_dir.exists() && self.is_up_to_date(task, loader.as_ref()) {
            return Ok(true);
        }

        let sdk_component = match loader
            .get_sdk_component(task.hw_target().unwrap_or_default())
            .await
        {
            Ok(component) => component,
            Err(error) => {
                self.logger.error(&format!(
                    "Failed to fetch SDK for {}: {error}",
                    or_null(task.hw_target())
                ));
                return Ok(false);
            }
        };

        if sdk_target_dir.exists() {
            fs::remove_dir_all(&sdk_target_dir)?;
        }

        let state = deployed_state(task, loader.as_ref());

        self.logger.info("Deploying SDK");
        self.extract_zip(&sdk_component, &sdk_target_dir).await?;

        state.write(self.paths.state_file())?;
        self.logger.info("SDK deployed.");
        Ok(true)
    }

    fn is_up_to_date(&self, task: &DeployTask, loader: &dyn SdkLoader) -> bool {
        let Some(state) = State::read(self.paths.state_file()).ok().flatten() else {
            self.logger
                .info("Cannot determine current SDK version, updating");
            return false;
        };

        let version = state.version();
        if version.is_some_and(|version| ALWAYS_UPDATE_VERSIONS.contains(&version)) {
            self.logger
                .info("Cannot determine current SDK version, updating");
            return false;
        }

        let metadata = loader.metadata();
        if version == metadata.get(VERSION_KEY).map(String::as_str)
            && state.hw_target() == task.hw_target()
        {
            self.logger.info("SDK is up-to-date");
            return true;
        }
        false
    }

    async fn extract_zip(&self, archive: &Path, target_dir: &Path) -> Result<(), SdkError> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let archive = archive.to_path_buf();
        let target_dir = target_dir.to_path_buf();
        let worker =
            tokio::task::spawn_blocking(move || extract_archive(&archive, &target_dir, &sender));

        let mut progress: Option<ProgressTask<'_>> = None;
        while let Some(event) = receiver.recv().await {
            match event {
                ExtractEvent::Total(total) => {
                    progress = Some(self.logger.progress("", Some(total)));
                }
                ExtractEvent::Advanced(done) => {
                    if let Some(progress) = progress.as_mut() {
                        progress.update(Some(done), None, None);
                    }
                }
            }
        }

        match worker.await.map_err(io::Error::other)? {
            Ok(()) => {
                if let Some(progress) = progress.as_mut() {
                    progress.finish();
                }
                Ok(())
            }
            Err(error) => {
                if let Some(progress) = progress.as_mut() {
                    progress.fail_with_message(&error.to_string());
                }
                Err(error.into())
            }
        }
    }
}

fn or_null(value: Option<&str>) -> &str {
    value.unwrap_or("null")
}

fn required_param<'a>(task: &'a DeployTask, mode: &str, param: &str) -> Result<&'a str, SdkError> {
    task.param(param).ok_or_else(|| SdkError::MissingParam {
        mode: mode.to_string(),
        param: param.to_string(),
    })
}

fn deployed_state(task: &DeployTask, loader: &dyn SdkLoader) -> State {
    let mut values = Map::new();
    values.insert(
        HW_TARGET_KEY.to_string(),
        task.hw_target()
            .map_or(Value::Null, |target| Value::String(target.to_string())),
    );
    for (key, value) in loader.metadata() {
        values.insert(key, Value::String(value));
    }
    State::new(values)
}

fn render_values(values: &Map<String, Value>) -> String {
    let mut rendered = String::from("{");
    for (index, (key, value)) in values.iter().enumerate() {
        if index > 0 {
            rendered.push_str(", ");
        }
        let _ = write!(rendered, "{key}: ");
        render_value(value, &mut rendered);
    }
    rendered.push('}');
    rendered
}

fn render_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => out.push_str(&number.to_string()),
        Value::String(text) => out.push_str(text),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                render_value(item, out);
            }
            out.push(']');
        }
        Value::Object(fields) => out.push_str(&render_values(fields)),
    }
}
