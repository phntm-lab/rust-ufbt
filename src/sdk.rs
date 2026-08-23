//! Flipper Zero SDK channel resolution and deployment.

mod directory_index;
mod error;
mod file_type;

pub use directory_index::{DirectoryIndex, IndexChannel, IndexFile, IndexVersion};
pub use error::SdkError;
pub use file_type::{FileType, UpdateChannel};
