use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use skill_local::{
    copy_skill, delete_skill, hash_directory, link_skill, read_skill, ExistingDestination,
    LinkKind, LocalError, LocalOperation,
};
use tempfile::{tempdir, tempdir_in};

fn write_skill(path: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Description for {name}\n---\n\n# {name}\n"),
    )
    .unwrap();
    path.to_path_buf()
}

#[test]
fn copy_skill_preserves_readability_and_hash() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("nested/instructions.md"), "keep this").unwrap();
    let target = root.path().join("targets/copied");
    fs::create_dir_all(target.parent().unwrap()).unwrap();

    let result = copy_skill(&source, &target, ExistingDestination::Reject).unwrap();

    assert_eq!(result.operation, LocalOperation::Copied);
    assert_eq!(result.path, target);
    assert!(read_skill(&target).is_ok());
    assert_eq!(
        hash_directory(&source).unwrap(),
        hash_directory(&target).unwrap()
    );
}

#[test]
fn copy_skill_rejects_existing_target_without_modifying_it() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = write_skill(&root.path().join("target"), "original");
    let before = hash_directory(&target).unwrap();

    let error = copy_skill(&source, &target, ExistingDestination::Reject).unwrap_err();

    assert!(matches!(error, LocalError::DestinationExists { .. }));
    assert_eq!(hash_directory(&target).unwrap(), before);
}

#[test]
fn copy_skill_replaces_only_when_explicitly_requested() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = write_skill(&root.path().join("target"), "original");
    let source_hash = hash_directory(&source).unwrap();

    let result = copy_skill(&source, &target, ExistingDestination::Replace).unwrap();

    assert_eq!(result.operation, LocalOperation::Copied);
    assert_eq!(hash_directory(&target).unwrap(), source_hash);
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
}

#[cfg(unix)]
#[test]
fn copy_skill_keeps_existing_target_when_copy_fails() {
    use std::os::unix::net::UnixListener;

    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let unsupported_entry = source.join("unsupported-entry");
    let _listener = UnixListener::bind(&unsupported_entry).unwrap();
    let target = write_skill(&root.path().join("target"), "original");
    let before = hash_directory(&target).unwrap();

    let error = copy_skill(&source, &target, ExistingDestination::Replace).unwrap_err();

    assert!(matches!(error, LocalError::Io { .. }));
    assert_eq!(hash_directory(&target).unwrap(), before);
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "original"
    );
    assert!(!fs::read_dir(root.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".skill-local-")
    }));
}

#[test]
fn copy_skill_rejects_target_nested_inside_source() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = source.join("nested/copied");
    fs::create_dir_all(target.parent().unwrap()).unwrap();

    let error = copy_skill(&source, &target, ExistingDestination::Reject).unwrap_err();

    assert!(matches!(error, LocalError::PathConflict { .. }));
    assert!(!target.exists());
}

#[test]
fn copy_skill_rejects_source_nested_inside_replace_target() {
    let root = tempdir().unwrap();
    let target = write_skill(&root.path().join("target"), "target");
    let source = write_skill(&target.join("source"), "source");
    let before = hash_directory(&target).unwrap();

    let error = copy_skill(&source, &target, ExistingDestination::Replace).unwrap_err();

    assert!(matches!(error, LocalError::PathConflict { .. }));
    assert_eq!(hash_directory(&target).unwrap(), before);
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "target"
    );
}

#[test]
fn copy_skill_rejects_an_existing_regular_file() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = root.path().join("target-file");
    fs::write(&target, "keep this file").unwrap();
    let before = fs::read(&target).unwrap();

    let error = copy_skill(&source, &target, ExistingDestination::Reject).unwrap_err();

    assert!(matches!(error, LocalError::DestinationExists { .. }));
    assert_eq!(fs::read(&target).unwrap(), before);
}

#[test]
fn copy_skill_rejects_empty_target_without_removing_current_directory() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let current_directory = root.path().join("current");
    fs::create_dir(&current_directory).unwrap();
    let marker = current_directory.join("keep.txt");
    fs::write(&marker, "keep this directory").unwrap();

    let status = Command::new(env::current_exe().unwrap())
        .arg("--exact")
        .arg("copy_skill_empty_target_child")
        .arg("--nocapture")
        .current_dir(&current_directory)
        .env("SKILL_LOCAL_EMPTY_TARGET_SOURCE", &source)
        .status()
        .unwrap();

    assert!(status.success());
    assert!(current_directory.is_dir());
    assert_eq!(fs::read(&marker).unwrap(), b"keep this directory");
}

#[test]
fn copy_skill_empty_target_child() {
    let Some(source) = env::var_os("SKILL_LOCAL_EMPTY_TARGET_SOURCE") else {
        return;
    };

    let error = copy_skill(
        Path::new(&source),
        Path::new(""),
        ExistingDestination::Replace,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        LocalError::InvalidPath { path } if path.as_os_str().is_empty()
    ));
}

fn run_invalid_final_path_child(operation: &str, target: &str) {
    let source_root = tempdir().unwrap();
    let current_root = tempdir().unwrap();
    let source = write_skill(&source_root.path().join("source"), "source");
    let current_directory = current_root.path().join("current");
    fs::create_dir(&current_directory).unwrap();
    let marker = current_directory.join("keep.txt");
    fs::write(&marker, "keep this directory").unwrap();
    let source_before = hash_directory(&source).unwrap();

    let status = Command::new(env::current_exe().unwrap())
        .arg("--exact")
        .arg("invalid_final_path_child")
        .arg("--nocapture")
        .current_dir(&current_directory)
        .env("SKILL_LOCAL_INVALID_FINAL_SOURCE", &source)
        .env("SKILL_LOCAL_INVALID_FINAL_OPERATION", operation)
        .env("SKILL_LOCAL_INVALID_FINAL_TARGET", target)
        .status()
        .unwrap();

    assert!(status.success());
    assert!(current_root.path().is_dir());
    assert!(current_directory.is_dir());
    assert_eq!(fs::read(&marker).unwrap(), b"keep this directory");
    assert_eq!(hash_directory(&source).unwrap(), source_before);
}

#[test]
fn copy_and_link_reject_dot_and_dotdot_without_modifying_source_or_cwd() {
    for target in [".", ".."] {
        run_invalid_final_path_child("copy", target);
        run_invalid_final_path_child("link", target);
    }
}

#[test]
fn invalid_final_path_child() {
    let (Some(source), Some(operation), Some(target)) = (
        env::var_os("SKILL_LOCAL_INVALID_FINAL_SOURCE"),
        env::var("SKILL_LOCAL_INVALID_FINAL_OPERATION").ok(),
        env::var("SKILL_LOCAL_INVALID_FINAL_TARGET").ok(),
    ) else {
        return;
    };

    let source = PathBuf::from(source);
    let error = match operation.as_str() {
        "copy" => copy_skill(&source, Path::new(&target), ExistingDestination::Replace),
        "link" => link_skill(
            &source,
            Path::new(&target),
            LinkKind::Symbolic,
            ExistingDestination::Replace,
        ),
        _ => panic!("unexpected operation: {operation}"),
    }
    .unwrap_err();

    assert!(matches!(error, LocalError::InvalidPath { .. }));
}

#[cfg(unix)]
#[test]
fn copy_skill_rejects_a_nested_symbolic_link_before_writing() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let external = write_skill(&root.path().join("external"), "external");
    symlink(&external, source.join("nested-link")).unwrap();
    let target = root.path().join("targets/copied");
    fs::create_dir_all(target.parent().unwrap()).unwrap();

    let error = copy_skill(&source, &target, ExistingDestination::Reject).unwrap_err();

    assert!(matches!(error, LocalError::NestedLink { .. }));
    assert!(!target.exists());
    assert_eq!(
        read_skill(&external).unwrap().document.metadata().name(),
        "external"
    );
}

#[cfg(windows)]
#[test]
fn copy_skill_rejects_a_nested_junction_before_writing() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let external = write_skill(&root.path().join("external"), "external");
    junction::create(&external, source.join("nested-link")).unwrap();
    let target = root.path().join("targets/copied");
    fs::create_dir_all(target.parent().unwrap()).unwrap();

    let error = copy_skill(&source, &target, ExistingDestination::Reject).unwrap_err();

    assert!(matches!(error, LocalError::NestedLink { .. }));
    assert!(!target.exists());
    assert_eq!(
        read_skill(&external).unwrap().document.metadata().name(),
        "external"
    );
}

#[cfg(unix)]
#[test]
fn copy_skill_replaces_a_final_symbolic_link_component_without_following_target() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let external = write_skill(&root.path().join("external"), "external");
    fs::write(external.join("keep.txt"), "keep this target").unwrap();
    let link = root.path().join("target-link");
    symlink(&external, &link).unwrap();
    let target = link.join(".");

    let result = copy_skill(&source, &target, ExistingDestination::Replace).unwrap();

    assert_eq!(result.operation, LocalOperation::Copied);
    assert_eq!(result.path, target);
    assert_eq!(
        read_skill(&external).unwrap().document.metadata().name(),
        "external"
    );
    assert_eq!(
        fs::read(external.join("keep.txt")).unwrap(),
        b"keep this target"
    );
    assert!(!fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
}

#[cfg(windows)]
#[test]
fn copy_skill_replaces_a_final_junction_component_without_following_target() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let external = write_skill(&root.path().join("external"), "external");
    fs::write(external.join("keep.txt"), "keep this target").unwrap();
    let link = root.path().join("target-link");
    junction::create(&external, &link).unwrap();
    let target = link.join(".");

    let result = copy_skill(&source, &target, ExistingDestination::Replace).unwrap();

    assert_eq!(result.operation, LocalOperation::Copied);
    assert_eq!(result.path, target);
    assert_eq!(
        read_skill(&external).unwrap().document.metadata().name(),
        "external"
    );
    assert_eq!(
        fs::read(external.join("keep.txt")).unwrap(),
        b"keep this target"
    );
    assert!(junction::get_target(&link).is_err());
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
}

#[cfg(windows)]
#[test]
fn copy_skill_replaces_a_dangling_junction_without_following_target() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let missing_target = root.path().join("missing-target");
    let link = root.path().join("target-link");
    junction::create(&missing_target, &link).unwrap();
    let target = link.join(".");

    let result = copy_skill(&source, &target, ExistingDestination::Replace).unwrap();

    assert_eq!(result.operation, LocalOperation::Copied);
    assert_eq!(result.path, target);
    assert!(!missing_target.exists());
    assert!(fs::symlink_metadata(&missing_target).is_err());
    assert!(junction::get_target(&link).is_err());
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
}

#[test]
fn delete_skill_removes_a_regular_skill_directory() {
    let root = tempdir().unwrap();
    let skill = write_skill(&root.path().join("skill"), "skill");
    fs::write(skill.join("extra.md"), "extra").unwrap();

    let result = delete_skill(&skill).unwrap();

    assert_eq!(result.operation, LocalOperation::Deleted);
    assert_eq!(result.path, skill);
    assert!(!skill.exists());
}

#[test]
fn delete_skill_rejects_a_directory_without_a_marker() {
    let root = tempdir().unwrap();
    let path = root.path().join("not-a-skill");
    fs::create_dir_all(&path).unwrap();

    let error = delete_skill(&path).unwrap_err();

    assert!(matches!(error, LocalError::MarkerNotFound { .. }));
    assert!(path.exists());
}

#[cfg(unix)]
#[test]
fn symbolic_link_can_be_read_and_deleted_without_deleting_source() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = root.path().join("linked");

    let result = link_skill(
        &source,
        &target,
        LinkKind::Symbolic,
        ExistingDestination::Reject,
    )
    .unwrap();

    assert_eq!(result.operation, LocalOperation::SymbolicLink);
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
    assert!(target.symlink_metadata().unwrap().file_type().is_symlink());
    delete_skill(&target.join(".")).unwrap();
    assert!(fs::symlink_metadata(&target).is_err());
    assert!(source.exists());
}

#[cfg(windows)]
#[test]
fn symbolic_link_can_be_read_and_deleted_without_deleting_source() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = root.path().join("linked");

    let result = link_skill(
        &source,
        &target,
        LinkKind::Symbolic,
        ExistingDestination::Reject,
    )
    .unwrap();

    assert_eq!(result.operation, LocalOperation::SymbolicLink);
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
    assert!(target.symlink_metadata().unwrap().file_type().is_symlink());
    delete_skill(&target.join(".")).unwrap();
    assert!(fs::symlink_metadata(&target).is_err());
    assert!(source.exists());
}

#[cfg(windows)]
#[test]
fn junction_can_be_read_and_deleted_without_deleting_source() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = root.path().join("junction");

    let result = link_skill(
        &source,
        &target,
        LinkKind::Junction,
        ExistingDestination::Reject,
    )
    .unwrap();

    assert_eq!(result.operation, LocalOperation::Junction);
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
    delete_skill(&target.join(".")).unwrap();
    assert!(fs::symlink_metadata(&target).is_err());
    assert!(junction::get_target(&target).is_err());
    assert!(source.exists());
}

#[cfg(not(windows))]
#[test]
fn junction_is_rejected_on_non_windows_platforms() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = root.path().join("junction");

    let error = link_skill(
        &source,
        &target,
        LinkKind::Junction,
        ExistingDestination::Reject,
    )
    .unwrap_err();

    assert!(matches!(error, LocalError::UnsupportedOperation { .. }));
    assert!(!target.exists());
}

#[cfg(not(windows))]
#[test]
fn link_skill_keeps_existing_target_when_junction_is_unsupported() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = write_skill(&root.path().join("target"), "original");
    let before = hash_directory(&target).unwrap();

    let error = link_skill(
        &source,
        &target,
        LinkKind::Junction,
        ExistingDestination::Replace,
    )
    .unwrap_err();

    assert!(matches!(error, LocalError::UnsupportedOperation { .. }));
    assert_eq!(hash_directory(&target).unwrap(), before);
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "original"
    );
}

#[test]
fn link_skill_rejects_an_existing_regular_target() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = write_skill(&root.path().join("target"), "original");

    let error = link_skill(
        &source,
        &target,
        LinkKind::Symbolic,
        ExistingDestination::Reject,
    )
    .unwrap_err();

    assert!(matches!(error, LocalError::DestinationExists { .. }));
    assert!(!target.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "original"
    );
}

#[test]
fn link_skill_replaces_an_existing_regular_target() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let target = write_skill(&root.path().join("target"), "original");

    let result = link_skill(
        &source,
        &target,
        LinkKind::Symbolic,
        ExistingDestination::Replace,
    )
    .unwrap();

    assert_eq!(result.operation, LocalOperation::SymbolicLink);
    assert!(target.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
}

#[test]
fn link_skill_rejects_a_source_target_conflict() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");

    let error = link_skill(
        &source,
        &source,
        LinkKind::Symbolic,
        ExistingDestination::Replace,
    )
    .unwrap_err();

    assert!(matches!(error, LocalError::PathConflict { .. }));
    assert_eq!(
        read_skill(&source).unwrap().document.metadata().name(),
        "source"
    );
}

#[cfg(unix)]
#[test]
fn link_skill_rejects_replacing_its_source_symbolic_link_entry() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let external = write_skill(&root.path().join("external"), "external");
    let source = root.path().join("source-link");
    symlink(&external, &source).unwrap();

    let error = link_skill(
        &source,
        &source.join("."),
        LinkKind::Symbolic,
        ExistingDestination::Replace,
    )
    .unwrap_err();

    assert!(matches!(error, LocalError::PathConflict { .. }));
    assert!(source.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(
        read_skill(&source).unwrap().document.metadata().name(),
        "external"
    );
}

#[cfg(windows)]
#[test]
fn link_skill_rejects_replacing_its_source_junction_entry() {
    let root = tempdir().unwrap();
    let external = write_skill(&root.path().join("external"), "external");
    let source = root.path().join("source-link");
    junction::create(&external, &source).unwrap();

    let error = link_skill(
        &source,
        &source.join("."),
        LinkKind::Junction,
        ExistingDestination::Replace,
    )
    .unwrap_err();

    assert!(matches!(error, LocalError::PathConflict { .. }));
    assert!(junction::get_target(&source).is_ok());
    assert_eq!(
        read_skill(&source).unwrap().document.metadata().name(),
        "external"
    );
}

#[cfg(unix)]
#[test]
fn link_skill_replaces_a_symbolic_link_without_deleting_external_target() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let external = write_skill(&root.path().join("external"), "external");
    fs::write(external.join("keep.txt"), "keep this target").unwrap();
    let target = root.path().join("target-link");
    symlink(&external, &target).unwrap();

    let result = link_skill(
        &source,
        &target,
        LinkKind::Symbolic,
        ExistingDestination::Replace,
    )
    .unwrap();

    assert_eq!(result.operation, LocalOperation::SymbolicLink);
    assert_eq!(
        fs::read(external.join("keep.txt")).unwrap(),
        b"keep this target"
    );
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
}

#[cfg(windows)]
#[test]
fn link_skill_replaces_a_junction_without_deleting_external_target() {
    let root = tempdir().unwrap();
    let source = write_skill(&root.path().join("source"), "source");
    let external = write_skill(&root.path().join("external"), "external");
    fs::write(external.join("keep.txt"), "keep this target").unwrap();
    let target = root.path().join("target-junction");
    junction::create(&external, &target).unwrap();

    let result = link_skill(
        &source,
        &target,
        LinkKind::Junction,
        ExistingDestination::Replace,
    )
    .unwrap();

    assert_eq!(result.operation, LocalOperation::Junction);
    assert_eq!(
        fs::read(external.join("keep.txt")).unwrap(),
        b"keep this target"
    );
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
}

#[test]
fn link_skill_resolves_a_relative_source_against_the_current_directory() {
    let current_directory = env::current_dir().unwrap();
    let root = tempdir_in(&current_directory).unwrap();
    let absolute_source = write_skill(&root.path().join("source"), "source");
    let relative_source = absolute_source
        .strip_prefix(&current_directory)
        .unwrap()
        .to_path_buf();
    let target = root.path().join("linked");

    let result = link_skill(
        &relative_source,
        &target,
        LinkKind::Symbolic,
        ExistingDestination::Reject,
    )
    .unwrap();

    assert_eq!(result.operation, LocalOperation::SymbolicLink);
    assert_eq!(
        read_skill(&target).unwrap().document.metadata().name(),
        "source"
    );
}

#[test]
fn link_skill_rejects_a_non_skill_source() {
    let root = tempdir().unwrap();
    let source = root.path().join("not-a-skill");
    fs::create_dir_all(&source).unwrap();
    let target = root.path().join("linked");

    let error = link_skill(
        &source,
        &target,
        LinkKind::Symbolic,
        ExistingDestination::Reject,
    )
    .unwrap_err();

    assert!(matches!(error, LocalError::MarkerNotFound { .. }));
    assert!(target.symlink_metadata().is_err());
}

#[cfg(unix)]
#[test]
fn delete_skill_rejects_a_root_symbolic_link_without_a_skill_marker() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let external = root.path().join("external");
    fs::create_dir_all(&external).unwrap();
    let link = root.path().join("link");
    symlink(&external, &link).unwrap();

    let error = delete_skill(&link).unwrap_err();

    assert!(matches!(error, LocalError::MarkerNotFound { .. }));
    assert!(link.symlink_metadata().is_ok());
    assert!(external.exists());
}

#[cfg(windows)]
#[test]
fn delete_skill_rejects_a_root_junction_without_a_skill_marker() {
    let root = tempdir().unwrap();
    let external = root.path().join("external");
    fs::create_dir_all(&external).unwrap();
    let link = root.path().join("link");
    junction::create(&external, &link).unwrap();

    let error = delete_skill(&link).unwrap_err();

    assert!(matches!(error, LocalError::MarkerNotFound { .. }));
    assert!(link.symlink_metadata().is_ok());
    assert!(external.exists());
}
