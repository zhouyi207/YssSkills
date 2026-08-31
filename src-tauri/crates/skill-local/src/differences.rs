use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use walkdir::{DirEntry, WalkDir};

use crate::LocalError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    File,
    Directory,
}

pub fn removed_skill_paths(current: &Path, replacement: &Path) -> Result<Vec<PathBuf>, LocalError> {
    let current_entries = collect_entries(current)?;
    let replacement_entries = collect_entries(replacement)?;
    let mut removed = Vec::new();
    let mut removed_directories = Vec::<PathBuf>::new();

    for (path, current_kind) in current_entries {
        if removed_directories
            .iter()
            .any(|directory| path.starts_with(directory) && path != *directory)
        {
            continue;
        }
        if replacement_entries.get(&path) == Some(&current_kind) {
            continue;
        }
        if current_kind == EntryKind::Directory {
            removed_directories.push(path.clone());
        }
        removed.push(path);
    }
    Ok(removed)
}

fn collect_entries(root: &Path) -> Result<BTreeMap<PathBuf, EntryKind>, LocalError> {
    let metadata = fs::symlink_metadata(root).map_err(|source| LocalError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(LocalError::NotDirectory {
            path: root.to_path_buf(),
        });
    }

    let mut entries = BTreeMap::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_regenerable_entry(entry));
    for entry in walker {
        let entry = entry.map_err(|source| LocalError::Walk {
            path: source
                .path()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| root.to_path_buf()),
            source,
        })?;
        if entry.depth() == 0 {
            continue;
        }
        if entry.file_type().is_symlink() {
            return Err(LocalError::NestedLink {
                path: entry.path().to_path_buf(),
            });
        }
        let kind = if entry.file_type().is_dir() {
            EntryKind::Directory
        } else if entry.file_type().is_file() {
            EntryKind::File
        } else {
            return Err(LocalError::Io {
                path: entry.path().to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "special filesystem entries are not supported",
                ),
            });
        };
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| LocalError::PathOutsideRoot {
                root: root.to_path_buf(),
                path: entry.path().to_path_buf(),
            })?
            .to_path_buf();
        entries.insert(relative, kind);
    }
    Ok(entries)
}

fn is_regenerable_entry(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    name == ".git"
        || name == ".DS_Store"
        || name == "Thumbs.db"
        || name == "__pycache__"
        || (entry.file_type().is_file() && name.ends_with(".pyc"))
}
