use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use skill_local::{LocalError, WatchChange, WatchManager, WatchTarget, WatchTargetKind};
use tempfile::tempdir;

fn target(id: &str, path: &std::path::Path, kind: WatchTargetKind) -> WatchTarget {
    WatchTarget {
        id: id.to_owned(),
        path: path.to_path_buf(),
        kind,
    }
}

fn wait_for_change(manager: &WatchManager, timeout: Duration) -> WatchChange {
    let deadline = Instant::now() + timeout;
    loop {
        match manager.try_recv() {
            Ok(Some(change)) => return change,
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => panic!("watcher did not emit a change before the deadline"),
            Err(error) => panic!("watcher returned an error: {error}"),
        }
    }
}

fn assert_no_change(manager: &WatchManager, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match manager.try_recv() {
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Ok(Some(change)) => panic!("unexpected watcher change: {change:?}"),
            Err(error) => panic!("watcher returned an error: {error}"),
        }
    }
}

#[test]
fn zero_debounce_is_rejected() {
    let error = WatchManager::new(Duration::ZERO).err().unwrap();

    assert!(matches!(error, LocalError::InvalidDebounce));
}

#[test]
fn duplicate_target_ids_are_rejected_before_registration() {
    let root = tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();

    let error = manager
        .replace_targets([
            target("duplicate", &first, WatchTargetKind::Skills),
            target("duplicate", &second, WatchTargetKind::Skills),
        ])
        .unwrap_err();

    assert!(matches!(error, LocalError::DuplicateWatchTarget { .. }));
}

#[test]
fn missing_config_file_is_allowed_when_its_parent_exists() {
    let root = tempdir().unwrap();
    let config = root.path().join("config.toml");
    let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();

    manager
        .replace_targets([target("config", &config, WatchTargetKind::Config)])
        .unwrap();
    assert_eq!(manager.try_recv().unwrap(), None);
}

#[test]
fn missing_config_parent_is_rejected() {
    let root = tempdir().unwrap();
    let parent = root.path().join("missing");
    let config = parent.join("config.toml");
    let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();

    let error = manager
        .replace_targets([target("config", &config, WatchTargetKind::Config)])
        .unwrap_err();

    assert!(matches!(
        error,
        LocalError::PathNotFound { path } if path == parent
    ));
}

#[test]
fn skills_target_must_be_an_existing_directory() {
    let root = tempdir().unwrap();
    let file = root.path().join("skills-file");
    fs::write(&file, "not a directory").unwrap();
    let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();

    let error = manager
        .replace_targets([target("skills", &file, WatchTargetKind::Skills)])
        .unwrap_err();

    assert!(matches!(
        error,
        LocalError::NotDirectory { path } if path == file
    ));
}

#[test]
fn discovery_target_must_be_an_existing_directory() {
    let root = tempdir().unwrap();
    let missing = root.path().join("discovery");
    let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();

    let error = manager
        .replace_targets([target("discovery", &missing, WatchTargetKind::Discovery)])
        .unwrap_err();

    assert!(matches!(
        error,
        LocalError::PathNotFound { path } if path == missing
    ));
}

#[test]
fn config_target_must_not_be_a_directory() {
    let root = tempdir().unwrap();
    let config = root.path().join("config");
    fs::create_dir_all(&config).unwrap();
    let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();

    let error = manager
        .replace_targets([target("config", &config, WatchTargetKind::Config)])
        .unwrap_err();

    assert!(matches!(
        error,
        LocalError::InvalidWatchTarget { id, path, reason }
            if id == "config" && path == config && reason == "config target must not be a directory"
    ));
}

#[test]
fn failed_replacement_keeps_the_existing_manager_usable() {
    let root = tempdir().unwrap();
    let existing = root.path().join("existing");
    fs::create_dir_all(&existing).unwrap();
    let missing_parent = root.path().join("missing");
    let missing_config = missing_parent.join("config.toml");
    let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();
    manager
        .replace_targets([target("skills", &existing, WatchTargetKind::Skills)])
        .unwrap();

    let error = manager
        .replace_targets([target("config", &missing_config, WatchTargetKind::Config)])
        .unwrap_err();

    assert!(matches!(error, LocalError::PathNotFound { path } if path == missing_parent));
    assert_eq!(manager.try_recv().unwrap(), None);
    manager
        .replace_targets([target("skills", &existing, WatchTargetKind::Skills)])
        .unwrap();
}

#[test]
fn shutdown_closes_the_manager() {
    let root = tempdir().unwrap();
    let mut manager = WatchManager::new(Duration::from_millis(20)).unwrap();
    manager
        .replace_targets([target("skills", root.path(), WatchTargetKind::Skills)])
        .unwrap();

    manager.shutdown().unwrap();
    manager.shutdown().unwrap();

    assert!(matches!(manager.try_recv(), Err(LocalError::WatcherClosed)));
}

#[test]
fn skills_target_reports_nested_file_changes() {
    let root = tempdir().unwrap();
    let skills = root.path().join("skills");
    let changed = skills.join("nested/instruction.md");
    fs::create_dir_all(changed.parent().unwrap()).unwrap();
    let mut manager = WatchManager::new(Duration::from_millis(40)).unwrap();
    manager
        .replace_targets([target("skills", &skills, WatchTargetKind::Skills)])
        .unwrap();

    fs::write(&changed, "changed").unwrap();

    let change = wait_for_change(&manager, Duration::from_secs(3));
    assert_eq!(change.target_id, "skills");
    assert_eq!(change.kind, WatchTargetKind::Skills);
    assert!(change.paths.iter().any(|path| path == &changed));
}

#[test]
fn config_target_ignores_siblings_and_reports_the_exact_file() {
    let root = tempdir().unwrap();
    let config = root.path().join("config.toml");
    let sibling = root.path().join("other.toml");
    fs::write(&config, "before").unwrap();
    let mut manager = WatchManager::new(Duration::from_millis(40)).unwrap();
    manager
        .replace_targets([target("config", &config, WatchTargetKind::Config)])
        .unwrap();

    fs::write(&sibling, "ignored").unwrap();
    assert_no_change(&manager, Duration::from_millis(180));
    fs::write(&config, "after").unwrap();

    let change = wait_for_change(&manager, Duration::from_secs(3));
    assert_eq!(change.target_id, "config");
    assert_eq!(change.paths, vec![config]);
}

#[test]
fn discovery_target_reports_only_first_level_changes() {
    let root = tempdir().unwrap();
    let mut manager = WatchManager::new(Duration::from_millis(40)).unwrap();
    manager
        .replace_targets([target("home", root.path(), WatchTargetKind::Discovery)])
        .unwrap();

    let harness = root.path().join(".new-harness");
    fs::create_dir_all(&harness).unwrap();
    let first_change = wait_for_change(&manager, Duration::from_secs(3));
    assert_eq!(first_change.target_id, "home");
    assert!(first_change.paths.iter().any(|path| path == &harness));

    fs::write(harness.join("nested-file"), "not a discovery event").unwrap();
    assert_no_change(&manager, Duration::from_millis(180));
}

#[test]
fn replacement_only_reports_changes_for_new_targets() {
    let root = tempdir().unwrap();
    let old = root.path().join("old");
    let new = root.path().join("new");
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&new).unwrap();
    let mut manager = WatchManager::new(Duration::from_millis(40)).unwrap();

    manager
        .replace_targets([target("old", &old, WatchTargetKind::Skills)])
        .unwrap();
    manager
        .replace_targets([target("new", &new, WatchTargetKind::Skills)])
        .unwrap();

    while manager.try_recv().unwrap().is_some() {}

    fs::write(old.join("old-file"), "ignored").unwrap();
    assert_no_change(&manager, Duration::from_millis(180));

    let changed = new.join("new-file");
    fs::write(&changed, "changed").unwrap();
    let change = wait_for_change(&manager, Duration::from_secs(3));
    assert_eq!(change.target_id, "new");
    assert_eq!(change.kind, WatchTargetKind::Skills);
    assert!(change.paths.iter().any(|path| path.starts_with(&new)));
}
