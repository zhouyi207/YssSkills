use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use rusqlite::Connection;
use skill_core::SkillId;
use skill_index::{IndexState, SkillIndex, SkillLock};
use tempfile::tempdir;

fn write_skill(path: &Path, name: &str, body: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: test skill\n---\n{body}\n"),
    )
    .unwrap();
}

fn open_index(database: &Path) -> (SkillIndex, bool) {
    let (index, status) = SkillIndex::open(database).unwrap();
    (index, status.needs_rebuild)
}

fn remove_sqlite_files(database: &Path) {
    for path in [
        database.to_path_buf(),
        append_suffix(database, "-wal"),
        append_suffix(database, "-shm"),
    ] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("failed to remove {}: {error}", path.display()),
        }
    }
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

#[test]
fn skill_lock_metadata_is_loaded_by_catalog_directory_name() {
    let root = tempdir().unwrap();
    let lock_path = root.path().join(".agents/.skill-lock.json");
    fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    fs::write(
        &lock_path,
        r#"{
          "version": 3,
          "skills": {
            "brainstorming": {
              "source": "obra/superpowers",
              "sourceType": "github",
              "sourceUrl": "https://github.com/obra/superpowers.git",
              "skillPath": "skills/brainstorming/SKILL.md",
              "skillFolderHash": "881fc4ac82a25e61a58d332426b5673efe060da0",
              "pluginName": "superpowers",
              "ref": "main",
              "installedAt": "2026-07-23T08:25:53.636Z",
              "updatedAt": "2026-08-31T05:57:34.009Z"
            }
          }
        }"#,
    )
    .unwrap();

    let lock = SkillLock::read(&lock_path).unwrap();
    let metadata = lock.skill("brainstorming").unwrap();

    assert_eq!(metadata.source.as_deref(), Some("obra/superpowers"));
    assert_eq!(metadata.source_type.as_deref(), Some("github"));
    assert_eq!(
        metadata.source_url.as_deref(),
        Some("https://github.com/obra/superpowers.git")
    );
    assert_eq!(
        metadata.skill_path.as_deref(),
        Some("skills/brainstorming/SKILL.md")
    );
    assert_eq!(metadata.reference.as_deref(), Some("main"));
    assert!(lock.skill("not-installed").is_none());
}

#[test]
fn missing_skill_lock_is_an_empty_metadata_source() {
    let root = tempdir().unwrap();
    let lock = SkillLock::read(&root.path().join("missing.json")).unwrap();

    assert!(lock.skill("brainstorming").is_none());
}

#[test]
fn deleting_the_database_rebuilds_all_core_index_data_from_filesystem_truth() {
    let root = tempdir().unwrap();
    let skills_root = root.path().join("catalog/skills");
    let database = root.path().join("state/skill-index.sqlite3");
    let valid_path = skills_root.join("valid-skill");
    let invalid_path = skills_root.join("invalid-skill");
    write_skill(&valid_path, "Valid Skill", "first body");
    fs::create_dir_all(&invalid_path).unwrap();
    fs::write(invalid_path.join("SKILL.md"), "not frontmatter").unwrap();
    let cancellation = AtomicBool::new(false);

    let (mut index, needs_rebuild) = open_index(&database);
    assert!(needs_rebuild);
    let first_report = index.rebuild(&skills_root, &cancellation).unwrap();
    assert_eq!(first_report.inserted.len(), 2);
    assert_eq!(first_report.invalid.len(), 1);
    let first = index.snapshot().unwrap();
    assert_eq!(first.state, IndexState::Ready);
    assert_eq!(first.skills.len(), 1);
    assert_eq!(first.diagnostics.len(), 1);
    let expected_hash = first.skills[0].content_hash;
    drop(index);

    let (reopened, status) = SkillIndex::open(&database).unwrap();
    assert!(!status.needs_rebuild);
    assert!(status.recovered_from.is_none());
    assert!(reopened.matches_root(&skills_root).unwrap());
    let other_root = root.path().join("other-catalog/skills");
    fs::create_dir_all(&other_root).unwrap();
    assert!(!reopened.matches_root(&other_root).unwrap());
    drop(reopened);

    remove_sqlite_files(&database);
    let (mut rebuilt, needs_rebuild) = open_index(&database);
    assert!(needs_rebuild);
    rebuilt.rebuild(&skills_root, &cancellation).unwrap();
    let restored = rebuilt.snapshot().unwrap();

    assert_eq!(restored.skills.len(), 1);
    assert_eq!(restored.skills[0].metadata.name(), "Valid Skill");
    assert_eq!(restored.skills[0].content_hash, expected_hash);
    assert_eq!(restored.diagnostics.len(), 1);
    assert!(valid_path.join("SKILL.md").is_file());
    assert!(invalid_path.join("SKILL.md").is_file());
}

#[test]
fn reconcile_skips_unchanged_entries_and_applies_add_update_delete_atomically() {
    let root = tempdir().unwrap();
    let skills_root = root.path().join("catalog/skills");
    let database = root.path().join("skill-index.sqlite3");
    let alpha_path = skills_root.join("alpha");
    write_skill(&alpha_path, "Alpha", "first body");
    let alpha_id = SkillId::from_directory_name(alpha_path.file_name().unwrap());
    let cancellation = AtomicBool::new(false);
    let (mut index, _) = open_index(&database);
    index.rebuild(&skills_root, &cancellation).unwrap();
    let before = index.snapshot().unwrap().skills.remove(0);

    let unchanged = index.reconcile(&skills_root, &cancellation).unwrap();
    assert_eq!(unchanged.unchanged, vec![alpha_id]);
    assert!(unchanged.inserted.is_empty());
    assert!(unchanged.updated.is_empty());
    assert_eq!(
        index.snapshot().unwrap().skills[0].indexed_at_epoch_millis,
        before.indexed_at_epoch_millis
    );

    fs::write(alpha_path.join("reference.txt"), "new supporting content").unwrap();
    let updated = index.reconcile(&skills_root, &cancellation).unwrap();
    assert_eq!(updated.updated, vec![alpha_id]);
    assert_ne!(
        index.snapshot().unwrap().skills[0].content_hash,
        before.content_hash
    );

    let beta_path = skills_root.join("beta");
    write_skill(&beta_path, "Beta", "beta body");
    let beta_id = SkillId::from_directory_name(beta_path.file_name().unwrap());
    fs::remove_dir_all(&alpha_path).unwrap();
    let changed = index.reconcile(&skills_root, &cancellation).unwrap();

    assert_eq!(changed.inserted, vec![beta_id]);
    assert_eq!(changed.removed, vec![alpha_id]);
    let snapshot = index.snapshot().unwrap();
    assert_eq!(snapshot.skills.len(), 1);
    assert_eq!(snapshot.skills[0].id, beta_id);
}

#[test]
fn one_invalid_skill_is_isolated_and_can_be_repaired_incrementally() {
    let root = tempdir().unwrap();
    let skills_root = root.path().join("catalog/skills");
    let database = root.path().join("skill-index.sqlite3");
    let valid_path = skills_root.join("valid");
    let invalid_path = skills_root.join("broken");
    write_skill(&valid_path, "Valid", "body");
    fs::create_dir_all(&invalid_path).unwrap();
    fs::write(invalid_path.join("SKILL.md"), "broken").unwrap();
    let invalid_id = SkillId::from_directory_name(invalid_path.file_name().unwrap());
    let cancellation = AtomicBool::new(false);
    let (mut index, _) = open_index(&database);

    index.rebuild(&skills_root, &cancellation).unwrap();
    let initial = index.snapshot().unwrap();
    assert_eq!(initial.skills.len(), 1);
    assert_eq!(initial.diagnostics.len(), 1);
    assert_eq!(initial.diagnostics[0].skill_id, invalid_id);

    write_skill(&invalid_path, "Repaired", "fixed body");
    let report = index.reconcile(&skills_root, &cancellation).unwrap();
    assert_eq!(report.updated, vec![invalid_id]);
    assert!(report.invalid.is_empty());
    let repaired = index.snapshot().unwrap();
    assert_eq!(repaired.skills.len(), 2);
    assert!(repaired.diagnostics.is_empty());
}

#[test]
fn failed_rebuild_keeps_the_previous_snapshot_and_never_touches_skill_files() {
    let root = tempdir().unwrap();
    let skills_root = root.path().join("catalog/skills");
    let database = root.path().join("skill-index.sqlite3");
    let skill_path = skills_root.join("kept");
    write_skill(&skill_path, "Kept", "body");
    let cancellation = AtomicBool::new(false);
    let (mut index, _) = open_index(&database);
    index.rebuild(&skills_root, &cancellation).unwrap();
    let before = index.snapshot().unwrap();

    let result = index.rebuild(&root.path().join("missing-root"), &cancellation);

    assert!(result.is_err());
    let after = index.snapshot().unwrap();
    assert_eq!(after.skills.len(), 1);
    assert_eq!(after.skills[0].id, before.skills[0].id);
    assert!(skill_path.join("SKILL.md").is_file());
}

#[test]
fn incompatible_schema_is_moved_aside_and_rebuilt_without_modifying_filesystem_truth() {
    let root = tempdir().unwrap();
    let skills_root = root.path().join("catalog/skills");
    let database = root.path().join("skill-index.sqlite3");
    let skill_path = skills_root.join("survivor");
    write_skill(&skill_path, "Survivor", "body");
    let cancellation = AtomicBool::new(false);
    let (mut index, _) = open_index(&database);
    index.rebuild(&skills_root, &cancellation).unwrap();
    drop(index);

    let connection = Connection::open(&database).unwrap();
    connection.pragma_update(None, "user_version", 999).unwrap();
    drop(connection);

    let (mut recovered, status) = SkillIndex::open(&database).unwrap();
    assert!(status.needs_rebuild);
    assert!(status.recovered_from.is_some());
    assert!(skill_path.join("SKILL.md").is_file());

    recovered.rebuild(&skills_root, &cancellation).unwrap();
    assert_eq!(recovered.snapshot().unwrap().skills.len(), 1);
    assert!(skill_path.join("SKILL.md").is_file());
}

#[test]
fn malformed_current_version_schema_is_moved_aside_and_rebuilt_from_filesystem() {
    let root = tempdir().unwrap();
    let skills_root = root.path().join("catalog/skills");
    let database = root.path().join("skill-index.sqlite3");
    let skill_path = skills_root.join("schema-survivor");
    write_skill(&skill_path, "Schema Survivor", "body");
    let cancellation = AtomicBool::new(false);
    let (mut index, _) = open_index(&database);
    index.rebuild(&skills_root, &cancellation).unwrap();
    drop(index);

    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "DROP TABLE skill_index_entries;
             CREATE TABLE skill_index_entries (skill_id TEXT PRIMARY KEY NOT NULL);",
        )
        .unwrap();
    drop(connection);

    let (mut recovered, status) = SkillIndex::open(&database).unwrap();
    assert!(status.needs_rebuild);
    assert!(status.recovered_from.is_some());
    recovered.rebuild(&skills_root, &cancellation).unwrap();

    assert_eq!(recovered.snapshot().unwrap().skills.len(), 1);
    assert!(skill_path.join("SKILL.md").is_file());
}

#[test]
fn malformed_derived_record_is_discarded_without_touching_skill_files() {
    let root = tempdir().unwrap();
    let skills_root = root.path().join("catalog/skills");
    let database = root.path().join("skill-index.sqlite3");
    let skill_path = skills_root.join("record-survivor");
    write_skill(&skill_path, "Record Survivor", "body");
    let cancellation = AtomicBool::new(false);
    let (mut index, _) = open_index(&database);
    index.rebuild(&skills_root, &cancellation).unwrap();
    drop(index);

    let connection = Connection::open(&database).unwrap();
    connection
        .execute(
            "UPDATE skill_index_entries SET content_hash = 'not-a-content-hash'",
            [],
        )
        .unwrap();
    drop(connection);

    let (mut recovered, status) = SkillIndex::open(&database).unwrap();
    assert!(status.needs_rebuild);
    assert!(status.recovered_from.is_some());
    recovered.rebuild(&skills_root, &cancellation).unwrap();

    assert_eq!(recovered.snapshot().unwrap().skills.len(), 1);
    assert!(skill_path.join("SKILL.md").is_file());
}
