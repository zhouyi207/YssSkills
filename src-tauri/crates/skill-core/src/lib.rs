use std::{ffi::OsStr, fmt, path::PathBuf, str::FromStr};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use thiserror::Error;
use uuid::Uuid;

pub const CANONICAL_SKILL_FILE_NAME: &str = "SKILL.md";
pub const LEGACY_SKILL_FILE_NAME: &str = "skill.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillMarker {
    Canonical,
    Legacy,
}

impl SkillMarker {
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Canonical => CANONICAL_SKILL_FILE_NAME,
            Self::Legacy => LEGACY_SKILL_FILE_NAME,
        }
    }
}

pub fn classify_skill_marker(file_name: &str) -> Option<SkillMarker> {
    match file_name {
        CANONICAL_SKILL_FILE_NAME => Some(SkillMarker::Canonical),
        LEGACY_SKILL_FILE_NAME => Some(SkillMarker::Legacy),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillId(Uuid);

impl SkillId {
    const DIRECTORY_NAMESPACE: Uuid = Uuid::from_u128(0x4dc978d0_9654_4f4e_8626_f0dfbe2d48e7);

    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(value: &str) -> Result<Self, SkillIdError> {
        if value.trim().is_empty() {
            return Err(SkillIdError::Empty);
        }

        Uuid::parse_str(value.trim())
            .map(Self)
            .map_err(|_| SkillIdError::InvalidFormat)
    }

    pub fn from_directory_name(name: &OsStr) -> Self {
        #[cfg(unix)]
        let bytes = name.as_bytes().to_vec();

        #[cfg(windows)]
        let bytes = name
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();

        #[cfg(not(any(unix, windows)))]
        let bytes = name.to_string_lossy().as_bytes().to_vec();

        Self(Uuid::new_v5(&Self::DIRECTORY_NAMESPACE, &bytes))
    }

    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SkillId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SkillId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SkillId {
    type Err = SkillIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillSetId(Uuid);

impl SkillSetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(value: &str) -> Result<Self, SkillSetIdError> {
        if value.trim().is_empty() {
            return Err(SkillSetIdError::Empty);
        }

        Uuid::parse_str(value.trim())
            .map(Self)
            .map_err(|_| SkillSetIdError::InvalidFormat)
    }
}

impl Default for SkillSetId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SkillSetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SkillSetId {
    type Err = SkillSetIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SkillSetIdError {
    #[error("skill set id must not be empty")]
    Empty,
    #[error("skill set id must be a valid UUID")]
    InvalidFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SkillIdError {
    #[error("skill id must not be empty")]
    Empty,
    #[error("skill id must be a valid UUID")]
    InvalidFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_hex(value: &str) -> Result<Self, ContentHashError> {
        if value.len() != 64 {
            return Err(ContentHashError::InvalidLength);
        }

        let mut bytes = [0; 32];
        for (slot, pair) in bytes.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
            let Some((&high, &low)) = pair.first().zip(pair.get(1)) else {
                return Err(ContentHashError::InvalidLength);
            };
            let Some(high) = hex_value(high) else {
                return Err(ContentHashError::InvalidHex);
            };
            let Some(low) = hex_value(low) else {
                return Err(ContentHashError::InvalidHex);
            };
            *slot = (high << 4) | low;
        }

        Ok(Self(bytes))
    }

    pub fn to_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut value = String::with_capacity(64);
        for byte in self.0 {
            value.push(HEX[(byte >> 4) as usize] as char);
            value.push(HEX[(byte & 0x0f) as usize] as char);
        }
        value
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ContentHashError {
    #[error("content hash must contain exactly 64 hexadecimal characters")]
    InvalidLength,
    #[error("content hash contains a non-hexadecimal character")]
    InvalidHex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataField {
    Name,
    Description,
    Version,
}

impl fmt::Display for MetadataField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Name => "name",
            Self::Description => "description",
            Self::Version => "version",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SkillValidationError {
    #[error("metadata field '{field}' is missing")]
    MissingField { field: MetadataField },
    #[error("metadata field '{field}' must be a string")]
    NonStringField { field: MetadataField },
    #[error("metadata field '{field}' must not be empty")]
    EmptyField { field: MetadataField },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMetadata {
    name: String,
    description: String,
    version: Option<String>,
}

impl SkillMetadata {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Self, SkillValidationError> {
        Self::new_with_version(name, description, None)
    }

    pub fn new_with_version(
        name: impl Into<String>,
        description: impl Into<String>,
        version: Option<String>,
    ) -> Result<Self, SkillValidationError> {
        let name = normalize_required_field(MetadataField::Name, name.into())?;
        let description = normalize_required_field(MetadataField::Description, description.into())?;
        let version = version
            .map(|value| normalize_required_field(MetadataField::Version, value))
            .transpose()?;

        Ok(Self {
            name,
            description,
            version,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

fn normalize_required_field(
    field: MetadataField,
    value: String,
) -> Result<String, SkillValidationError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(SkillValidationError::EmptyField { field });
    }
    Ok(value.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDocument {
    metadata: SkillMetadata,
    body: String,
}

impl SkillDocument {
    pub fn metadata(&self) -> &SkillMetadata {
        &self.metadata
    }

    pub fn body(&self) -> &str {
        &self.body
    }
}

#[derive(Debug, Error)]
pub enum SkillParseError {
    #[error("skill document is not valid UTF-8")]
    InvalidUtf8 { source: std::str::Utf8Error },
    #[error("skill document must start with YAML frontmatter")]
    MissingFrontmatter,
    #[error("skill document frontmatter is not closed")]
    UnclosedFrontmatter,
    #[error("skill document frontmatter is invalid: {message}")]
    InvalidFrontmatter { message: String },
    #[error("skill metadata is invalid: {0}")]
    Validation(#[from] SkillValidationError),
}

pub fn parse_skill_document(bytes: &[u8]) -> Result<SkillDocument, SkillParseError> {
    let input =
        std::str::from_utf8(bytes).map_err(|source| SkillParseError::InvalidUtf8 { source })?;
    let (frontmatter, body) = split_frontmatter(input)?;
    let metadata = parse_frontmatter(frontmatter)?;

    Ok(SkillDocument {
        metadata,
        body: body.to_owned(),
    })
}

fn split_frontmatter(input: &str) -> Result<(&str, &str), SkillParseError> {
    let (opening, mut cursor) = next_line(input, 0);
    if opening != "---" {
        return Err(SkillParseError::MissingFrontmatter);
    }

    let frontmatter_start = cursor;
    while cursor < input.len() {
        let (line, next_cursor) = next_line(input, cursor);
        if line == "---" {
            return Ok((&input[frontmatter_start..cursor], &input[next_cursor..]));
        }
        cursor = next_cursor;
    }

    Err(SkillParseError::UnclosedFrontmatter)
}

fn next_line(input: &str, start: usize) -> (&str, usize) {
    let remainder = &input[start..];
    let (line_end, next_cursor) = match remainder.find('\n') {
        Some(relative_end) => {
            let line_end = start + relative_end;
            (line_end, line_end + 1)
        }
        None => (input.len(), input.len()),
    };
    let line = &input[start..line_end];
    (line.strip_suffix('\r').unwrap_or(line), next_cursor)
}

fn parse_frontmatter(frontmatter: &str) -> Result<SkillMetadata, SkillParseError> {
    let mapping = if frontmatter.trim().is_empty() {
        serde_yaml::Mapping::new()
    } else {
        let value = serde_yaml::from_str::<serde_yaml::Value>(frontmatter).map_err(|error| {
            SkillParseError::InvalidFrontmatter {
                message: error.to_string(),
            }
        })?;

        let serde_yaml::Value::Mapping(mapping) = value else {
            return Err(SkillParseError::InvalidFrontmatter {
                message: "frontmatter must be a YAML mapping".to_owned(),
            });
        };
        mapping
    };

    let name = required_metadata_string(&mapping, MetadataField::Name, "name")?;
    let description =
        required_metadata_string(&mapping, MetadataField::Description, "description")?;
    let version = optional_metadata_string(&mapping, MetadataField::Version, "version")?;

    SkillMetadata::new_with_version(name, description, version).map_err(Into::into)
}

fn required_metadata_string(
    mapping: &serde_yaml::Mapping,
    field: MetadataField,
    key: &str,
) -> Result<String, SkillParseError> {
    let key = serde_yaml::Value::String(key.to_owned());
    let Some(value) = mapping.get(&key) else {
        return Err(SkillValidationError::MissingField { field }.into());
    };
    let Some(value) = value.as_str() else {
        return Err(SkillValidationError::NonStringField { field }.into());
    };
    Ok(value.to_owned())
}

fn optional_metadata_string(
    mapping: &serde_yaml::Mapping,
    field: MetadataField,
    key: &str,
) -> Result<Option<String>, SkillParseError> {
    let key = serde_yaml::Value::String(key.to_owned());
    let Some(value) = mapping.get(&key) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Err(SkillValidationError::NonStringField { field }.into());
    };
    Ok(Some(value.to_owned()))
}

const WINDOWS_RESERVED_CHARACTERS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const WINDOWS_RESERVED_BASENAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn sanitize_skill_name(name: &str) -> Option<String> {
    let last = name.rsplit(['/', '\\']).next()?;
    if last == "." || last == ".." || last.is_empty() {
        return None;
    }

    let clean: String = last
        .chars()
        .map(|character| {
            if character.is_control() || WINDOWS_RESERVED_CHARACTERS.contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect();
    let trimmed = clean.trim().trim_end_matches('.').trim_end();

    if trimmed.is_empty() {
        return None;
    }

    let basename = trimmed
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if WINDOWS_RESERVED_BASENAMES.contains(&basename.as_str()) {
        Some(format!("_{trimmed}"))
    } else {
        Some(trimmed.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSource {
    Local {
        path: PathBuf,
    },
    Registry {
        registry: String,
        skill: String,
        version: Option<String>,
    },
    Git {
        url: String,
        revision: Option<String>,
        subdirectory: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledSkill {
    pub id: SkillId,
    pub metadata: SkillMetadata,
    pub location: PathBuf,
    pub source: SkillSource,
    pub content_hash: ContentHash,
}
