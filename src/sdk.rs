//! Flipper Zero SDK channel resolution and deployment.

mod deploy_task;
mod directory_index;
mod error;
mod file_type;
mod loader;

pub use deploy_task::{BundleTaskBuilder, DeployTask, DeployTaskBuilder};
pub use directory_index::{DirectoryIndex, IndexChannel, IndexFile, IndexVersion};
pub use error::SdkError;
pub use file_type::{FileType, UpdateChannel};
pub use loader::{LocalLoader, SdkFuture, SdkLoader, UrlLoader, VERSION_UNKNOWN};
