use std::{
    fs, io,
    path::{Component, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use tempfile::TempDir;
use thiserror::Error;

use crate::{parse_git_source, SourceParseError};

const DEFAULT_GIT_TIMEOUT: Duration = Duration::from_secs(90);

pub struct GitCheckout {
    directory: TempDir,
    next_materialization: usize,
}

impl GitCheckout {
    pub fn clone(source_url: &str, reference: Option<&str>) -> Result<Self, GitCheckoutError> {
        let source = parse_git_source(source_url)?;
        let reference = reference
            .or(source.branch.as_deref())
            .map(validate_reference)
            .transpose()?;
        let directory = tempfile::tempdir().map_err(GitCheckoutError::CreateTemporaryDirectory)?;
        let repository_path = directory.path().join("repository");
        clone_repository(
            &source.clone_url,
            reference.as_deref(),
            &repository_path,
            DEFAULT_GIT_TIMEOUT,
        )?;
        Ok(Self {
            directory,
            next_materialization: 0,
        })
    }

    pub fn skill_directory(&mut self, skill_path: &str) -> Result<PathBuf, GitCheckoutError> {
        let relative_marker = validate_skill_path(skill_path)?;
        let repository_root = self.directory.path().join("repository");
        let marker_path = repository_root.join(&relative_marker);
        let marker_metadata = fs::symlink_metadata(&marker_path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                GitCheckoutError::SkillPathNotFound {
                    skill_path: skill_path.to_owned(),
                }
            } else {
                GitCheckoutError::InspectSkillPath { source }
            }
        })?;
        if !marker_metadata.is_file() || marker_metadata.file_type().is_symlink() {
            return Err(GitCheckoutError::InvalidSkillPath);
        }
        let canonical_root = fs::canonicalize(&repository_root)
            .map_err(|source| GitCheckoutError::InspectSkillPath { source })?;
        let canonical_marker = fs::canonicalize(&marker_path)
            .map_err(|source| GitCheckoutError::InspectSkillPath { source })?;
        if !canonical_marker.starts_with(&canonical_root) {
            return Err(GitCheckoutError::InvalidSkillPath);
        }
        let skill_directory = relative_marker.parent().unwrap_or_else(|| Path::new(""));
        let sequence = self.next_materialization;
        self.next_materialization = self.next_materialization.saturating_add(1);
        let archive_path = self.directory.path().join(format!("skill-{sequence}.tar"));
        let materialized_path = self
            .directory
            .path()
            .join(format!("materialized-{sequence}"));
        fs::create_dir(&materialized_path)
            .map_err(|source| GitCheckoutError::MaterializeSkill { source })?;
        archive_skill_directory(
            &repository_root,
            skill_directory,
            &archive_path,
            DEFAULT_GIT_TIMEOUT,
        )?;
        extract_skill_archive(&archive_path, &materialized_path)?;
        let materialized_marker = materialized_path.join(
            relative_marker
                .file_name()
                .ok_or(GitCheckoutError::InvalidSkillPath)?,
        );
        if !materialized_marker.is_file() {
            return Err(GitCheckoutError::InvalidSkillPath);
        }
        Ok(materialized_path)
    }
}

#[derive(Debug, Error)]
pub enum GitCheckoutError {
    #[error(transparent)]
    InvalidSource(#[from] SourceParseError),
    #[error("git reference is invalid")]
    InvalidReference,
    #[error("Skill path must be a safe repository-relative SKILL.md path")]
    InvalidSkillPath,
    #[error("Skill path does not exist in the fetched repository: {skill_path}")]
    SkillPathNotFound { skill_path: String },
    #[error("failed to create a temporary checkout directory")]
    CreateTemporaryDirectory(#[source] io::Error),
    #[error("Git could not be started")]
    StartGit(#[source] io::Error),
    #[error("Git clone failed with status {status:?}")]
    CloneFailed { status: Option<i32> },
    #[error("Git clone exceeded the {seconds}-second timeout")]
    CloneTimedOut { seconds: u64 },
    #[error("Git archive failed with status {status:?}")]
    ArchiveFailed { status: Option<i32> },
    #[error("Git archive exceeded the {seconds}-second timeout")]
    ArchiveTimedOut { seconds: u64 },
    #[error("failed to materialize the fetched Skill")]
    MaterializeSkill { source: io::Error },
    #[error("failed to inspect the fetched Skill path")]
    InspectSkillPath { source: io::Error },
}

fn validate_reference(value: &str) -> Result<String, GitCheckoutError> {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('-')
        || value.starts_with('.')
        || value.ends_with('.')
        || value.ends_with(".lock")
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains("..")
        || value.contains("//")
        || value.contains("@{")
        || value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '~' | '^' | ':' | '?' | '*' | '[' | ']' | '\\')
        })
    {
        return Err(GitCheckoutError::InvalidReference);
    }
    Ok(value.to_owned())
}

fn validate_skill_path(value: &str) -> Result<PathBuf, GitCheckoutError> {
    let value = value.trim();
    if value.is_empty() || value.contains('\\') || value.chars().any(char::is_control) {
        return Err(GitCheckoutError::InvalidSkillPath);
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_)
                    | Component::RootDir
                    | Component::ParentDir
                    | Component::CurDir
            )
        })
    {
        return Err(GitCheckoutError::InvalidSkillPath);
    }
    match path.file_name().and_then(|name| name.to_str()) {
        Some("SKILL.md" | "skill.md") => Ok(path.to_path_buf()),
        _ => Err(GitCheckoutError::InvalidSkillPath),
    }
}

fn clone_repository(
    source_url: &str,
    reference: Option<&str>,
    destination: &Path,
    timeout: Duration,
) -> Result<(), GitCheckoutError> {
    let mut command = Command::new("git");
    command
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg("--no-tags")
        .arg("--single-branch");
    if let Some(reference) = reference {
        command.arg("--branch").arg(reference);
    }
    command
        .arg("--")
        .arg(source_url)
        .arg(destination)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command.spawn().map_err(GitCheckoutError::StartGit)?;
    let status = wait_for_child(&mut child, timeout)
        .map_err(GitCheckoutError::StartGit)?
        .ok_or(GitCheckoutError::CloneTimedOut {
            seconds: timeout.as_secs(),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(GitCheckoutError::CloneFailed {
            status: status.code(),
        })
    }
}

fn archive_skill_directory(
    repository_root: &Path,
    skill_directory: &Path,
    archive_path: &Path,
    timeout: Duration,
) -> Result<(), GitCheckoutError> {
    let treeish = if skill_directory.as_os_str().is_empty() {
        "HEAD".to_owned()
    } else {
        format!(
            "HEAD:{}",
            skill_directory.to_string_lossy().replace('\\', "/")
        )
    };
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repository_root)
        .arg("archive")
        .arg("--format=tar")
        .arg("--output")
        .arg(archive_path)
        .arg(treeish)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(GitCheckoutError::StartGit)?;
    let status = wait_for_child(&mut child, timeout)
        .map_err(GitCheckoutError::StartGit)?
        .ok_or(GitCheckoutError::ArchiveTimedOut {
            seconds: timeout.as_secs(),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(GitCheckoutError::ArchiveFailed {
            status: status.code(),
        })
    }
}

fn extract_skill_archive(archive_path: &Path, target: &Path) -> Result<(), GitCheckoutError> {
    let archive_file = fs::File::open(archive_path)
        .map_err(|source| GitCheckoutError::MaterializeSkill { source })?;
    let mut archive = tar::Archive::new(archive_file);
    let entries = archive
        .entries()
        .map_err(|source| GitCheckoutError::MaterializeSkill { source })?;
    for entry in entries {
        let mut entry = entry.map_err(|source| GitCheckoutError::MaterializeSkill { source })?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_pax_global_extensions() || entry_type.is_pax_local_extensions() {
            continue;
        }
        if !entry_type.is_file() && !entry_type.is_dir() {
            return Err(GitCheckoutError::InvalidSkillPath);
        }
        let path = entry
            .path()
            .map_err(|source| GitCheckoutError::MaterializeSkill { source })?;
        if path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(GitCheckoutError::InvalidSkillPath);
        }
        let unpacked = entry
            .unpack_in(target)
            .map_err(|source| GitCheckoutError::MaterializeSkill { source })?;
        if !unpacked {
            return Err(GitCheckoutError::InvalidSkillPath);
        }
    }
    Ok(())
}

fn wait_for_child(
    child: &mut std::process::Child,
    timeout: Duration,
) -> io::Result<Option<ExitStatus>> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(repository: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("Git must be available for checkout support");
        assert!(status.success(), "Git command failed: {arguments:?}");
    }

    #[test]
    fn rejects_unsafe_references_and_skill_paths_before_git_or_filesystem_access() {
        for reference in ["", "-main", "../main", "feature//branch", "main.lock"] {
            assert!(matches!(
                validate_reference(reference),
                Err(GitCheckoutError::InvalidReference)
            ));
        }
        for path in [
            "",
            "../SKILL.md",
            "/skills/demo/SKILL.md",
            r"skills\demo\SKILL.md",
            "skills/demo/README.md",
        ] {
            assert!(matches!(
                validate_skill_path(path),
                Err(GitCheckoutError::InvalidSkillPath)
            ));
        }
        assert_eq!(
            validate_skill_path("skills/demo/SKILL.md").unwrap(),
            PathBuf::from("skills/demo/SKILL.md")
        );
    }

    #[test]
    fn materializes_only_tracked_skill_content_including_repository_root_skills() {
        let source = tempfile::tempdir().unwrap();
        git(source.path(), &["init"]);
        git(source.path(), &["config", "user.name", "YssSkills Test"]);
        git(
            source.path(),
            &["config", "user.email", "yssskills@example.invalid"],
        );
        fs::write(
            source.path().join("SKILL.md"),
            "---\nname: root\ndescription: root skill\n---\nbody\n",
        )
        .unwrap();
        fs::write(source.path().join("tracked.txt"), "tracked").unwrap();
        fs::write(source.path().join("untracked.txt"), "untracked").unwrap();
        fs::create_dir_all(source.path().join("skills/nested")).unwrap();
        fs::write(
            source.path().join("skills/nested/SKILL.md"),
            "---\nname: nested\ndescription: nested skill\n---\nbody\n",
        )
        .unwrap();
        git(
            source.path(),
            &["add", "SKILL.md", "tracked.txt", "skills/nested/SKILL.md"],
        );
        git(source.path(), &["commit", "-m", "initial"]);

        let checkout_directory = tempfile::tempdir().unwrap();
        clone_repository(
            &source.path().to_string_lossy(),
            None,
            &checkout_directory.path().join("repository"),
            Duration::from_secs(10),
        )
        .unwrap();
        let mut checkout = GitCheckout {
            directory: checkout_directory,
            next_materialization: 0,
        };

        let materialized = checkout.skill_directory("SKILL.md").unwrap();

        assert!(materialized.join("SKILL.md").is_file());
        assert!(materialized.join("tracked.txt").is_file());
        assert!(!materialized.join("untracked.txt").exists());
        assert!(!materialized.join(".git").exists());

        let nested = checkout.skill_directory("skills/nested/SKILL.md").unwrap();
        assert!(nested.join("SKILL.md").is_file());
        assert!(!nested.join("tracked.txt").exists());
    }
}
