use std::{
    cmp::Ordering,
    fmt,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use skill_core::{ContentHash, InstalledSkill, SkillId};
use skill_harness::HarnessId;
use skill_local::{ScanMode, ScannedSkill};
use uuid::Uuid;

use crate::error::WorkspaceError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(Uuid);

impl WorkspaceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceKind {
    Agents,
    Project {
        root: PathBuf,
    },
    Linked {
        root: PathBuf,
        disabled_root: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub kind: WorkspaceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentMode {
    Copy,
    SymbolicLink,
    Junction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRole {
    Primary,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTarget {
    pub workspace_id: WorkspaceId,
    pub harness_id: HarnessId,
    pub path: PathBuf,
    pub role: TargetRole,
    pub scan_mode: ScanMode,
    pub deployment_mode: DeploymentMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRoot {
    pub path: PathBuf,
    pub scan_mode: ScanMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedWorkspaceTarget {
    pub harness_id: HarnessId,
    pub path: PathBuf,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceResolution {
    pub targets: Vec<WorkspaceTarget>,
    pub discovery_roots: Vec<DiscoveryRoot>,
    pub unsupported: Vec<UnsupportedWorkspaceTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeploymentKey {
    pub skill_id: SkillId,
    pub harness_id: HarnessId,
    pub workspace_id: WorkspaceId,
}

impl fmt::Display for DeploymentKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.skill_id, self.harness_id, self.workspace_id
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillVersion {
    pub content_hash: ContentHash,
    pub marker_modified_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CentralSkillSnapshot {
    pub installed: InstalledSkill,
    pub version: SkillVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentBinding {
    pub key: DeploymentKey,
    pub target_path: PathBuf,
    pub deployment_mode: DeploymentMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCandidate {
    pub path: PathBuf,
    pub version: SkillVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NormalizedPathKey {
    components: Vec<PathComponentKey>,
    order: NormalizedOrderKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PathKey {
    normalized: NormalizedPathKey,
    raw: RawPathKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
enum PathComponentKey {
    Prefix(ComponentValue),
    Root,
    Parent,
    Normal(ComponentValue),
}

#[cfg(unix)]
type ComponentValue = Vec<u8>;
#[cfg(windows)]
type ComponentValue = Vec<u32>;
#[cfg(not(any(unix, windows)))]
type ComponentValue = String;

#[cfg(unix)]
type NormalizedOrderKey = Vec<u8>;
#[cfg(windows)]
type NormalizedOrderKey = Vec<u32>;
#[cfg(not(any(unix, windows)))]
type NormalizedOrderKey = String;

#[cfg(unix)]
type RawPathKey = Vec<u8>;
#[cfg(windows)]
type RawPathKey = Vec<u16>;
#[cfg(not(any(unix, windows)))]
type RawPathKey = String;

impl Ord for PathKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.normalized
            .order
            .cmp(&other.normalized.order)
            .then_with(|| self.normalized.components.cmp(&other.normalized.components))
            .then_with(|| self.raw.cmp(&other.raw))
    }
}

impl PartialOrd for PathKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PathKey {
    pub(crate) fn is_strict_descendant_of(&self, root: &Self) -> bool {
        self.normalized.components.len() > root.normalized.components.len()
            && self
                .normalized
                .components
                .starts_with(&root.normalized.components)
    }

    pub(crate) fn is_descendant_or_equal_of(&self, root: &Self) -> bool {
        self.normalized.components.len() >= root.normalized.components.len()
            && self
                .normalized
                .components
                .starts_with(&root.normalized.components)
    }
}

pub(crate) fn path_key(path: &Path) -> PathKey {
    let components: Vec<PathComponentKey> = path
        .components()
        .filter_map(|component| match component {
            Component::CurDir => None,
            Component::Prefix(_) => Some(PathComponentKey::Prefix(component_value(&component))),
            Component::RootDir => Some(PathComponentKey::Root),
            Component::ParentDir => Some(PathComponentKey::Parent),
            Component::Normal(_) => Some(PathComponentKey::Normal(component_value(&component))),
        })
        .collect();

    PathKey {
        normalized: NormalizedPathKey {
            order: normalized_order_key(&components),
            components,
        },
        raw: raw_path_key(path),
    }
}

pub(crate) fn normalized_path_key(path: &Path) -> NormalizedPathKey {
    path_key(path).normalized
}

pub(crate) fn compare_paths(left: &Path, right: &Path) -> Ordering {
    path_key(left).cmp(&path_key(right))
}

#[cfg(unix)]
fn normalized_order_key(components: &[PathComponentKey]) -> NormalizedOrderKey {
    let mut key = Vec::new();
    let mut has_component = false;
    let mut last_was_separator = false;

    for component in components {
        match component {
            PathComponentKey::Prefix(value) | PathComponentKey::Normal(value) => {
                if has_component && !last_was_separator {
                    key.push(b'/');
                }
                key.extend(value);
                has_component = true;
                last_was_separator = false;
            }
            PathComponentKey::Root => {
                key.push(b'/');
                has_component = true;
                last_was_separator = true;
            }
            PathComponentKey::Parent => {
                if has_component && !last_was_separator {
                    key.push(b'/');
                }
                key.extend(b"..");
                has_component = true;
                last_was_separator = false;
            }
        }
    }

    key
}

#[cfg(windows)]
fn normalized_order_key(components: &[PathComponentKey]) -> NormalizedOrderKey {
    let mut key = Vec::new();
    let mut has_component = false;
    let mut last_was_separator = false;

    for component in components {
        match component {
            PathComponentKey::Prefix(value) | PathComponentKey::Normal(value) => {
                if has_component && !last_was_separator {
                    key.push(u32::from(b'/'));
                }
                key.extend(value);
                has_component = true;
                last_was_separator = false;
            }
            PathComponentKey::Root => {
                key.push(u32::from(b'/'));
                has_component = true;
                last_was_separator = true;
            }
            PathComponentKey::Parent => {
                if has_component && !last_was_separator {
                    key.push(u32::from(b'/'));
                }
                key.extend([u32::from(b'.'), u32::from(b'.')]);
                has_component = true;
                last_was_separator = false;
            }
        }
    }

    key
}

#[cfg(not(any(unix, windows)))]
fn normalized_order_key(components: &[PathComponentKey]) -> NormalizedOrderKey {
    let mut key = String::new();
    let mut has_component = false;
    let mut last_was_separator = false;

    for component in components {
        match component {
            PathComponentKey::Prefix(value) | PathComponentKey::Normal(value) => {
                if has_component && !last_was_separator {
                    key.push('/');
                }
                key.push_str(value);
                has_component = true;
                last_was_separator = false;
            }
            PathComponentKey::Root => {
                key.push('/');
                has_component = true;
                last_was_separator = true;
            }
            PathComponentKey::Parent => {
                if has_component && !last_was_separator {
                    key.push('/');
                }
                key.push_str("..");
                has_component = true;
                last_was_separator = false;
            }
        }
    }

    key
}

#[cfg(unix)]
fn component_value(component: &Component<'_>) -> ComponentValue {
    component.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn component_value(component: &Component<'_>) -> ComponentValue {
    let mut value = Vec::new();
    for decoded in char::decode_utf16(component.as_os_str().encode_wide()) {
        match decoded {
            Ok(character) => {
                value.extend(character.to_lowercase().map(|lowered| {
                    if matches!(lowered, '/' | '\\') {
                        u32::from(b'/')
                    } else {
                        u32::from(lowered)
                    }
                }));
            }
            Err(error) => value.push(0x11_0000 + u32::from(error.unpaired_surrogate())),
        }
    }
    value
}

#[cfg(not(any(unix, windows)))]
fn component_value(component: &Component<'_>) -> ComponentValue {
    component.as_os_str().to_string_lossy().into_owned()
}

#[cfg(unix)]
fn raw_path_key(path: &Path) -> RawPathKey {
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn raw_path_key(path: &Path) -> RawPathKey {
    path.as_os_str().encode_wide().collect()
}

#[cfg(not(any(unix, windows)))]
fn raw_path_key(path: &Path) -> RawPathKey {
    path.to_string_lossy().into_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentStatus {
    NotDeployed,
    InSync,
    LocalNewer,
    CenterNewer,
    Missing,
    Unsupported,
    Error,
}

#[derive(Debug)]
pub struct DeploymentObservation {
    pub key: DeploymentKey,
    pub target_path: PathBuf,
    pub role: TargetRole,
    pub center: Option<CentralSkillSnapshot>,
    pub local: Option<SkillVersion>,
    pub status: DeploymentStatus,
}

#[derive(Debug)]
pub struct UnmatchedLocalSkill {
    pub scanned: ScannedSkill,
    pub target: Option<WorkspaceTarget>,
}

#[derive(Debug)]
pub struct WorkspaceDiagnostic {
    pub path: PathBuf,
    pub status: DeploymentStatus,
    pub error: WorkspaceError,
}

#[derive(Debug)]
pub struct WorkspaceReport {
    pub workspace_id: WorkspaceId,
    pub observations: Vec<DeploymentObservation>,
    pub unmatched_local: Vec<UnmatchedLocalSkill>,
    pub diagnostics: Vec<WorkspaceDiagnostic>,
}

#[derive(Debug)]
pub struct ReconcileReport {
    pub workspace_id: WorkspaceId,
    pub imported: Vec<SkillId>,
    pub center_updated: Vec<SkillId>,
    pub propagated: Vec<DeploymentKey>,
    pub final_report: WorkspaceReport,
}

pub fn classify_deployment(
    binding_exists: bool,
    center: Option<&SkillVersion>,
    local: Option<&SkillVersion>,
) -> DeploymentStatus {
    if !binding_exists {
        return DeploymentStatus::NotDeployed;
    }

    let (Some(center), Some(local)) = (center, local) else {
        return DeploymentStatus::Missing;
    };

    if center.content_hash == local.content_hash {
        return DeploymentStatus::InSync;
    }

    match (center.marker_modified_at, local.marker_modified_at) {
        (Some(center_time), Some(local_time)) if local_time > center_time => {
            DeploymentStatus::LocalNewer
        }
        _ => DeploymentStatus::CenterNewer,
    }
}

pub fn choose_newest_local(candidates: &[LocalCandidate], center: &SkillVersion) -> Option<usize> {
    let center_time = center.marker_modified_at?;

    let mut newest: Option<(usize, SystemTime, PathKey, ContentHash)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        let Some(candidate_time) = candidate.version.marker_modified_at else {
            continue;
        };
        if candidate.version.content_hash == center.content_hash || candidate_time <= center_time {
            continue;
        }

        let candidate_path_key = path_key(&candidate.path);
        let should_replace = match &newest {
            None => true,
            Some((_, newest_time, newest_path_key, newest_hash)) => {
                candidate_time > *newest_time
                    || (candidate_time == *newest_time
                        && (candidate_path_key < *newest_path_key
                            || (candidate_path_key == *newest_path_key
                                && candidate.version.content_hash.as_bytes()
                                    < newest_hash.as_bytes())))
            }
        };

        if should_replace {
            newest = Some((
                index,
                candidate_time,
                candidate_path_key,
                candidate.version.content_hash,
            ));
        }
    }

    newest.map(|(index, _, _, _)| index)
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    use skill_core::ContentHash;

    use super::*;

    #[test]
    fn equal_hash_is_in_sync_even_when_times_differ() {
        let center = SkillVersion {
            content_hash: ContentHash::from_bytes([7; 32]),
            marker_modified_at: Some(SystemTime::UNIX_EPOCH),
        };
        let local = SkillVersion {
            content_hash: center.content_hash,
            marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(30)),
        };

        assert_eq!(
            classify_deployment(true, Some(&center), Some(&local)),
            DeploymentStatus::InSync
        );
    }

    #[test]
    fn binding_without_center_snapshot_is_missing() {
        let local = SkillVersion {
            content_hash: ContentHash::from_bytes([9; 32]),
            marker_modified_at: Some(SystemTime::UNIX_EPOCH),
        };

        assert_eq!(
            classify_deployment(true, None, Some(&local)),
            DeploymentStatus::Missing
        );
    }

    #[test]
    fn local_is_newer_only_when_both_times_are_available_and_local_is_later() {
        let center = SkillVersion {
            content_hash: ContentHash::from_bytes([1; 32]),
            marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(10)),
        };
        let local = SkillVersion {
            content_hash: ContentHash::from_bytes([2; 32]),
            marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(20)),
        };

        assert_eq!(
            classify_deployment(true, Some(&center), Some(&local)),
            DeploymentStatus::LocalNewer
        );
    }

    #[test]
    fn center_wins_equal_or_unavailable_time_comparisons() {
        let center = SkillVersion {
            content_hash: ContentHash::from_bytes([3; 32]),
            marker_modified_at: None,
        };
        let local = SkillVersion {
            content_hash: ContentHash::from_bytes([4; 32]),
            marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(20)),
        };

        assert_eq!(
            classify_deployment(true, Some(&center), Some(&local)),
            DeploymentStatus::CenterNewer
        );
    }

    #[test]
    fn newest_local_candidate_wins_and_path_breaks_ties() {
        let center = SkillVersion {
            content_hash: ContentHash::from_bytes([0; 32]),
            marker_modified_at: Some(SystemTime::UNIX_EPOCH),
        };
        let candidates = vec![
            LocalCandidate {
                path: PathBuf::from("zeta/skill"),
                version: SkillVersion {
                    content_hash: ContentHash::from_bytes([1; 32]),
                    marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(5)),
                },
            },
            LocalCandidate {
                path: PathBuf::from("alpha/skill"),
                version: SkillVersion {
                    content_hash: ContentHash::from_bytes([2; 32]),
                    marker_modified_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(5)),
                },
            },
        ];

        assert_eq!(choose_newest_local(&candidates, &center), Some(1));
    }
}
