use std::fs;
use std::io;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use tokio::sync::mpsc::UnboundedSender;
use zip::ZipArchive;

pub(crate) enum ExtractEvent {
    Total(u64),
    Advanced(u64),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ExtractError {
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub(crate) fn extract_archive(
    archive: &Path,
    target_dir: &Path,
    sender: &UnboundedSender<ExtractEvent>,
) -> Result<(), ExtractError> {
    fs::create_dir_all(target_dir)?;

    let mut zip = ZipArchive::new(fs::File::open(archive)?)?;
    let total = zip.len();
    let _ = sender.send(ExtractEvent::Total(total as u64));

    for index in 0..total {
        let mut entry = zip.by_index(index)?;
        if let Some(path) = safe_entry_path(target_dir, entry.name()) {
            if entry.is_dir() {
                fs::create_dir_all(&path)?;
            } else {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                io::copy(&mut entry, &mut fs::File::create(&path)?)?;
            }
        }
        let _ = sender.send(ExtractEvent::Advanced(index as u64 + 1));
    }
    Ok(())
}

fn safe_entry_path(target_dir: &Path, name: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = name.split(['/', '\\']).collect();
    if parts.iter().any(|part| *part == ".." || *part == ".") {
        return None;
    }
    let clean: Vec<&str> = parts.into_iter().filter(|part| !part.is_empty()).collect();
    if clean.is_empty() {
        return None;
    }
    let mut path = target_dir.to_string_lossy().into_owned();
    for part in clean {
        path.push(MAIN_SEPARATOR);
        path.push_str(part);
    }
    Some(PathBuf::from(path))
}
