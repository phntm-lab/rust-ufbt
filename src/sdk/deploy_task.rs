use std::fmt;
use std::path::Path;

use indexmap::IndexMap;
use serde_json::Value;

use super::UpdateChannel;
use crate::state::State;

pub(crate) const BRANCH_MODE_KEY: &str = "branch";
pub(crate) const CHANNEL_MODE_KEY: &str = "channel";
pub(crate) const URL_MODE_KEY: &str = "url";
pub(crate) const LOCAL_MODE_KEY: &str = "local";

const BRANCH_PARAM: &str = "branch";
const BRANCH_ROOT_PARAM: &str = "branch_root";
const CHANNEL_PARAM: &str = "channel";
const JSON_INDEX_PARAM: &str = "json_index";
const URL_PARAM: &str = "url";
const FILE_PATH_PARAM: &str = "file_path";

/// Description of an SDK deployment: where the SDK comes from and which hardware target it
/// is built for.
///
/// A task is built through one of the constructors — [`defaults`](DeployTask::defaults),
/// [`channel`](DeployTask::channel), [`branch`](DeployTask::branch),
/// [`url`](DeployTask::url), [`local`](DeployTask::local) or
/// [`from_state`](DeployTask::from_state) — each of which fills in
/// [`mode`](DeployTask::mode) and the [`params`](DeployTask::params) the matching loader
/// reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeployTask {
    hw_target: Option<String>,
    force: bool,
    mode: Option<String>,
    params: IndexMap<String, String>,
}

impl DeployTask {
    /// Hardware target assumed when none is given.
    pub const DEFAULT_HW_TARGET: &'static str = "f7";

    /// A task that deploys the latest release for the default hardware target.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            hw_target: Some(Self::DEFAULT_HW_TARGET.to_string()),
            force: false,
            mode: Some(CHANNEL_MODE_KEY.to_string()),
            params: IndexMap::from([(
                CHANNEL_PARAM.to_string(),
                UpdateChannel::Release.key().to_string(),
            )]),
        }
    }

    /// Starts a task that deploys the newest version of an update channel.
    ///
    /// The hardware target defaults to [`DEFAULT_HW_TARGET`](DeployTask::DEFAULT_HW_TARGET)
    /// and the index is read from the official update server unless
    /// [`index_url`](DeployTaskBuilder::index_url) says otherwise.
    #[must_use]
    pub fn channel(channel: UpdateChannel) -> DeployTaskBuilder {
        DeployTaskBuilder::new(
            CHANNEL_MODE_KEY,
            CHANNEL_PARAM,
            channel.key(),
            JSON_INDEX_PARAM,
        )
    }

    /// Starts a task that deploys the newest build of a firmware branch.
    ///
    /// The hardware target defaults to [`DEFAULT_HW_TARGET`](DeployTask::DEFAULT_HW_TARGET)
    /// and the branch listing is read from the official update server unless
    /// [`index_url`](DeployTaskBuilder::index_url) says otherwise.
    #[must_use]
    pub fn branch(branch: impl Into<String>) -> DeployTaskBuilder {
        DeployTaskBuilder::new(
            BRANCH_MODE_KEY,
            BRANCH_PARAM,
            branch.into(),
            BRANCH_ROOT_PARAM,
        )
    }

    /// Starts a task that deploys an SDK bundle downloaded from an explicit address.
    #[must_use]
    pub fn url(url: impl Into<String>, hw_target: impl Into<String>) -> BundleTaskBuilder {
        BundleTaskBuilder::new(URL_MODE_KEY, URL_PARAM, url.into(), hw_target)
    }

    /// Starts a task that deploys an SDK bundle already present on this machine.
    #[must_use]
    pub fn local(path: impl AsRef<Path>, hw_target: impl Into<String>) -> BundleTaskBuilder {
        BundleTaskBuilder::new(
            LOCAL_MODE_KEY,
            FILE_PATH_PARAM,
            path.as_ref().to_string_lossy().into_owned(),
            hw_target,
        )
    }

    /// Reconstructs the task that produced a persisted deployment state.
    ///
    /// Every string value of the state becomes a parameter, including `hw_target` and
    /// `mode`, which are therefore both read into their own fields and repeated in
    /// [`params`](DeployTask::params). Values that are not strings are left out, and
    /// [`force`](DeployTask::force) is never recorded in the state, so it reads as `false`.
    #[must_use]
    pub fn from_state(state: &State) -> Self {
        let params = state
            .values()
            .iter()
            .filter_map(|(key, value)| match value {
                Value::String(value) => Some((key.clone(), value.clone())),
                _ => None,
            })
            .collect();
        Self {
            hw_target: state.hw_target().map(ToString::to_string),
            force: false,
            mode: state.mode().map(ToString::to_string),
            params,
        }
    }

    /// Hardware target the SDK is deployed for.
    #[must_use]
    pub fn hw_target(&self) -> Option<&str> {
        self.hw_target.as_deref()
    }

    /// Whether the SDK is redeployed even when the deployed version already matches.
    #[must_use]
    pub fn force(&self) -> bool {
        self.force
    }

    /// Key of the loader that resolves the SDK bundle for this task.
    #[must_use]
    pub fn mode(&self) -> Option<&str> {
        self.mode.as_deref()
    }

    /// Parameters the loader reads, in the order they were set.
    #[must_use]
    pub fn params(&self) -> &IndexMap<String, String> {
        &self.params
    }

    /// Value of a single parameter.
    #[must_use]
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }

    /// Overlays another task onto this one.
    ///
    /// The hardware target and the mode are taken over only when the other task carries
    /// them, and parameters are merged with empty values ignored — so an incomplete request
    /// refines a task read back from the deployment state instead of erasing it.
    /// [`force`](DeployTask::force) is the exception: it is always taken over, because a
    /// request that does not ask for a forced deployment means the deployment is not forced.
    pub fn update_from(&mut self, other: &Self) {
        if other.hw_target.is_some() {
            self.hw_target = other.hw_target.clone();
        }
        if other.mode.is_some() {
            self.mode = other.mode.clone();
        }
        self.force = other.force;
        for (key, value) in &other.params {
            if !value.is_empty() {
                self.params.insert(key.clone(), value.clone());
            }
        }
    }
}

fn write_optional(f: &mut fmt::Formatter<'_>, value: Option<&str>) -> fmt::Result {
    match value {
        Some(value) => f.write_str(value),
        None => f.write_str("null"),
    }
}

impl fmt::Display for DeployTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SdkDeployTask(hw_target: ")?;
        write_optional(f, self.hw_target())?;
        f.write_str(", mode: ")?;
        write_optional(f, self.mode())?;
        write!(f, ", force: {}, params: {{", self.force)?;
        for (index, (key, value)) in self.params.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{key}: {value}")?;
        }
        f.write_str("})")
    }
}

/// Builder of a [`DeployTask`] that resolves the SDK through an index on the update server.
///
/// Returned by [`DeployTask::channel`] and [`DeployTask::branch`].
#[derive(Debug, Clone)]
pub struct DeployTaskBuilder {
    task: DeployTask,
    index_url_param: &'static str,
}

impl DeployTaskBuilder {
    fn new(
        mode: &'static str,
        param: &'static str,
        value: impl Into<String>,
        index_url_param: &'static str,
    ) -> Self {
        Self {
            task: DeployTask {
                hw_target: Some(DeployTask::DEFAULT_HW_TARGET.to_string()),
                force: false,
                mode: Some(mode.to_string()),
                params: IndexMap::from([(param.to_string(), value.into())]),
            },
            index_url_param,
        }
    }

    /// Sets the hardware target, replacing
    /// [`DeployTask::DEFAULT_HW_TARGET`](DeployTask::DEFAULT_HW_TARGET).
    #[must_use]
    pub fn hw_target(mut self, hw_target: impl Into<String>) -> Self {
        self.task.hw_target = Some(hw_target.into());
        self
    }

    /// Reads the index from the given address instead of the official update server.
    #[must_use]
    pub fn index_url(mut self, index_url: impl Into<String>) -> Self {
        self.task
            .params
            .insert(self.index_url_param.to_string(), index_url.into());
        self
    }

    /// Redeploys the SDK even when the deployed version already matches.
    #[must_use]
    pub fn force(mut self, force: bool) -> Self {
        self.task.force = force;
        self
    }

    /// Finishes the task.
    #[must_use]
    pub fn build(self) -> DeployTask {
        self.task
    }
}

/// Builder of a [`DeployTask`] that names the SDK bundle directly.
///
/// Returned by [`DeployTask::url`] and [`DeployTask::local`], both of which already take the
/// hardware target, so only [`force`](BundleTaskBuilder::force) is left to set.
#[derive(Debug, Clone)]
pub struct BundleTaskBuilder {
    task: DeployTask,
}

impl BundleTaskBuilder {
    fn new(
        mode: &'static str,
        param: &'static str,
        value: String,
        hw_target: impl Into<String>,
    ) -> Self {
        Self {
            task: DeployTask {
                hw_target: Some(hw_target.into()),
                force: false,
                mode: Some(mode.to_string()),
                params: IndexMap::from([(param.to_string(), value)]),
            },
        }
    }

    /// Redeploys the SDK even when the deployed version already matches.
    #[must_use]
    pub fn force(mut self, force: bool) -> Self {
        self.task.force = force;
        self
    }

    /// Finishes the task.
    #[must_use]
    pub fn build(self) -> DeployTask {
        self.task
    }
}
