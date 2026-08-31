use std::{fs, path::Path};

use skill_local::removed_skill_paths;
use tempfile::tempdir;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn reports_removed_user_content_concisely_and_ignores_regenerable_files() {
    let root = tempdir().unwrap();
    let current = root.path().join("current");
    let replacement = root.path().join("replacement");
    write(&current.join("SKILL.md"), "old");
    write(&current.join("notes.txt"), "keep me");
    write(&current.join("assets/a.txt"), "a");
    write(&current.join("assets/b.txt"), "b");
    write(&current.join("cache.pyc"), "generated");
    write(&current.join(".gitignore"), "important");
    write(&replacement.join("SKILL.md"), "new");

    let removed = removed_skill_paths(&current, &replacement).unwrap();

    assert_eq!(
        removed,
        vec![
            Path::new(".gitignore").to_path_buf(),
            Path::new("assets").to_path_buf(),
            Path::new("notes.txt").to_path_buf(),
        ]
    );
}

#[test]
fn matching_paths_are_not_removals_even_when_contents_change() {
    let root = tempdir().unwrap();
    let current = root.path().join("current");
    let replacement = root.path().join("replacement");
    write(&current.join("SKILL.md"), "old");
    write(&replacement.join("SKILL.md"), "new");

    assert!(removed_skill_paths(&current, &replacement)
        .unwrap()
        .is_empty());
}
