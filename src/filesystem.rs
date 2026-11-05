use crate::model::Snapshot;
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

/// A capability boundary. No traversal of symlinks or Windows reparse points.
#[derive(Clone, Debug)]
pub struct Scope {
    roots: Vec<PathBuf>,
    excluded: PathBuf,
}

impl Scope {
    pub fn new(roots: &[PathBuf], excluded: &Path) -> Result<Self> {
        ensure!(!roots.is_empty(), "Select at least one authorized root");
        let roots = roots
            .iter()
            .map(|p| fs::canonicalize(p).with_context(|| format!("Root {}", p.display())))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            roots.iter().all(|p| p.is_dir()),
            "All roots must be directories"
        );
        Ok(Self {
            roots,
            excluded: fs::canonicalize(excluded)?,
        })
    }
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
    pub fn checked(&self, path: &Path, missing_leaf: bool) -> Result<PathBuf> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        ensure!(
            !absolute.components().any(|c| c == Component::ParentDir),
            "Parent traversal is not accepted: {}",
            path.display()
        );
        // Check the spelling before canonicalization, so a symlink cannot disappear during resolution.
        let mut prefix = PathBuf::new();
        for component in absolute.components() {
            prefix.push(component);
            // A Windows drive/UNC prefix is not a standalone filesystem object.
            if matches!(component, Component::Prefix(_)) {
                continue;
            }
            match fs::symlink_metadata(&prefix) {
                Ok(m) => ensure!(
                    !is_link(&m),
                    "Symlink/reparse traversal is disabled: {}",
                    prefix.display()
                ),
                Err(e)
                    if missing_leaf
                        && prefix == absolute
                        && e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).with_context(|| format!("Inspect {}", prefix.display())),
            }
        }
        let normalized = if missing_leaf {
            fs::canonicalize(absolute.parent().context("Missing parent")?)?
                .join(absolute.file_name().context("Missing name")?)
        } else {
            fs::canonicalize(&absolute)?
        };
        ensure!(
            self.roots.iter().any(|root| normalized.starts_with(root)),
            "Outside authorized roots: {}",
            path.display()
        );
        ensure!(
            !normalized.starts_with(&self.excluded),
            "Application state is protected"
        );
        ensure!(
            !normalized
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(".filespring-")),
            "Reserved recovery filename"
        );
        Ok(normalized)
    }
    pub(crate) fn check_internal(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("Missing parent")?;
        self.checked(parent, false)?;
        match fs::symlink_metadata(path) {
            Ok(m) => ensure!(!is_link(&m), "Recovery file replaced with a link"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }
}

pub fn is_link(m: &fs::Metadata) -> bool {
    if m.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub fn identity(path: &Path) -> Result<String> {
    Ok(format!("{:?}", file_id::get_file_id(path)?))
}

pub fn snapshot(path: &Path) -> Result<Snapshot> {
    let before = fs::symlink_metadata(path)?;
    ensure!(
        !is_link(&before) && before.is_file(),
        "Only regular files are supported: {}",
        path.display()
    );
    let id = identity(path)?;
    let mut file = File::open(path)?;
    let mut hash = blake3::Hasher::new();
    let mut buffer = vec![0; 256 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let after = fs::symlink_metadata(path)?;
    let modified_ns = before.modified()?.duration_since(UNIX_EPOCH)?.as_nanos();
    ensure!(
        !is_link(&after)
            && before.len() == after.len()
            && before.modified()? == after.modified()?
            && id == identity(path)?,
        "File changed while hashing: {}",
        path.display()
    );
    Ok(Snapshot {
        identity: id,
        bytes: before.len(),
        modified_ns,
        hash: hash.finalize().to_hex().to_string(),
    })
}

pub fn verify(path: &Path, expected: &Snapshot) -> Result<()> {
    ensure!(
        snapshot(path)? == *expected,
        "File changed since preview: {}",
        path.display()
    );
    Ok(())
}

pub fn absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => bail!("Destination already exists: {}", path.display()),
        Err(e) => Err(e.into()),
    }
}

/// A disconnected or inaccessible path must never be interpreted as an absent file.
pub fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| format!("Inspect {}", path.display())),
    }
}

pub fn sync_parent(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path.parent().context("Missing parent")?)?.sync_all()?;
    // Windows std cannot open a directory for fsync. File data is still sync_all'ed.
    Ok(())
}

/// Copy into a new staging file. Caller journals its identity before writing data.
pub fn write_stage(source: &Path, expected: &Snapshot, file: &mut File) -> Result<()> {
    verify(source, expected)?;
    let mut input = File::open(source)?;
    let mut buffer = vec![0; 256 * 1024];
    let mut hash = blake3::Hasher::new();
    let mut bytes = 0;
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
        bytes += n as u64;
    }
    ensure!(
        bytes == expected.bytes && hash.finalize().to_hex().as_str() == expected.hash,
        "Source changed during copy"
    );
    verify(source, expected)?;
    file.set_permissions(input.metadata()?.permissions())?;
    file.set_times(fs::FileTimes::new().set_modified(input.metadata()?.modified()?))?;
    file.sync_all()?;
    Ok(())
}

pub fn new_stage(path: &Path) -> Result<File> {
    Ok(OpenOptions::new().write(true).create_new(true).open(path)?)
}

pub fn remove_owned(path: &Path, expected: &Snapshot) -> Result<()> {
    verify(path, expected)?;
    fs::remove_file(path)?;
    sync_parent(path)
}
