//! Persisted SDK deployment state.

use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde_json::ser::PrettyFormatter;
use serde_json::{Map, Value};

/// Versions that carry no usable version information, so a deployment holding one of them
/// is always replaced rather than compared.
pub const ALWAYS_UPDATE_VERSIONS: [&str; 2] = ["unknown", "local"];

const INDENT: &[u8] = b"    ";

/// Contents of the `ufbt_state.json` file written next to a deployed SDK.
///
/// The state is a free-form JSON object: the well-known keys are exposed as getters, and
/// everything the SDK loader recorded alongside them is preserved as written. Key order is
/// kept as well, so reading a state and writing it back reproduces the original file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    values: Map<String, Value>,
}

impl State {
    /// Wraps a set of state values.
    #[must_use]
    pub fn new(values: Map<String, Value>) -> Self {
        Self { values }
    }

    /// Reads the state from the given file.
    ///
    /// Returns `Ok(None)` when the file does not exist, or when it holds valid JSON that is
    /// not an object. A file that cannot be read, or that is not valid JSON at all, is an
    /// error.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, or if its contents are not valid JSON.
    pub fn read(path: impl AsRef<Path>) -> io::Result<Option<Self>> {
        let path = path.as_ref();
        if !path.is_file() {
            return Ok(None);
        }
        let contents = fs::read_to_string(path)?;
        let decoded: Value = serde_json::from_str(&contents).map_err(io::Error::other)?;
        Ok(match decoded {
            Value::Object(values) => Some(Self { values }),
            _ => None,
        })
    }

    /// Writes the state to the given file, creating the parent directory if it is missing.
    ///
    /// The file is indented with four spaces, keys are written in the order they were
    /// inserted, and no trailing line break is added.
    ///
    /// # Errors
    ///
    /// Returns an error if the parent directory cannot be created or the file cannot be
    /// written.
    pub fn write(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => fs::create_dir_all(parent)?,
            _ => {}
        }
        fs::write(path, self.to_json())
    }

    /// Renders the state exactly as [`write`](Self::write) would store it.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::with_formatter(
            &mut buffer,
            PrettyFormatter::with_indent(INDENT),
        );
        self.values
            .serialize(&mut serializer)
            .expect("serializing a JSON object into memory cannot fail");
        String::from_utf8(buffer).expect("serde_json emits valid UTF-8")
    }

    /// All state values, in the order they were inserted.
    #[must_use]
    pub fn values(&self) -> &Map<String, Value> {
        &self.values
    }

    /// Hardware target the deployed SDK was built for.
    #[must_use]
    pub fn hw_target(&self) -> Option<&str> {
        self.string("hw_target")
    }

    /// Loader mode the SDK was deployed with.
    #[must_use]
    pub fn mode(&self) -> Option<&str> {
        self.string("mode")
    }

    /// Version of the deployed SDK.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.string("version")
    }

    /// URL of the directory index the SDK was resolved through.
    #[must_use]
    pub fn json_index(&self) -> Option<&str> {
        self.string("json_index")
    }

    /// Update channel the SDK was taken from.
    #[must_use]
    pub fn channel(&self) -> Option<&str> {
        self.string("channel")
    }

    /// Whether the recorded version says nothing about what is actually deployed, which is
    /// the case when it is missing or one of [`ALWAYS_UPDATE_VERSIONS`].
    #[must_use]
    pub fn is_version_unknown(&self) -> bool {
        match self.version() {
            None => true,
            Some(version) => ALWAYS_UPDATE_VERSIONS.contains(&version),
        }
    }

    fn string(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(Value::as_str)
    }
}
