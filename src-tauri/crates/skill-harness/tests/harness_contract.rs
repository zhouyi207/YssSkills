use std::{fs, path::Path};

use skill_harness::{
    default_harnesses, CustomHarnessDefinition, DetectionStatus, HarnessAdapter, HarnessCategory,
    HarnessEnvironment, HarnessError, HarnessId, HarnessRegistry,
};
use tempfile::tempdir;

fn harness<'a>(harnesses: &'a [HarnessAdapter], id: &str) -> &'a HarnessAdapter {
    harnesses
        .iter()
        .find(|adapter| adapter.id().as_str() == id)
        .expect("built-in harness should exist")
}

fn absolute_test_skills_path() -> String {
    std::env::temp_dir()
        .join("yssskills-custom-skills")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn includes_reference_built_in_harnesses() {
    let harnesses = default_harnesses();

    assert_eq!(harnesses.len(), 53);
    assert_eq!(harness(&harnesses, "codex").display_name(), "Codex");
    assert_eq!(
        harness(&harnesses, "claude_code").display_name(),
        "Claude Code"
    );
    assert_eq!(harness(&harnesses, "cursor").display_name(), "Cursor");
    assert_eq!(
        harness(&harnesses, "gemini_cli").display_name(),
        "Gemini CLI"
    );
    assert_eq!(harness(&harnesses, "opencode").display_name(), "OpenCode");
}

#[test]
fn resolves_home_relative_paths_and_detection_without_scanning_skills() {
    let temp = tempdir().expect("temporary directory");
    let home = temp.path().join("home");
    fs::create_dir_all(home.join(".codex")).expect("codex config directory");
    fs::create_dir_all(home.join(".agents/skills")).expect("shared discovery directory");
    let environment = HarnessEnvironment::new(home.clone(), None);
    let harnesses = default_harnesses();
    let adapter = harness(&harnesses, "codex");

    let locations = adapter
        .resolve_locations(&environment, Some(temp.path()))
        .expect("codex locations");
    let detection = adapter.detect(&environment).expect("codex detection");

    assert_eq!(locations.global_skills_dir, home.join(".codex/skills"));
    assert_eq!(
        locations.project_skills_dir,
        Some(temp.path().join(".codex/skills"))
    );
    assert_eq!(locations.config_dir, Some(home.join(".codex")));
    assert_eq!(
        locations.additional_global_discovery_dirs,
        vec![home.join(".agents/skills")]
    );
    assert_eq!(detection.status, DetectionStatus::Installed);
    assert_eq!(detection.checked_paths, vec![home.join(".codex")]);
}

#[test]
fn built_in_paths_render_with_platform_native_separators() {
    let temp = tempdir().expect("temporary directory");
    let home = temp.path().join("home");
    let environment = HarnessEnvironment::new(home.clone(), None);
    let harnesses = default_harnesses();
    let adapter = harness(&harnesses, "adal");

    let locations = adapter
        .resolve_locations(&environment, Some(temp.path()))
        .expect("adal locations");

    assert_eq!(
        locations.global_skills_dir.to_string_lossy(),
        home.join(".adal").join("skills").to_string_lossy()
    );
    assert_eq!(
        locations
            .project_skills_dir
            .as_deref()
            .expect("adal project skills directory")
            .to_string_lossy(),
        temp.path().join(".adal").join("skills").to_string_lossy()
    );
}

#[test]
fn config_based_harnesses_prefer_existing_platform_config_directory() {
    let temp = tempdir().expect("temporary directory");
    let home = temp.path().join("home");
    let config = temp.path().join("config");
    fs::create_dir_all(config.join("opencode/skills")).expect("opencode config directory");
    let environment = HarnessEnvironment::new(home, Some(config.clone()));
    let harnesses = default_harnesses();
    let adapter = harness(&harnesses, "opencode");

    let locations = adapter
        .resolve_locations(&environment, Some(temp.path()))
        .expect("opencode locations");

    assert_eq!(locations.global_skills_dir, config.join("opencode/skills"));
    assert_eq!(locations.config_dir, Some(config.join("opencode")));
    assert_eq!(
        locations.project_skills_dir,
        Some(temp.path().join(".opencode/skills"))
    );
}

#[test]
fn preserves_asymmetric_project_paths_and_discovery_only_roots() {
    let harnesses = default_harnesses();
    let omp = harness(&harnesses, "omp_agent");
    let opencode = harness(&harnesses, "opencode");
    let pi = harness(&harnesses, "pi");
    let codex = harness(&harnesses, "codex");

    assert_eq!(
        omp.project_relative_skills_path(),
        Some(Path::new(".omp/skills"))
    );
    assert_eq!(
        opencode.project_relative_skills_path(),
        Some(Path::new(".opencode/skills"))
    );
    assert_eq!(
        pi.project_relative_skills_path(),
        Some(Path::new(".pi/skills"))
    );
    assert!(codex
        .additional_global_discovery_paths()
        .any(|path| path == Path::new(".agents/skills")));
}

#[test]
fn exposes_category_and_discovery_capabilities_without_changing_path_semantics() {
    let harnesses = default_harnesses();
    let cursor = harness(&harnesses, "cursor");
    let hermes = harness(&harnesses, "hermes");
    let openclaw = harness(&harnesses, "openclaw");

    assert_eq!(cursor.category(), HarnessCategory::Coding);
    assert!(cursor.capabilities().supports_global_scope);
    assert!(cursor.capabilities().supports_project_scope);
    assert!(!cursor.capabilities().recursive_global_discovery);
    assert_eq!(hermes.category(), HarnessCategory::Lobster);
    assert!(hermes.capabilities().recursive_global_discovery);
    assert_eq!(openclaw.category(), HarnessCategory::Lobster);
}

#[test]
fn custom_adapter_uses_explicit_paths_and_is_explicitly_available() {
    let temp = tempdir().expect("temporary directory");
    let home = temp.path().join("home");
    let global_skills = temp.path().join("custom-skills");
    let environment = HarnessEnvironment::new(home, None);
    let adapter = HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: "custom_agent".to_owned(),
        display_name: "Custom Agent".to_owned(),
        global_skills_path: global_skills.to_string_lossy().into_owned(),
        project_skills_path: Some(".custom/skills".to_owned()),
        config_path: None,
        category: HarnessCategory::Coding,
    })
    .expect("valid custom adapter");

    let locations = adapter
        .resolve_locations(&environment, Some(temp.path()))
        .expect("custom locations");
    let detection = adapter.detect(&environment).expect("custom detection");

    assert!(adapter.is_custom());
    assert_eq!(locations.global_skills_dir, global_skills);
    assert_eq!(
        locations.project_skills_dir,
        Some(temp.path().join(".custom/skills"))
    );
    assert_eq!(locations.config_dir, None);
    assert_eq!(detection.status, DetectionStatus::ExplicitlyConfigured);
    assert!(detection.is_installed());
}

#[test]
fn rejects_unsafe_custom_paths() {
    let relative_global = HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: "custom".to_owned(),
        display_name: "Custom".to_owned(),
        global_skills_path: "relative/skills".to_owned(),
        project_skills_path: None,
        config_path: None,
        category: HarnessCategory::Coding,
    });
    assert!(matches!(
        relative_global,
        Err(HarnessError::GlobalSkillsPathMustBeAbsolute { .. })
    ));

    let parent_project = HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: "custom".to_owned(),
        display_name: "Custom".to_owned(),
        global_skills_path: absolute_test_skills_path(),
        project_skills_path: Some("../outside".to_owned()),
        config_path: None,
        category: HarnessCategory::Coding,
    });
    assert!(matches!(
        parent_project,
        Err(HarnessError::ProjectSkillsPathContainsParent { .. })
    ));
}

#[test]
fn registry_rejects_duplicate_ids_and_finds_registered_adapters() {
    let mut registry = HarnessRegistry::with_builtins();
    let custom = HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: "custom_agent".to_owned(),
        display_name: "Custom Agent".to_owned(),
        global_skills_path: absolute_test_skills_path(),
        project_skills_path: None,
        config_path: None,
        category: HarnessCategory::Coding,
    })
    .expect("valid custom adapter");

    registry.register(custom).expect("unique custom adapter");
    assert_eq!(
        registry
            .find("custom_agent")
            .expect("registered adapter")
            .display_name(),
        "Custom Agent"
    );

    let duplicate = HarnessAdapter::from_custom(CustomHarnessDefinition {
        id: "codex".to_owned(),
        display_name: "Duplicate Codex".to_owned(),
        global_skills_path: absolute_test_skills_path(),
        project_skills_path: None,
        config_path: None,
        category: HarnessCategory::Coding,
    })
    .expect("valid duplicate definition");
    assert!(matches!(
        registry.register(duplicate),
        Err(HarnessError::DuplicateId { .. })
    ));
}

#[test]
fn harness_ids_remain_path_independent_and_validate_keys() {
    let id = HarnessId::new("custom-agent").expect("valid harness id");
    assert_eq!(id.as_str(), "custom-agent");
    assert!(HarnessId::new("").is_err());
    assert!(HarnessId::new("bad/agent").is_err());
}
