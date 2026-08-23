use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

use super::FileType;

fn object(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

fn string_field(fields: &Map<String, Value>, key: &str) -> String {
    fields
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn optional_string_field(fields: &Map<String, Value>, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn optional_int_field(fields: &Map<String, Value>, key: &str) -> Option<i64> {
    fields.get(key).and_then(Value::as_i64)
}

fn array_field<T>(
    fields: &Map<String, Value>,
    key: &str,
    from_fields: fn(&Map<String, Value>) -> T,
) -> Vec<T> {
    fields
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(object).map(from_fields).collect())
        .unwrap_or_default()
}

/// One downloadable file of a firmware version listed in the directory index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexFile {
    url: String,
    target: String,
    file_type: String,
    sha256: Option<String>,
}

impl IndexFile {
    /// Address the file is downloaded from, or an empty string when the index carries none.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Hardware target the file was built for, such as `f7`.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Kind of the file, matching [`FileType::id`] for the types this crate knows.
    #[must_use]
    pub fn file_type(&self) -> &str {
        &self.file_type
    }

    /// SHA-256 checksum published alongside the file, if any.
    #[must_use]
    pub fn sha256(&self) -> Option<&str> {
        self.sha256.as_deref()
    }

    fn from_fields(fields: &Map<String, Value>) -> Self {
        Self {
            url: string_field(fields, "url"),
            target: string_field(fields, "target"),
            file_type: string_field(fields, "type"),
            sha256: optional_string_field(fields, "sha256"),
        }
    }
}

/// One firmware version of an update channel, together with its downloadable files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexVersion {
    version: String,
    changelog: Option<String>,
    timestamp: Option<i64>,
    files: Vec<IndexFile>,
}

impl IndexVersion {
    /// Version string, or an empty string when the index carries none.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Changelog published for the version, if any.
    #[must_use]
    pub fn changelog(&self) -> Option<&str> {
        self.changelog.as_deref()
    }

    /// Build timestamp published for the version, if any.
    #[must_use]
    pub fn timestamp(&self) -> Option<i64> {
        self.timestamp
    }

    /// Files published for the version.
    #[must_use]
    pub fn files(&self) -> &[IndexFile] {
        &self.files
    }

    /// Finds the first file of the given type built for the given hardware target.
    #[must_use]
    pub fn find_file(&self, file_type: FileType, target: &str) -> Option<&IndexFile> {
        self.files
            .iter()
            .find(|file| file.file_type == file_type.id() && file.target == target)
    }

    fn from_fields(fields: &Map<String, Value>) -> Self {
        Self {
            version: string_field(fields, "version"),
            changelog: optional_string_field(fields, "changelog"),
            timestamp: optional_int_field(fields, "timestamp"),
            files: array_field(fields, "files", IndexFile::from_fields),
        }
    }
}

/// One update channel of the directory index, holding its versions newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexChannel {
    id: String,
    title: Option<String>,
    description: Option<String>,
    versions: Vec<IndexVersion>,
}

impl IndexChannel {
    /// Identifier of the channel, matching [`UpdateChannel::id`](super::UpdateChannel::id).
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Human-readable title of the channel, if any.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Human-readable description of the channel, if any.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Versions published in the channel, in the order the index lists them.
    #[must_use]
    pub fn versions(&self) -> &[IndexVersion] {
        &self.versions
    }

    fn from_fields(fields: &Map<String, Value>) -> Self {
        Self {
            id: string_field(fields, "id"),
            title: optional_string_field(fields, "title"),
            description: optional_string_field(fields, "description"),
            versions: array_field(fields, "versions", IndexVersion::from_fields),
        }
    }
}

/// Contents of the `directory.json` index published by the update server.
///
/// Parsing never fails on unexpected shapes: a field of the wrong type, or a missing field,
/// reads as absent, and an array element that is not an object is skipped. Whatever the
/// index turns out to hold, the result is a well-formed value that the loader inspects.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectoryIndex {
    channels: Vec<IndexChannel>,
}

impl DirectoryIndex {
    /// Channels published in the index.
    #[must_use]
    pub fn channels(&self) -> &[IndexChannel] {
        &self.channels
    }

    /// Finds the channel carrying the given identifier.
    #[must_use]
    pub fn find_channel(&self, id: &str) -> Option<&IndexChannel> {
        self.channels.iter().find(|channel| channel.id == id)
    }

    fn from_fields(fields: &Map<String, Value>) -> Self {
        Self {
            channels: array_field(fields, "channels", IndexChannel::from_fields),
        }
    }
}

macro_rules! deserialize_from_fields {
    ($type:ty) => {
        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = Value::deserialize(deserializer)?;
                Ok(object(&value).map(Self::from_fields).unwrap_or_default())
            }
        }
    };
}

deserialize_from_fields!(IndexFile);
deserialize_from_fields!(IndexVersion);
deserialize_from_fields!(IndexChannel);
deserialize_from_fields!(DirectoryIndex);
