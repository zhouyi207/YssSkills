use std::{
    ffi::{OsStr, OsString},
    fs, io,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use walkdir::WalkDir;

use super::{
    ensure_directory, find_skill_marker, hash_directory, map_io_error, read_skill, ContentHash,
    LocalError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingDestination {
    Reject,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalOperation {
    Copied,
    Linked,
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlatformLinkStrategy {
    #[cfg(unix)]
    SymbolicLink,
    #[cfg(windows)]
    Junction,
    #[cfg(not(any(unix, windows)))]
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformLinker {
    strategy: PlatformLinkStrategy,
}

impl PlatformLinker {
    pub fn detect() -> Self {
        #[cfg(unix)]
        {
            Self {
                strategy: PlatformLinkStrategy::SymbolicLink,
            }
        }

        #[cfg(windows)]
        {
            Self {
                strategy: PlatformLinkStrategy::Junction,
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            Self {
                strategy: PlatformLinkStrategy::Unsupported,
            }
        }
    }

    pub fn link(
        &self,
        source: &Path,
        target: &Path,
        existing: ExistingDestination,
    ) -> Result<OperationResult, LocalError> {
        link_skill_with_strategy(source, target, existing, self.strategy)
    }
}

impl Default for PlatformLinker {
    fn default() -> Self {
        Self::detect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationResult {
    pub operation: LocalOperation,
    pub path: PathBuf,
}

static AUXILIARY_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn copy_skill(
    source: &Path,
    target: &Path,
    existing: ExistingDestination,
) -> Result<OperationResult, LocalError> {
    let target_path = normalize_final_path(target)?;
    ensure_directory(source)?;
    let source_metadata =
        fs::symlink_metadata(source).map_err(|source_error| map_io_error(source, source_error))?;
    let source_is_link = is_link(source, source_metadata.file_type())?;
    preflight_nested_links(source)?;

    let source_hash = read_skill(source)?.content_hash;
    let canonical_source =
        fs::canonicalize(source).map_err(|source_error| map_io_error(source, source_error))?;
    if source_is_link {
        preflight_nested_links(&canonical_source)?;
    }

    let target_exists = match fs::symlink_metadata(&target_path) {
        Ok(_) => true,
        Err(source_error) if source_error.kind() == io::ErrorKind::NotFound => false,
        Err(source_error) => return Err(map_io_error(&target_path, source_error)),
    };

    if target_exists && matches!(existing, ExistingDestination::Reject) {
        return Err(LocalError::DestinationExists {
            path: target.to_path_buf(),
        });
    }

    let canonical_target = canonicalize_existing_prefix(&target_path)?;
    if paths_conflict(&canonical_source, &canonical_target) {
        return Err(LocalError::PathConflict {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
        });
    }

    let target_parent = target_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure_directory(target_parent)?;

    let staging_path = auxiliary_path(target_parent, "staging")?;
    prepare_staging(&staging_path, target, &source_hash, |staging| {
        copy_directory(&canonical_source, staging)
    })?;
    commit_staged(&staging_path, &target_path, target_exists, target_parent)?;

    Ok(OperationResult {
        operation: LocalOperation::Copied,
        path: target.to_path_buf(),
    })
}

pub fn link_skill(
    source: &Path,
    target: &Path,
    existing: ExistingDestination,
) -> Result<OperationResult, LocalError> {
    PlatformLinker::detect().link(source, target, existing)
}

fn link_skill_with_strategy(
    source: &Path,
    target: &Path,
    existing: ExistingDestination,
    strategy: PlatformLinkStrategy,
) -> Result<OperationResult, LocalError> {
    let target_path = normalize_final_path(target)?;
    let source_path = normalize_final_path(source)?;
    ensure_directory(source)?;
    let source_hash = read_skill(source)?.content_hash;
    let canonical_source =
        fs::canonicalize(source).map_err(|source_error| map_io_error(source, source_error))?;
    let source_entry = canonicalize_existing_prefix(&source_path)?;

    let target_exists = match fs::symlink_metadata(&target_path) {
        Ok(_) => true,
        Err(source_error) if source_error.kind() == io::ErrorKind::NotFound => false,
        Err(source_error) => return Err(map_io_error(&target_path, source_error)),
    };

    if target_exists && matches!(existing, ExistingDestination::Reject) {
        return Err(LocalError::DestinationExists {
            path: target.to_path_buf(),
        });
    }

    let canonical_target = canonicalize_existing_prefix(&target_path)?;
    if paths_conflict(&canonical_source, &canonical_target)
        || paths_conflict(&source_entry, &canonical_target)
    {
        return Err(LocalError::PathConflict {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
        });
    }

    let target_parent = target_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure_directory(target_parent)?;

    let staging_path = auxiliary_path(target_parent, "staging")?;
    prepare_staging(&staging_path, target, &source_hash, |staging| {
        create_platform_link(&canonical_source, staging, strategy)
    })?;
    commit_staged(&staging_path, &target_path, target_exists, target_parent)?;

    Ok(OperationResult {
        operation: LocalOperation::Linked,
        path: target.to_path_buf(),
    })
}

pub fn delete_skill(path: &Path) -> Result<OperationResult, LocalError> {
    if find_skill_marker(path)?.is_none() {
        return Err(LocalError::MarkerNotFound {
            path: path.to_path_buf(),
        });
    }

    remove_without_following(path)?;

    Ok(OperationResult {
        operation: LocalOperation::Deleted,
        path: path.to_path_buf(),
    })
}

fn prepare_staging<F>(
    staging: &Path,
    target: &Path,
    expected_hash: &ContentHash,
    operation: F,
) -> Result<(), LocalError>
where
    F: FnOnce(&Path) -> Result<(), LocalError>,
{
    if let Err(operation_error) = operation(staging) {
        return Err(cleanup_after_staging_failure(staging, operation_error));
    }

    if let Err(verification_error) = verify_staged_skill(staging, target, expected_hash) {
        return Err(cleanup_after_staging_failure(staging, verification_error));
    }

    Ok(())
}

fn verify_staged_skill(
    staging: &Path,
    target: &Path,
    expected_hash: &ContentHash,
) -> Result<(), LocalError> {
    let _verified_skill =
        read_skill(staging).map_err(|source_error| LocalError::VerificationRead {
            path: target.to_path_buf(),
            source: Box::new(source_error),
        })?;
    let actual_hash =
        hash_directory(staging).map_err(|source_error| LocalError::VerificationRead {
            path: target.to_path_buf(),
            source: Box::new(source_error),
        })?;
    if actual_hash != *expected_hash {
        return Err(LocalError::VerificationMismatch {
            path: target.to_path_buf(),
            expected: *expected_hash,
            actual: actual_hash,
        });
    }

    Ok(())
}

fn cleanup_after_staging_failure(staging: &Path, operation_error: LocalError) -> LocalError {
    match remove_if_exists(staging) {
        Ok(()) => operation_error,
        Err(cleanup_error) => LocalError::VerificationCleanup {
            path: staging.to_path_buf(),
            operation: Box::new(operation_error),
            cleanup: Box::new(cleanup_error),
        },
    }
}

fn remove_if_exists(path: &Path) -> Result<(), LocalError> {
    match remove_without_following(path) {
        Ok(()) | Err(LocalError::PathNotFound { .. }) => Ok(()),
        Err(error) => Err(error),
    }
}

fn commit_staged(
    staging: &Path,
    target: &Path,
    target_exists: bool,
    parent: &Path,
) -> Result<(), LocalError> {
    if !target_exists {
        return match rename_entry(staging, target) {
            Ok(()) => Ok(()),
            Err(operation_error) => Err(cleanup_after_staging_failure(staging, operation_error)),
        };
    }

    let backup = match auxiliary_path(parent, "backup") {
        Ok(path) => path,
        Err(operation_error) => {
            return Err(cleanup_after_staging_failure(staging, operation_error));
        }
    };

    if let Err(operation_error) = rename_entry(target, &backup) {
        return Err(cleanup_after_staging_failure(staging, operation_error));
    }

    if let Err(replacement_error) = rename_entry(staging, target) {
        let staging_cleanup = remove_if_exists(staging).err();
        let restore_result = rename_entry(&backup, target);
        let operation_error = match staging_cleanup {
            Some(cleanup_error) => LocalError::VerificationCleanup {
                path: staging.to_path_buf(),
                operation: Box::new(replacement_error),
                cleanup: Box::new(cleanup_error),
            },
            None => replacement_error,
        };

        return Err(match restore_result {
            Ok(()) => operation_error,
            Err(restore_error) => LocalError::VerificationCleanup {
                path: backup.to_path_buf(),
                operation: Box::new(operation_error),
                cleanup: Box::new(backup_restore_failure(&backup, target, restore_error)),
            },
        });
    }

    if let Err(cleanup_error) = remove_if_exists(&backup) {
        return Err(backup_cleanup_failure(target, &backup, cleanup_error));
    }

    Ok(())
}

fn rename_entry(source: &Path, target: &Path) -> Result<(), LocalError> {
    fs::rename(source, target).map_err(|source_error| LocalError::Io {
        path: target.to_path_buf(),
        source: io::Error::new(
            source_error.kind(),
            format!(
                "failed to rename {} to {}: {source_error}",
                source.display(),
                target.display()
            ),
        ),
    })
}

fn backup_restore_failure(backup: &Path, target: &Path, error: LocalError) -> LocalError {
    LocalError::Io {
        path: target.to_path_buf(),
        source: io::Error::other(format!(
            "failed to restore backup {} to {}; backup may remain at {}: {error}",
            backup.display(),
            target.display(),
            backup.display()
        )),
    }
}

fn backup_cleanup_failure(target: &Path, backup: &Path, error: LocalError) -> LocalError {
    LocalError::Io {
        path: backup.to_path_buf(),
        source: io::Error::other(format!(
            "replacement committed at {}; failed to remove backup {}; backup retained: {error}",
            target.display(),
            backup.display()
        )),
    }
}

fn auxiliary_path(parent: &Path, purpose: &str) -> Result<PathBuf, LocalError> {
    loop {
        let sequence = AUXILIARY_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut file_name = OsString::from(".skill-local-");
        file_name.push(purpose);
        file_name.push("-");
        file_name.push(std::process::id().to_string());
        file_name.push("-");
        file_name.push(sequence.to_string());
        let candidate = parent.join(file_name);

        match fs::symlink_metadata(&candidate) {
            Ok(_) => continue,
            Err(source_error) if source_error.kind() == io::ErrorKind::NotFound => {
                return Ok(candidate);
            }
            Err(source_error) => return Err(map_io_error(&candidate, source_error)),
        }
    }
}

fn create_platform_link(
    source: &Path,
    target: &Path,
    strategy: PlatformLinkStrategy,
) -> Result<(), LocalError> {
    match strategy {
        #[cfg(unix)]
        PlatformLinkStrategy::SymbolicLink => create_symbolic_link(source, target),
        #[cfg(windows)]
        PlatformLinkStrategy::Junction => create_junction(source, target),
        #[cfg(not(any(unix, windows)))]
        PlatformLinkStrategy::Unsupported => Err(LocalError::UnsupportedOperation {
            operation: "platform link",
        }),
    }
}

pub fn delete_link(path: &Path) -> Result<OperationResult, LocalError> {
    if link_target(path)?.is_none() {
        return Err(LocalError::NotLink {
            path: path.to_path_buf(),
        });
    }
    remove_without_following(path)?;
    Ok(OperationResult {
        operation: LocalOperation::Deleted,
        path: path.to_path_buf(),
    })
}

pub fn remove_broken_links(root: &Path) -> Result<Vec<PathBuf>, LocalError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(map_io_error(root, source)),
    };
    let mut removed = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| map_io_error(root, source))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|source| map_io_error(&path, source))?;
        if !is_link(&path, metadata.file_type())? {
            continue;
        }
        let Some(target) = link_target(&path)? else {
            continue;
        };
        match target.try_exists() {
            Ok(true) => continue,
            Ok(false) => {}
            Err(source) => return Err(map_io_error(&path, source)),
        }
        remove_link(&path)?;
        removed.push(path);
    }
    removed.sort();
    Ok(removed)
}

#[cfg(unix)]
fn create_symbolic_link(source: &Path, target: &Path) -> Result<(), LocalError> {
    std::os::unix::fs::symlink(source, target)
        .map_err(|source_error| map_io_error(target, source_error))
}

#[cfg(windows)]
fn create_junction(source: &Path, target: &Path) -> Result<(), LocalError> {
    junction::create(source, target).map_err(|source_error| map_io_error(target, source_error))
}

fn preflight_nested_links(root: &Path) -> Result<(), LocalError> {
    let entries = collect_walk_entries(root)?;
    for (path, depth) in entries {
        if depth == 0 {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)
            .map_err(|source_error| map_io_error(&path, source_error))?;
        if is_link(&path, metadata.file_type())? {
            return Err(LocalError::NestedLink { path });
        }
    }

    Ok(())
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), LocalError> {
    let entries = collect_walk_entries(source)?;
    fs::create_dir(target).map_err(|source_error| map_io_error(target, source_error))?;

    for (source_path, depth) in entries {
        if depth == 0 {
            continue;
        }

        let metadata = fs::symlink_metadata(&source_path)
            .map_err(|source_error| map_io_error(&source_path, source_error))?;
        if is_link(&source_path, metadata.file_type())? {
            return Err(LocalError::NestedLink { path: source_path });
        }

        let relative_path =
            source_path
                .strip_prefix(source)
                .map_err(|_| LocalError::PathOutsideRoot {
                    root: source.to_path_buf(),
                    path: source_path.to_path_buf(),
                })?;
        let target_path = target.join(relative_path);

        if metadata.is_dir() {
            fs::create_dir(&target_path)
                .map_err(|source_error| map_io_error(&target_path, source_error))?;
        } else if metadata.is_file() {
            copy_file_without_following(&source_path, &target_path)
                .map_err(|source_error| map_io_error(&target_path, source_error))?;
        } else {
            return Err(LocalError::Io {
                path: source_path,
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "copying special filesystem entries is not supported",
                ),
            });
        }
    }

    Ok(())
}

#[cfg(unix)]
fn open_source_without_following(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(windows)]
fn open_source_without_following(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_source_without_following(_path: &Path) -> io::Result<fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "copying files without following links is not supported on this platform",
    ))
}

fn copy_file_without_following(source: &Path, target: &Path) -> io::Result<()> {
    let mut source_file = open_source_without_following(source)?;
    let source_metadata = source_file.metadata()?;

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        if source_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "copying reparse points is not supported",
            ));
        }
    }

    if !source_metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "copying non-regular filesystem entries is not supported",
        ));
    }

    let mut target_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    io::copy(&mut source_file, &mut target_file)?;
    target_file.set_permissions(source_metadata.permissions())?;

    Ok(())
}

fn collect_walk_entries(root: &Path) -> Result<Vec<(PathBuf, usize)>, LocalError> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        let entry = entry.map_err(|source| map_walk_error(root, source))?;
        entries.push((entry.path().to_path_buf(), entry.depth()));
    }
    Ok(entries)
}

fn canonicalize_existing_prefix(path: &Path) -> Result<PathBuf, LocalError> {
    if path.as_os_str().is_empty() {
        return Err(LocalError::InvalidPath {
            path: path.to_path_buf(),
        });
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link(path, metadata.file_type())? => {
            return canonicalize_without_following_final(path);
        }
        Ok(_) => {}
        Err(source_error) if source_error.kind() == io::ErrorKind::NotFound => {}
        Err(source_error) => return Err(map_io_error(path, source_error)),
    }

    let mut unresolved = Vec::<OsString>::new();
    let mut current = path.to_path_buf();

    loop {
        match fs::canonicalize(&current) {
            Ok(mut canonical) => {
                for component in unresolved.iter().rev() {
                    if component == OsStr::new(".") {
                        continue;
                    }
                    if component == OsStr::new("..") {
                        canonical.pop();
                    } else {
                        canonical.push(component);
                    }
                }
                return Ok(canonical);
            }
            Err(source_error) if source_error.kind() == io::ErrorKind::NotFound => {
                let Some(file_name) = current.file_name() else {
                    return Err(map_io_error(&current, source_error));
                };
                unresolved.push(file_name.to_os_string());

                let Some(parent) = current.parent() else {
                    return Err(map_io_error(&current, source_error));
                };
                current = if parent.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    parent.to_path_buf()
                };
            }
            Err(source_error) => return Err(map_io_error(&current, source_error)),
        }
    }
}

fn canonicalize_without_following_final(path: &Path) -> Result<PathBuf, LocalError> {
    let Some(file_name) = path.file_name() else {
        return Err(LocalError::InvalidPath {
            path: path.to_path_buf(),
        });
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut canonical_parent =
        fs::canonicalize(parent).map_err(|source_error| map_io_error(parent, source_error))?;
    canonical_parent.push(file_name);
    Ok(canonical_parent)
}

fn paths_conflict(source: &Path, target: &Path) -> bool {
    source == target || source.starts_with(target) || target.starts_with(source)
}

fn normalize_final_path(path: &Path) -> Result<PathBuf, LocalError> {
    if path.as_os_str().is_empty() {
        return Err(LocalError::InvalidPath {
            path: path.to_path_buf(),
        });
    }

    let mut normalized = PathBuf::new();
    let mut has_normal_component = false;
    for component in path.components() {
        match component {
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err(LocalError::InvalidPath {
                    path: path.to_path_buf(),
                });
            }
            Component::Normal(_) => has_normal_component = true,
            Component::Prefix(_) | Component::RootDir => {}
        }
        normalized.push(component.as_os_str());
    }

    if !has_normal_component {
        return Err(LocalError::InvalidPath {
            path: path.to_path_buf(),
        });
    }

    Ok(normalized)
}

pub(crate) fn remove_without_following(path: &Path) -> Result<(), LocalError> {
    let path = normalize_final_path(path)?;
    let metadata =
        fs::symlink_metadata(&path).map_err(|source_error| map_io_error(&path, source_error))?;
    if is_link(&path, metadata.file_type())? {
        return remove_link(&path);
    }

    if metadata.is_dir() {
        fs::remove_dir_all(&path).map_err(|source_error| map_io_error(&path, source_error))?;
    } else {
        fs::remove_file(&path).map_err(|source_error| map_io_error(&path, source_error))?;
    }

    Ok(())
}

#[cfg(windows)]
fn is_link(path: &Path, file_type: fs::FileType) -> Result<bool, LocalError> {
    if file_type.is_symlink() {
        return Ok(true);
    }
    if !file_type.is_dir() {
        return Ok(false);
    }

    is_junction(path)
}

#[cfg(windows)]
pub fn link_target(path: &Path) -> Result<Option<PathBuf>, LocalError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| map_io_error(path, source))?;
    let target = if metadata.file_type().is_symlink() {
        Some(fs::read_link(path).map_err(|source| map_io_error(path, source))?)
    } else if metadata.is_dir() {
        match junction::get_target(path) {
            Ok(target) => Some(target),
            Err(source) if is_not_a_junction_error(&source) => None,
            Err(source) => return Err(map_io_error(path, source)),
        }
    } else {
        None
    };
    Ok(target.map(|target| resolve_link_target(path, target)))
}

#[cfg(not(windows))]
fn is_link(_path: &Path, file_type: fs::FileType) -> Result<bool, LocalError> {
    Ok(file_type.is_symlink())
}

#[cfg(not(windows))]
pub fn link_target(path: &Path) -> Result<Option<PathBuf>, LocalError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| map_io_error(path, source))?;
    if !metadata.file_type().is_symlink() {
        return Ok(None);
    }
    let target = fs::read_link(path).map_err(|source| map_io_error(path, source))?;
    Ok(Some(resolve_link_target(path, target)))
}

fn resolve_link_target(path: &Path, target: PathBuf) -> PathBuf {
    if target.is_absolute() {
        target
    } else {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .join(target)
    }
}

#[cfg(windows)]
fn is_junction(path: &Path) -> Result<bool, LocalError> {
    match junction::get_target(path) {
        Ok(_) => Ok(true),
        Err(source_error) if is_not_a_junction_error(&source_error) => Ok(false),
        Err(source_error) => Err(map_io_error(path, source_error)),
    }
}

#[cfg(windows)]
fn is_not_a_junction_error(source: &io::Error) -> bool {
    source.raw_os_error() == Some(4390)
        || (source.kind() == io::ErrorKind::Other
            && source.to_string() == "not a reparse tag mount point")
}

#[cfg(windows)]
fn remove_link(path: &Path) -> Result<(), LocalError> {
    if is_junction(path)? {
        junction::delete(path).map_err(|source_error| map_io_error(path, source_error))?;
        fs::remove_dir(path).map_err(|source_error| map_io_error(path, source_error))?;
        return Ok(());
    }

    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(directory_error) => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(file_error) => {
                let source_error = if file_error.kind() == io::ErrorKind::NotADirectory {
                    directory_error
                } else {
                    file_error
                };
                Err(map_io_error(path, source_error))
            }
        },
    }
}

#[cfg(not(windows))]
fn remove_link(path: &Path) -> Result<(), LocalError> {
    fs::remove_file(path).map_err(|source_error| map_io_error(path, source_error))
}

fn map_walk_error(root: &Path, source: walkdir::Error) -> LocalError {
    let path = source
        .path()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.to_path_buf());
    LocalError::Walk { path, source }
}
