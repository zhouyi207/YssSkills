use std::path::PathBuf;

use skill_core::{
    classify_skill_marker, parse_skill_document, sanitize_skill_name, ContentHash, InstalledSkill,
    MetadataField, SkillId, SkillMarker, SkillParseError, SkillSetId, SkillSetIdError, SkillSource,
    SkillValidationError,
};

#[test]
fn skill_set_id_is_distinct_and_parseable() {
    let set_id = SkillSetId::new();

    assert_eq!(SkillSetId::parse(&set_id.to_string()).unwrap(), set_id);
    assert_eq!(SkillSetId::parse(""), Err(SkillSetIdError::Empty));
    assert_eq!(
        SkillSetId::parse("not-a-uuid"),
        Err(SkillSetIdError::InvalidFormat)
    );
}

#[test]
fn parses_valid_frontmatter_and_preserves_body() {
    let document = parse_skill_document(
        b"---\nname: demo-skill\ndescription: A useful skill\nversion: 1.2.3\n---\n# Instructions\n\nKeep this body exactly.\n",
    )
    .expect("valid skill document");

    assert_eq!(document.metadata().name(), "demo-skill");
    assert_eq!(document.metadata().description(), "A useful skill");
    assert_eq!(document.metadata().version(), Some("1.2.3"));
    assert_eq!(
        document.body(),
        "# Instructions\n\nKeep this body exactly.\n"
    );
}

#[test]
fn rejects_missing_frontmatter() {
    assert!(matches!(
        parse_skill_document(b"# Not a skill document"),
        Err(SkillParseError::MissingFrontmatter)
    ));
}

#[test]
fn rejects_unclosed_frontmatter() {
    assert!(matches!(
        parse_skill_document(b"---\nname: demo\ndescription: desc\n"),
        Err(SkillParseError::UnclosedFrontmatter)
    ));
}

#[test]
fn rejects_invalid_frontmatter_yaml() {
    assert!(matches!(
        parse_skill_document(b"---\nname: [\ndescription: desc\n---\nbody"),
        Err(SkillParseError::InvalidFrontmatter { .. })
    ));
}

#[test]
fn reports_missing_metadata_fields() {
    let error =
        parse_skill_document(b"---\nname: demo\n---\nbody").expect_err("description is required");

    assert!(matches!(
        error,
        SkillParseError::Validation(SkillValidationError::MissingField {
            field: MetadataField::Description
        })
    ));
}

#[test]
fn reports_empty_metadata_fields() {
    let error = parse_skill_document(b"---\nname: demo\ndescription: '   '\n---\nbody")
        .expect_err("description cannot be empty");

    assert!(matches!(
        error,
        SkillParseError::Validation(SkillValidationError::EmptyField {
            field: MetadataField::Description
        })
    ));
}

#[test]
fn rejects_invalid_utf8() {
    assert!(matches!(
        parse_skill_document(&[b'-', b'-', b'-', b'\n', 0xff]),
        Err(SkillParseError::InvalidUtf8 { .. })
    ));
}

#[test]
fn recognizes_only_canonical_and_legacy_markers() {
    assert_eq!(
        classify_skill_marker("SKILL.md"),
        Some(SkillMarker::Canonical)
    );
    assert_eq!(classify_skill_marker("skill.md"), Some(SkillMarker::Legacy));
    assert_eq!(classify_skill_marker("README.md"), None);
    assert_eq!(classify_skill_marker("readme.md"), None);
    assert_eq!(classify_skill_marker("CLAUDE.md"), None);
}

#[test]
fn normalizes_skill_names_to_safe_single_components() {
    assert_eq!(
        sanitize_skill_name(r"../unsafe:name "),
        Some("unsafe_name".to_owned())
    );
    assert_eq!(sanitize_skill_name("CON.txt"), Some("_CON.txt".to_owned()));
    assert_eq!(sanitize_skill_name(".."), None);
    assert_eq!(sanitize_skill_name("..."), None);
    assert_eq!(sanitize_skill_name("   ...   "), None);
}

#[test]
fn skill_id_is_opaque_and_parseable() {
    let id = SkillId::parse("550e8400-e29b-41d4-a716-446655440000").expect("valid skill id");

    assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    assert!(SkillId::parse("").is_err());
    assert!(SkillId::parse("not-an-id").is_err());
}

#[test]
fn skill_id_is_stable_for_a_catalog_directory_name() {
    let first = SkillId::from_directory_name(std::ffi::OsStr::new("example-skill"));
    let same = SkillId::from_directory_name(std::ffi::OsStr::new("example-skill"));
    let different = SkillId::from_directory_name(std::ffi::OsStr::new("other-skill"));

    assert_eq!(first, same);
    assert_ne!(first, different);
    assert_eq!(SkillId::parse(&first.to_string()).unwrap(), first);
}

#[test]
fn installed_skill_keeps_identity_independent_from_location_and_content() {
    let id = SkillId::new();
    let first_hash =
        ContentHash::from_hex("0000000000000000000000000000000000000000000000000000000000000000")
            .expect("valid hash");
    let second_hash =
        ContentHash::from_hex("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
            .expect("valid hash");
    let first_metadata =
        skill_core::SkillMetadata::new("first", "description").expect("valid metadata");
    let second_metadata =
        skill_core::SkillMetadata::new("renamed", "changed").expect("valid metadata");

    let first = InstalledSkill {
        id,
        metadata: first_metadata,
        location: PathBuf::from("/one"),
        source: SkillSource::Local {
            path: PathBuf::from("/one"),
        },
        content_hash: first_hash,
    };
    let second = InstalledSkill {
        id,
        metadata: second_metadata,
        location: PathBuf::from("/two"),
        source: SkillSource::Registry {
            registry: "skills.sh".to_owned(),
            skill: "renamed".to_owned(),
            version: Some("2".to_owned()),
        },
        content_hash: second_hash,
    };

    assert_eq!(first.id, second.id);
}

#[test]
fn content_hash_round_trips_as_hex() {
    let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let hash = ContentHash::from_hex(hex).expect("valid hash");

    assert_eq!(hash.to_hex(), hex);
    assert!(ContentHash::from_hex("too-short").is_err());
}
