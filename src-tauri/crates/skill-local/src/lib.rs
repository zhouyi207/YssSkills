use std::{
    collections::HashSet,
    ffi::OsStr,
    fmt, fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use sha2::{Digest, Sha256};
use skill_core::{parse_skill_document, ContentHash, SkillDocument, SkillMarker, SkillParseError};

use walkdir::{DirEntry as WalkDirEntry, WalkDir};

mod operations;
mod watcher;

pub use operations::{
    copy_skill, delete_link, delete_skill, link_skill, link_target, remove_broken_links,
    ExistingDestination, LocalOperation, OperationResult, PlatformLinker,
};
pub use watcher::{WatchChange, WatchManager, WatchTarget, WatchTargetKind};

const IGNORED_HASH_NAMES: &[&str] = &[
    ".git",
    ".DS_Store",
    "Thumbs.db",
    ".gitignore",
    "__pycache__",
];
const IGNORED_SCAN_DIRECTORIES: &[&str] = &[".git", ".hub", "node_modules"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    Flat,
    Recursive,
}

#[derive(Debug)]
pub struct WatchFailure {
    source: notify::Error,
}

impl WatchFailure {
    fn from_notify(source: notify::Error) -> Self {
        Self { source }
    }
}

impl fmt::Display for WatchFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl std::error::Error for WatchFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

// `PathConflict::source` is path context, not an error source; keep the required
// field name without letting thiserror infer a non-error source field.
#[derive(Debug)]
pub enum LocalError {
    PathNotFound {
        path: PathBuf,
    },
    NotDirectory {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
    MarkerNotFound {
        path: PathBuf,
    },
    Parse {
        path: PathBuf,
        source: SkillParseError,
    },
    Walk {
        path: PathBuf,
        source: walkdir::Error,
    },
    PathOutsideRoot {
        root: PathBuf,
        path: PathBuf,
    },
    InvalidPathEncoding {
        path: PathBuf,
    },
    InvalidPath {
        path: PathBuf,
    },
    DestinationExists {
        path: PathBuf,
    },
    NotLink {
        path: PathBuf,
    },
    PathConflict {
        source: PathBuf,
        target: PathBuf,
    },
    NestedLink {
        path: PathBuf,
    },
    UnsupportedOperation {
        operation: &'static str,
    },
    VerificationRead {
        path: PathBuf,
        source: Box<LocalError>,
    },
    VerificationMismatch {
        path: PathBuf,
        expected: ContentHash,
        actual: ContentHash,
    },
    VerificationCleanup {
        path: PathBuf,
        operation: Box<LocalError>,
        cleanup: Box<LocalError>,
    },
    InvalidDebounce,
    DuplicateWatchTarget {
        id: String,
    },
    InvalidWatchTarget {
        id: String,
        path: PathBuf,
        reason: &'static str,
    },
    Watch {
        operation: &'static str,
        path: PathBuf,
        source: WatchFailure,
    },
    WatcherCallback {
        errors: Vec<WatchFailure>,
    },
    WatcherStatePoisoned,
    WatcherClosed,
}

impl fmt::Display for LocalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathNotFound { path } => {
                write!(formatter, "path does not exist: {}", path.display())
            }
            Self::NotDirectory { path } => {
                write!(formatter, "path is not a directory: {}", path.display())
            }
            Self::Io { path, source } => write!(
                formatter,
                "filesystem operation failed for {}: {source}",
                path.display()
            ),
            Self::MarkerNotFound { path } => {
                write!(
                    formatter,
                    "skill marker was not found in {}",
                    path.display()
                )
            }
            Self::Parse { path, source } => write!(
                formatter,
                "failed to parse skill marker {}: {source}",
                path.display()
            ),
            Self::Walk { path, source } => {
                write!(formatter, "failed to walk {}: {source}", path.display())
            }
            Self::PathOutsideRoot { root, path } => write!(
                formatter,
                "file path {} is outside scan root {}",
                path.display(),
                root.display()
            ),
            Self::InvalidPathEncoding { path } => write!(
                formatter,
                "path cannot be represented losslessly for hashing: {}",
                path.display()
            ),
            Self::InvalidPath { path } => {
                write!(formatter, "invalid path: {}", path.display())
            }
            Self::DestinationExists { path } => {
                write!(formatter, "destination already exists: {}", path.display())
            }
            Self::NotLink { path } => {
                write!(
                    formatter,
                    "path is not a symbolic link or junction: {}",
                    path.display()
                )
            }
            Self::PathConflict { source, target } => write!(
                formatter,
                "source and destination paths conflict: {} and {}",
                source.display(),
                target.display()
            ),
            Self::NestedLink { path } => write!(
                formatter,
                "skill directory contains a nested symbolic link or junction: {}",
                path.display()
            ),
            Self::UnsupportedOperation { operation } => write!(
                formatter,
                "operation is not supported on this platform: {operation}"
            ),
            Self::VerificationRead { path, source } => write!(
                formatter,
                "operation read-back failed for {}: {source}",
                path.display()
            ),
            Self::VerificationMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "operation verification failed for {}: expected {expected}, got {actual}",
                path.display()
            ),
            Self::VerificationCleanup {
                path,
                operation,
                cleanup,
            } => write!(
                formatter,
                "operation verification failed for {}: {operation}; cleanup failed: {cleanup}",
                path.display()
            ),
            Self::InvalidDebounce => write!(formatter, "debounce duration must be non-zero"),
            Self::DuplicateWatchTarget { id } => {
                write!(formatter, "watch target id is duplicated: {id}")
            }
            Self::InvalidWatchTarget { id, path, reason } => write!(
                formatter,
                "invalid watch target {id} at {}: {reason}",
                path.display()
            ),
            Self::Watch {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "watcher {operation} failed for {}: {source}",
                path.display()
            ),
            Self::WatcherCallback { errors } => {
                write!(formatter, "watcher callback reported errors: {errors:?}")
            }
            Self::WatcherStatePoisoned => write!(formatter, "watcher state lock is poisoned"),
            Self::WatcherClosed => write!(formatter, "watch manager is closed"),
        }
    }
}

impl std::error::Error for LocalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Walk { source, .. } => Some(source),
            Self::Watch { source, .. } => Some(source),
            Self::WatcherCallback { errors } => errors
                .first()
                .map(|source| source as &(dyn std::error::Error + 'static)),
            Self::VerificationRead { source, .. } => Some(source.as_ref()),
            Self::VerificationCleanup { operation, .. } => Some(operation.as_ref()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedSkill {
    pub path: PathBuf,
    pub link_target: Option<PathBuf>,
    pub marker: SkillMarker,
    pub marker_path: PathBuf,
    pub marker_modified_at: Option<SystemTime>,
    pub document: SkillDocument,
    pub content_hash: ContentHash,
}

#[derive(Debug)]
pub struct ScanDiagnostic {
    pub path: PathBuf,
    pub error: LocalError,
}

#[derive(Debug)]
pub struct ScanReport {
    pub skills: Vec<ScannedSkill>,
    pub diagnostics: Vec<ScanDiagnostic>,
}

pub fn find_skill_marker(path: &Path) -> Result<Option<(SkillMarker, PathBuf)>, LocalError> {
    ensure_directory(path)?;

    let entries = fs::read_dir(path).map_err(|source| map_io_error(path, source))?;
    let mut canonical = None;
    let mut legacy = None;

    for entry in entries {
        let entry = entry.map_err(|source| map_io_error(path, source))?;
        let file_name = entry.file_name();
        let marker = if file_name.as_os_str() == OsStr::new("SKILL.md") {
            SkillMarker::Canonical
        } else if file_name.as_os_str() == OsStr::new("skill.md") {
            SkillMarker::Legacy
        } else {
            continue;
        };
        let marker_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| map_io_error(&marker_path, source))?;
        if !file_type.is_file() {
            continue;
        }

        match marker {
            SkillMarker::Canonical => canonical = Some((marker, marker_path)),
            SkillMarker::Legacy => legacy = Some((marker, marker_path)),
        }
    }

    Ok(canonical.or(legacy))
}

pub fn read_skill(path: &Path) -> Result<ScannedSkill, LocalError> {
    let Some((marker, marker_path)) = find_skill_marker(path)? else {
        return Err(LocalError::MarkerNotFound {
            path: path.to_path_buf(),
        });
    };

    let bytes = fs::read(&marker_path).map_err(|source| map_io_error(&marker_path, source))?;
    let document = parse_skill_document(&bytes).map_err(|source| LocalError::Parse {
        path: marker_path.clone(),
        source,
    })?;
    let marker_modified_at = fs::metadata(&marker_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    let content_hash = hash_directory(path)?;
    let link_target = link_target(path)?;

    Ok(ScannedSkill {
        path: path.to_path_buf(),
        link_target,
        marker,
        marker_path,
        marker_modified_at,
        document,
        content_hash,
    })
}

pub fn scan_directory(root: &Path, mode: ScanMode) -> Result<ScanReport, LocalError> {
    ensure_directory(root)?;

    let candidates = match mode {
        ScanMode::Flat => collect_flat_candidates(root)?,
        ScanMode::Recursive => collect_recursive_candidates(root)?,
    };

    let mut skills = Vec::new();
    let mut diagnostics = Vec::new();
    for path in candidates {
        match read_skill(&path) {
            Ok(skill) => skills.push(skill),
            Err(error) => diagnostics.push(ScanDiagnostic { path, error }),
        }
    }

    skills.sort_by(|left, right| left.path.cmp(&right.path));
    diagnostics.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(ScanReport {
        skills,
        diagnostics,
    })
}

fn update_hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

pub fn hash_directory(path: &Path) -> Result<ContentHash, LocalError> {
    ensure_directory(path)?;
    let root = fs::canonicalize(path).map_err(|source| map_io_error(path, source))?;
    let mut files = Vec::new();

    let entries = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !hash_entry_is_ignored(entry));
    for entry in entries {
        let entry = entry.map_err(|source| {
            let error_path = source
                .path()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| root.clone());
            LocalError::Walk {
                path: error_path,
                source,
            }
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let relative_path = relative_path_bytes(&root, entry.path())?;
        files.push((relative_path, entry.into_path()));
    }

    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (relative_path, file_path) in files {
        update_hash_field(&mut hasher, &relative_path);
        let content = fs::read(&file_path).map_err(|source| map_io_error(&file_path, source))?;
        update_hash_field(&mut hasher, &content);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&file_path)
                .map_err(|source| map_io_error(&file_path, source))?
                .permissions()
                .mode()
                & 0o111;
            update_hash_field(&mut hasher, &mode.to_le_bytes());
        }
    }

    Ok(ContentHash::from_bytes(hasher.finalize().into()))
}

fn collect_flat_candidates(root: &Path) -> Result<Vec<PathBuf>, LocalError> {
    let entries = fs::read_dir(root).map_err(|source| map_io_error(root, source))?;
    let mut candidates = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|source| map_io_error(root, source))?;
        let path = entry.path();
        if entry_directory_target(&entry)?.is_none() {
            continue;
        }
        if find_skill_marker(&path)?.is_some() {
            candidates.push(path);
        }
    }

    candidates.sort();
    Ok(candidates)
}

fn collect_recursive_candidates(root: &Path) -> Result<Vec<PathBuf>, LocalError> {
    let mut candidates = Vec::new();
    let mut visited = HashSet::new();
    let canonical = fs::canonicalize(root).map_err(|source| map_io_error(root, source))?;
    collect_recursive(root, canonical, &mut visited, &mut candidates)?;
    candidates.sort();
    Ok(candidates)
}

fn collect_recursive(
    directory: &Path,
    canonical: PathBuf,
    visited: &mut HashSet<PathBuf>,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), LocalError> {
    if !visited.insert(canonical) {
        return Ok(());
    }

    let entries = fs::read_dir(directory).map_err(|source| map_io_error(directory, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| map_io_error(directory, source))?;
        if is_ignored_scan_directory(&entry) {
            continue;
        }

        let Some(canonical) = entry_directory_target(&entry)? else {
            continue;
        };
        let path = entry.path();
        if find_skill_marker(&path)?.is_some() {
            candidates.push(path);
            continue;
        }

        collect_recursive(&path, canonical, visited, candidates)?;
    }

    Ok(())
}

fn entry_directory_target(entry: &fs::DirEntry) -> Result<Option<PathBuf>, LocalError> {
    let path = entry.path();
    let file_type = entry
        .file_type()
        .map_err(|source| map_io_error(&path, source))?;
    if !file_type.is_dir() && !file_type.is_symlink() {
        return Ok(None);
    }

    let canonical = match fs::canonicalize(&path) {
        Ok(canonical) => canonical,
        Err(source) if file_type.is_symlink() && is_skippable_symlink_error(&source) => {
            return Ok(None);
        }
        Err(source) => return Err(map_io_error(&path, source)),
    };
    let metadata = fs::metadata(&canonical).map_err(|source| map_io_error(&path, source))?;
    if metadata.is_dir() {
        Ok(Some(canonical))
    } else {
        Ok(None)
    }
}

fn is_skippable_symlink_error(source: &io::Error) -> bool {
    if source.kind() == io::ErrorKind::NotFound {
        return true;
    }

    #[cfg(unix)]
    if source.raw_os_error() == Some(libc::ELOOP) {
        return true;
    }

    #[cfg(windows)]
    if matches!(source.raw_os_error(), Some(114) | Some(1921)) {
        return true;
    }

    false
}

fn is_ignored_scan_directory(entry: &fs::DirEntry) -> bool {
    let file_name = entry.file_name();
    let name = file_name.to_string_lossy();
    IGNORED_SCAN_DIRECTORIES
        .iter()
        .any(|ignored| name == *ignored)
}

fn relative_path_bytes(root: &Path, path: &Path) -> Result<Vec<u8>, LocalError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| LocalError::PathOutsideRoot {
            root: root.to_path_buf(),
            path: path.to_path_buf(),
        })?;

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        return Ok(relative.as_os_str().as_bytes().to_vec());
    }

    #[cfg(windows)]
    {
        let relative = relative
            .to_str()
            .ok_or_else(|| LocalError::InvalidPathEncoding {
                path: path.to_path_buf(),
            })?;
        Ok(relative.replace('\\', "/").into_bytes())
    }

    #[cfg(not(any(unix, windows)))]
    {
        let relative = relative
            .to_str()
            .ok_or_else(|| LocalError::InvalidPathEncoding {
                path: path.to_path_buf(),
            })?;
        Ok(relative.as_bytes().to_vec())
    }
}

fn ensure_directory(path: &Path) -> Result<(), LocalError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(LocalError::NotDirectory {
            path: path.to_path_buf(),
        }),
        Err(source) => Err(map_io_error(path, source)),
    }
}

pub(crate) fn map_io_error(path: &Path, source: io::Error) -> LocalError {
    match source.kind() {
        io::ErrorKind::NotFound => LocalError::PathNotFound {
            path: path.to_path_buf(),
        },
        io::ErrorKind::NotADirectory => LocalError::NotDirectory {
            path: path.to_path_buf(),
        },
        _ => LocalError::Io {
            path: path.to_path_buf(),
            source,
        },
    }
}

fn is_ignored_hash_name(name: &str) -> bool {
    IGNORED_HASH_NAMES.contains(&name) || name.ends_with(".pyc")
}

fn hash_entry_is_ignored(entry: &WalkDirEntry) -> bool {
    is_ignored_hash_name(&entry.file_name().to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::is_ignored_hash_name;

    #[test]
    fn hash_ignore_rules_match_only_the_declared_names() {
        assert!(is_ignored_hash_name(".git"));
        assert!(is_ignored_hash_name("module.pyc"));
        assert!(!is_ignored_hash_name("module.PYC"));
        assert!(!is_ignored_hash_name(".hub"));
    }
}
