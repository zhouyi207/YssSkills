use std::{
    fs,
    path::{Path, PathBuf},
};

use skill_core::SkillMarker;
use skill_local::{
    find_skill_marker, hash_directory, read_skill, scan_directory, LocalError, ScanMode,
};
use tempfile::tempdir;

fn write_skill(path: &Path, marker: &str, name: &str) -> PathBuf {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join(marker),
        format!(
            "---\nname: {name}\ndescription: Description for {name}\nversion: \"1.0\"\n---\n\n# {name}\n"
        ),
    )
    .unwrap();
    path.to_path_buf()
}

#[test]
fn flat_scan_only_checks_direct_skill_directories() {
    let root = tempdir().unwrap();
    let direct_zeta = write_skill(&root.path().join("zeta"), "SKILL.md", "zeta");
    let direct_alpha = write_skill(&root.path().join("alpha"), "SKILL.md", "alpha");
    write_skill(&root.path().join("namespace/nested"), "SKILL.md", "nested");
    fs::create_dir_all(root.path().join("not-a-skill")).unwrap();

    let report = scan_directory(root.path(), ScanMode::Flat).unwrap();
    let paths: Vec<_> = report
        .skills
        .iter()
        .map(|skill| skill.path.clone())
        .collect();

    assert_eq!(paths, vec![direct_alpha, direct_zeta]);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn recursive_scan_finds_skills_and_treats_skill_directories_as_leaves() {
    let root = tempdir().unwrap();
    let nested = write_skill(
        &root.path().join("category/skill"),
        "SKILL.md",
        "category-skill",
    );
    write_skill(
        &root.path().join("category/skill/nested"),
        "SKILL.md",
        "nested",
    );
    let other = write_skill(&root.path().join("other"), "SKILL.md", "other");

    write_skill(&root.path().join(".git/hidden"), "SKILL.md", "git-hidden");
    write_skill(&root.path().join(".hub/hidden"), "SKILL.md", "hub-hidden");
    write_skill(
        &root.path().join("node_modules/hidden"),
        "SKILL.md",
        "node-hidden",
    );

    let report = scan_directory(root.path(), ScanMode::Recursive).unwrap();
    let paths: Vec<_> = report
        .skills
        .iter()
        .map(|skill| skill.path.clone())
        .collect();

    assert_eq!(paths, vec![nested, other]);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn marker_detection_prefers_canonical_and_only_matches_direct_regular_files() {
    let root = tempdir().unwrap();
    let skill = root.path().join("skill");
    fs::create_dir_all(skill.join("nested")).unwrap();
    #[cfg(not(windows))]
    fs::write(skill.join("skill.md"), "legacy").unwrap();
    fs::write(skill.join("SKILL.md"), "canonical").unwrap();
    fs::write(skill.join("README.md"), "readme").unwrap();
    fs::write(skill.join("CLAUDE.md"), "claude").unwrap();
    fs::write(skill.join("nested/SKILL.md"), "nested").unwrap();

    let marker = find_skill_marker(&skill).unwrap();
    assert_eq!(
        marker,
        Some((SkillMarker::Canonical, skill.join("SKILL.md")))
    );

    let no_marker = root.path().join("no-marker");
    fs::create_dir_all(no_marker.join("nested")).unwrap();
    fs::write(no_marker.join("README.md"), "not a skill").unwrap();
    fs::write(no_marker.join("CLAUDE.md"), "not a skill").unwrap();
    fs::write(no_marker.join("nested/skill.md"), "not direct").unwrap();
    assert_eq!(find_skill_marker(&no_marker).unwrap(), None);
}

#[test]
fn legacy_marker_is_discoverable_when_canonical_marker_is_absent() {
    let root = tempdir().unwrap();
    let skill = root.path().join("skill");
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("skill.md"), "legacy").unwrap();

    assert_eq!(
        find_skill_marker(&skill).unwrap(),
        Some((SkillMarker::Legacy, skill.join("skill.md")))
    );
}

#[test]
fn malformed_skill_is_reported_as_a_diagnostic_without_hiding_other_skills() {
    let root = tempdir().unwrap();
    let bad = root.path().join("bad");
    fs::create_dir_all(&bad).unwrap();
    fs::write(bad.join("SKILL.md"), "not frontmatter").unwrap();
    let good = write_skill(&root.path().join("good"), "SKILL.md", "good");

    let report = scan_directory(root.path(), ScanMode::Flat).unwrap();

    assert_eq!(report.skills.len(), 1);
    assert_eq!(report.skills[0].path, good);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].path, bad);
    assert!(matches!(
        report.diagnostics[0].error,
        LocalError::Parse { .. }
    ));
}

#[test]
fn read_skill_returns_document_marker_and_content_hash() {
    let root = tempdir().unwrap();
    let skill = write_skill(&root.path().join("readable"), "SKILL.md", "readable");

    let scanned = read_skill(&skill).unwrap();

    assert_eq!(scanned.path, skill);
    assert_eq!(scanned.marker, SkillMarker::Canonical);
    assert_eq!(scanned.marker_path, skill.join("SKILL.md"));
    assert_eq!(scanned.document.metadata().name(), "readable");
    assert_eq!(
        scanned.document.metadata().description(),
        "Description for readable"
    );
    assert_eq!(scanned.document.metadata().version(), Some("1.0"));
    assert_eq!(scanned.document.body(), "\n# readable\n");
    assert_eq!(scanned.content_hash, hash_directory(&skill).unwrap());
}

#[test]
fn read_skill_reports_marker_modified_time_when_metadata_is_available() {
    let root = tempdir().unwrap();
    let skill = write_skill(&root.path().join("with-time"), "SKILL.md", "with-time");

    let scanned = read_skill(&skill).unwrap();

    assert!(scanned.marker_modified_at.is_some());
}

#[test]
fn hash_ignores_generated_and_repository_files() {
    let root = tempdir().unwrap();
    fs::create_dir_all(root.path().join("nested")).unwrap();
    fs::write(root.path().join("content.txt"), "content").unwrap();
    fs::write(root.path().join("nested/kept.txt"), "kept").unwrap();
    let before = hash_directory(root.path()).unwrap();

    fs::create_dir_all(root.path().join(".git/objects")).unwrap();
    fs::write(root.path().join(".git/config"), "git metadata").unwrap();
    fs::write(root.path().join(".DS_Store"), "finder metadata").unwrap();
    fs::write(root.path().join("Thumbs.db"), "thumbnail cache").unwrap();
    fs::write(root.path().join(".gitignore"), "*.tmp").unwrap();
    fs::create_dir_all(root.path().join("__pycache__/nested")).unwrap();
    fs::write(root.path().join("__pycache__/module.pyc"), "bytecode").unwrap();
    fs::write(root.path().join("nested/module.pyc"), "bytecode").unwrap();

    assert_eq!(hash_directory(root.path()).unwrap(), before);
}

#[test]
fn hash_is_sensitive_to_relative_paths_and_file_contents_but_not_absolute_root() {
    let first = tempdir().unwrap();
    fs::create_dir_all(first.path().join("nested")).unwrap();
    fs::write(first.path().join("nested/file.txt"), "same").unwrap();

    let second = tempdir().unwrap();
    fs::create_dir_all(second.path().join("nested")).unwrap();
    fs::write(second.path().join("nested/file.txt"), "same").unwrap();
    assert_eq!(
        hash_directory(first.path()).unwrap(),
        hash_directory(second.path()).unwrap()
    );

    fs::rename(second.path().join("nested"), second.path().join("renamed")).unwrap();
    assert_ne!(
        hash_directory(first.path()).unwrap(),
        hash_directory(second.path()).unwrap()
    );

    fs::write(second.path().join("renamed/file.txt"), "changed").unwrap();
    assert_ne!(
        hash_directory(first.path()).unwrap(),
        hash_directory(second.path()).unwrap()
    );
}

#[test]
fn hash_distinguishes_path_and_content_concatenation_collisions() {
    let first = tempdir().unwrap();
    let second = tempdir().unwrap();
    let first_skill = write_skill(&first.path().join("skill"), "SKILL.md", "same");
    let second_skill = write_skill(&second.path().join("skill"), "SKILL.md", "same");

    fs::write(first_skill.join("a"), "bc").unwrap();
    fs::write(second_skill.join("ab"), "c").unwrap();

    assert_ne!(
        hash_directory(&first_skill).unwrap(),
        hash_directory(&second_skill).unwrap()
    );
}

#[test]
fn empty_directory_hash_is_sha256_of_empty_input() {
    let root = tempdir().unwrap();

    assert_eq!(
        hash_directory(root.path()).unwrap().to_hex(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[cfg(unix)]
#[test]
fn executable_bits_are_part_of_the_hash() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().unwrap();
    let file = root.path().join("script.sh");
    fs::write(&file, "echo skill").unwrap();

    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(&file, permissions).unwrap();
    let non_executable = hash_directory(root.path()).unwrap();

    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&file, permissions).unwrap();
    let executable = hash_directory(root.path()).unwrap();

    assert_ne!(non_executable, executable);
}

#[cfg(unix)]
#[test]
fn hash_through_a_symlinked_root_matches_the_real_directory() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let real = root.path().join("real");
    fs::create_dir_all(&real).unwrap();
    fs::write(real.join("SKILL.md"), "content").unwrap();
    let link = root.path().join("link");
    symlink(&real, &link).unwrap();

    assert_eq!(
        hash_directory(&real).unwrap(),
        hash_directory(&link).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn marker_symlink_is_not_a_skill_marker() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let skill = root.path().join("skill");
    fs::create_dir_all(&skill).unwrap();
    let target = root.path().join("target.md");
    fs::write(&target, "---\nname: target\ndescription: target\n---\n").unwrap();
    symlink(&target, skill.join("SKILL.md")).unwrap();

    assert_eq!(find_skill_marker(&skill).unwrap(), None);
}

#[cfg(unix)]
#[test]
fn recursive_scan_survives_a_directory_symlink_cycle() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let skill = write_skill(
        &root.path().join("category/real-skill"),
        "SKILL.md",
        "real-skill",
    );
    symlink(
        root.path().join("category"),
        root.path().join("category/loop"),
    )
    .unwrap();

    let report = scan_directory(root.path(), ScanMode::Recursive).unwrap();
    let paths: Vec<_> = report
        .skills
        .iter()
        .map(|scanned| scanned.path.clone())
        .collect();

    assert_eq!(paths, vec![skill]);
}

#[cfg(unix)]
#[test]
fn recursive_scan_survives_a_self_referential_directory_symlink() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let skill = write_skill(
        &root.path().join("category/real-skill"),
        "SKILL.md",
        "real-skill",
    );
    symlink("loop", root.path().join("category/loop")).unwrap();

    let report = scan_directory(root.path(), ScanMode::Recursive).unwrap();
    let paths: Vec<_> = report
        .skills
        .iter()
        .map(|scanned| scanned.path.clone())
        .collect();

    assert_eq!(paths, vec![skill]);
}

#[cfg(unix)]
#[test]
fn recursive_scan_prunes_ignored_symlinks_before_resolving_them() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let skill = write_skill(&root.path().join("real-skill"), "SKILL.md", "real-skill");
    symlink(".git", root.path().join(".git")).unwrap();

    let report = scan_directory(root.path(), ScanMode::Recursive).unwrap();
    let paths: Vec<_> = report
        .skills
        .iter()
        .map(|scanned| scanned.path.clone())
        .collect();

    assert_eq!(paths, vec![skill]);
}

#[cfg(unix)]
#[test]
fn hash_keeps_distinct_non_utf8_relative_paths_distinct() {
    use std::os::unix::ffi::OsStrExt;

    let first = tempdir().unwrap();
    let second = tempdir().unwrap();
    fs::write(
        first
            .path()
            .join(std::ffi::OsStr::from_bytes(&[b'f', 0x80])),
        "content",
    )
    .unwrap();
    fs::write(
        second
            .path()
            .join(std::ffi::OsStr::from_bytes(&[b'f', 0x81])),
        "content",
    )
    .unwrap();

    assert_ne!(
        hash_directory(first.path()).unwrap(),
        hash_directory(second.path()).unwrap()
    );
}

#[test]
fn invalid_scan_roots_return_typed_errors() {
    let root = tempdir().unwrap();
    let missing = root.path().join("missing");
    assert!(matches!(
        scan_directory(&missing, ScanMode::Flat),
        Err(LocalError::PathNotFound { .. })
    ));

    let file = root.path().join("file");
    fs::write(&file, "not a directory").unwrap();
    assert!(matches!(
        scan_directory(&file, ScanMode::Flat),
        Err(LocalError::NotDirectory { .. })
    ));
}
