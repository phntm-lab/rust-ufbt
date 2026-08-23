use std::io;

use crate::net::FetchError;

/// Failure of SDK channel resolution or SDK deployment.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SdkError {
    /// A deploy task named a loader mode that no loader implements.
    #[error("Invalid mode: {mode}")]
    InvalidMode {
        /// Mode key taken from the deploy task.
        mode: String,
    },
    /// A branch listing offered more than one SDK version.
    #[error("Found multiple versions: {first} and {second}")]
    MultipleVersions {
        /// Version discovered first.
        first: String,
        /// Version that contradicts it.
        second: String,
    },
    /// A branch listing has no SDK bundle for the requested hardware target.
    #[error("SDK bundle not found for {target}")]
    SdkBundleNotFound {
        /// Requested hardware target.
        target: String,
    },
    /// Version information was requested before the loader had been loaded.
    #[error("Loader is not initialized")]
    LoaderNotInitialized,
    /// The directory index is not valid JSON.
    #[error("Invalid JSON: {message}")]
    InvalidJson {
        /// Message reported by the JSON parser.
        message: String,
    },
    /// The requested update channel is not present in the directory index.
    #[error("Invalid channel: {channel}")]
    InvalidChannel {
        /// Channel key as requested by the caller.
        channel: String,
    },
    /// The requested update channel carries no versions.
    #[error("Empty channel: {channel}")]
    EmptyChannel {
        /// Channel key as requested by the caller.
        channel: String,
    },
    /// The selected version carries no files.
    #[error("Empty files list")]
    EmptyFilesList,
    /// The selected version has no file of the requested type.
    #[error("Invalid file type: FileType.{file_type}")]
    InvalidFileType {
        /// Upper-case identifier of the file type.
        file_type: String,
    },
    /// The selected file has an empty download URL.
    #[error("Invalid file url")]
    InvalidFileUrl,
    /// A deploy task named a loader mode without the parameter that mode needs.
    #[error("Missing {param} parameter for mode {mode}")]
    MissingParam {
        /// Mode key taken from the deploy task.
        mode: String,
        /// Name of the parameter the mode requires.
        param: String,
    },
    /// The SDK bundle or the directory index could not be downloaded.
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// The SDK bundle could not be unpacked.
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    /// A file system operation failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}
