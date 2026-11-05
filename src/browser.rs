use crate::filesystem::{Scope, is_link};
use anyhow::Result;
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub link: bool,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
    pub error: Option<String>,
}

/// Streaming batches let callers display partial results without blocking the UI.
/// Symlinks are listed, never followed. Individual metadata failures remain visible.
pub fn scan(
    scope: &Scope,
    path: &std::path::Path,
    cancel: &Arc<AtomicBool>,
    mut batch: impl FnMut(Vec<Entry>),
) -> Result<usize> {
    let path = scope.checked(path, false)?;
    let mut pending = Vec::with_capacity(256);
    let mut count = 0;
    for entry in fs::read_dir(path)? {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".filespring-") {
            continue;
        }
        let m = fs::symlink_metadata(entry.path());
        let (directory, link, bytes, modified, error) = match m {
            Ok(m) => (
                m.is_dir() && !is_link(&m),
                is_link(&m),
                m.len(),
                m.modified().ok(),
                None,
            ),
            Err(e) => (false, false, 0, None, Some(e.to_string())),
        };
        pending.push(Entry {
            path: entry.path(),
            name,
            directory,
            link,
            bytes,
            modified,
            error,
        });
        count += 1;
        if pending.len() == 256 {
            batch(std::mem::replace(&mut pending, Vec::with_capacity(256)));
        }
    }
    if !pending.is_empty() {
        batch(pending);
    }
    Ok(count)
}
