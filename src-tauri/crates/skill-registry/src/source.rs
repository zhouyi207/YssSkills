use reqwest::Url;
use thiserror::Error;

use crate::model::SourceKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSource {
    pub original_input: String,
    pub clone_url: String,
    pub branch: Option<String>,
    pub subpath: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceParseError {
    #[error("remote source input must not be empty")]
    EmptyInput,
    #[error("remote source input must not contain control characters")]
    ContainsControlCharacter,
    #[error("remote source URL is invalid")]
    InvalidUrl,
    #[error("remote source uses unsupported URL scheme '{scheme}'")]
    UnsupportedScheme { scheme: String },
    #[error("local paths and UNC paths are not valid remote sources")]
    LocalPath,
    #[error("remote source URL must not contain a query or fragment")]
    QueryOrFragment,
    #[error("GitHub shorthand must have the form owner/repo")]
    InvalidShorthand,
    #[error("GitHub tree URL has an empty branch")]
    EmptyBranch,
    #[error("GitHub tree URL has an invalid branch")]
    InvalidBranch,
    #[error("repository subpath must be relative and stay inside the repository")]
    InvalidSubpath { subpath: String },
    #[error("GitHub tree path has an ambiguous branch")]
    AmbiguousBranch {
        path: String,
        candidates: Vec<String>,
    },
    #[error("source kind '{kind}' is not supported for source resolution")]
    UnsupportedSourceKind { kind: String },
    #[error("source kind '{expected}' does not match source host '{actual}'")]
    SourceKindMismatch { expected: String, actual: String },
}

/// Parse a Git source without performing any network or filesystem operation.
///
/// Remote URLs use HTTPS or SSH; scp-style SSH inputs are also accepted when
/// their user, host, and repository path are unambiguous. URL passwords and
/// HTTPS userinfo are rejected.
///
/// A GitHub tree URL with a slash-containing branch is initially interpreted
/// using its first path segment as the branch. Call
/// [`parse_git_source_with_branches`] when the caller has an explicit branch
/// list and wants the longest matching branch instead.
pub fn parse_git_source(input: &str) -> Result<GitSource, SourceParseError> {
    parse_git_source_with_branches(input, &[])
}

/// Parse a Git source using a caller-provided list of known branch names.
///
/// The known branch list is only used for GitHub `tree/<path>` URLs. A matching
/// branch must end on a `/` boundary, and the longest matching branch wins. If
/// the list is empty, or no known branch matches, the parser deliberately uses
/// the first path segment as the branch; it never performs an implicit network
/// lookup.
pub fn parse_git_source_with_branches(
    input: &str,
    known_branches: &[String],
) -> Result<GitSource, SourceParseError> {
    validate_input(input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(SourceParseError::EmptyInput);
    }

    if trimmed.contains("?") || trimmed.contains('#') {
        return Err(SourceParseError::QueryOrFragment);
    }

    if is_local_path(trimmed) {
        return Err(SourceParseError::LocalPath);
    }
    // URL parsers may treat backslashes as path separators on special URLs;
    // reject them before parsing so validation and the preserved clone URL agree.
    if trimmed.contains('\\') && trimmed.contains("://") {
        return Err(SourceParseError::InvalidUrl);
    }
    if has_dot_path_segment(trimmed) {
        return Err(SourceParseError::InvalidSubpath {
            subpath: "<remote URL path>".to_owned(),
        });
    }

    // Reject encoded path data before Url parses or normalizes it. In
    // particular, an encoded dot segment must never become a branch/subpath.
    if trimmed.contains('%') && (trimmed.contains("://") || looks_like_scp_style_ssh(trimmed)) {
        return Err(SourceParseError::InvalidUrl);
    }

    if trimmed.contains("://") {
        let url = Url::parse(trimmed).map_err(|_| SourceParseError::InvalidUrl)?;
        let scheme = url.scheme().to_ascii_lowercase();
        if !matches!(scheme.as_str(), "https" | "ssh") {
            return Err(SourceParseError::UnsupportedScheme { scheme });
        }
        if url.host_str().is_none() {
            return Err(SourceParseError::InvalidUrl);
        }
        let has_userinfo = has_url_userinfo(trimmed);
        if (scheme == "https" && has_userinfo)
            || (scheme == "ssh"
                && (url.password().is_some() || (has_userinfo && url.username().is_empty())))
        {
            return Err(SourceParseError::InvalidUrl);
        }

        if scheme == "https" && is_github_host(&url) {
            if let Some((clone_url, tree_path)) = github_tree_path(&url)? {
                let (branch, subpath) = resolve_tree_branch_path(&tree_path, known_branches)?;
                return Ok(GitSource {
                    original_input: input.to_owned(),
                    clone_url,
                    branch: Some(branch),
                    subpath,
                });
            }
        }

        if is_github_host(&url) && !has_valid_github_repository_path(&url) {
            return Err(SourceParseError::InvalidUrl);
        }

        return Ok(GitSource {
            original_input: input.to_owned(),
            clone_url: trimmed.to_owned(),
            branch: None,
            subpath: None,
        });
    }

    if let Some((_, host, path)) = parse_scp_style_ssh(trimmed) {
        if host.eq_ignore_ascii_case("github.com") && !has_valid_scp_repository_path(path) {
            return Err(SourceParseError::InvalidUrl);
        }
        return Ok(GitSource {
            original_input: input.to_owned(),
            clone_url: trimmed.to_owned(),
            branch: None,
            subpath: None,
        });
    }
    if looks_like_scp_style_ssh(trimmed) {
        return Err(SourceParseError::InvalidUrl);
    }

    parse_github_shorthand(input, trimmed)
}

/// Re-resolve a previously parsed Git source with a known branch list.
///
/// The original input is retained by `GitSource`, so this operation remains a
/// pure transformation and does not need to reconstruct a tree URL from the
/// already-split branch and subpath.
pub fn resolve_git_source_branch(
    source: &GitSource,
    known_branches: &[String],
) -> Result<GitSource, SourceParseError> {
    parse_git_source_with_branches(&source.original_input, known_branches)
}

/// Compatibility spelling for callers that prefer a parser-shaped API.
pub fn parse_git_source_resolved(
    input: &str,
    known_branches: &[String],
) -> Result<GitSource, SourceParseError> {
    parse_git_source_with_branches(input, known_branches)
}

/// Split the path after a GitHub `tree/` marker into a branch and repository
/// relative subpath. This function is intentionally network-free.
pub fn resolve_tree_branch_path(
    tree_path: &str,
    known_branches: &[String],
) -> Result<(String, Option<String>), SourceParseError> {
    validate_input(tree_path)?;
    let trimmed = tree_path.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(SourceParseError::EmptyBranch);
    }
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return Err(SourceParseError::InvalidSubpath {
            subpath: tree_path.to_owned(),
        });
    }

    let segments: Vec<&str> = trimmed.split('/').collect();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err(SourceParseError::InvalidSubpath {
            subpath: tree_path.to_owned(),
        });
    }

    let mut matching_branches: Vec<String> = known_branches
        .iter()
        .filter(|branch| is_valid_known_branch(branch))
        .filter(|branch| {
            trimmed == branch.as_str()
                || trimmed
                    .strip_prefix(branch.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
        .map(|branch| branch.to_owned())
        .collect();
    matching_branches.sort();
    matching_branches.dedup();

    let branch = if matching_branches.is_empty() {
        segments
            .first()
            .copied()
            .ok_or(SourceParseError::EmptyBranch)?
            .to_owned()
    } else {
        let longest_length = matching_branches
            .iter()
            .map(String::len)
            .max()
            .ok_or(SourceParseError::EmptyBranch)?;
        let longest: Vec<String> = matching_branches
            .into_iter()
            .filter(|candidate| candidate.len() == longest_length)
            .collect();
        if longest.len() > 1 {
            return Err(SourceParseError::AmbiguousBranch {
                path: tree_path.to_owned(),
                candidates: longest,
            });
        }
        longest
            .into_iter()
            .next()
            .ok_or(SourceParseError::EmptyBranch)?
    };

    validate_branch(&branch)?;
    let remainder = trimmed
        .strip_prefix(&branch)
        .and_then(|rest| rest.strip_prefix('/'));
    let subpath = validate_subpath(remainder)?;

    Ok((branch, subpath))
}

/// Resolve a registry source according to its explicit kind.
///
/// `WellKnown` sources are retained as opaque references because this crate
/// does not materialize or install them. `Unknown` and registry kinds that are
/// not implemented produce a typed error instead of being rewritten as
/// GitHub shorthand.
pub fn parse_source_reference(
    kind: SourceKind,
    raw_input: &str,
    install_url: Option<&str>,
) -> Result<SourceReference, SourceParseError> {
    validate_input(raw_input)?;
    if raw_input.trim().is_empty() {
        return Err(SourceParseError::EmptyInput);
    }
    if install_url.is_some_and(|url| url.chars().any(char::is_control)) {
        return Err(SourceParseError::ContainsControlCharacter);
    }

    match kind {
        SourceKind::GitHub => {
            let git_input = install_url
                .filter(|url| !url.trim().is_empty())
                .unwrap_or(raw_input);
            let git_source = parse_git_source(git_input)?;
            if !is_github_clone_url(&git_source.clone_url) {
                if source_host_label(&git_source.clone_url).eq_ignore_ascii_case("github.com") {
                    return Err(SourceParseError::InvalidUrl);
                }
                return Err(SourceParseError::SourceKindMismatch {
                    expected: SourceKind::GitHub.to_string(),
                    actual: source_host_label(&git_source.clone_url),
                });
            }
            Ok(SourceReference::GitHub(git_source))
        }
        SourceKind::WellKnown => Ok(SourceReference::WellKnown {
            original_input: raw_input.to_owned(),
            install_url: install_url.map(str::to_owned),
        }),
        SourceKind::Unknown => Err(SourceParseError::UnsupportedSourceKind {
            kind: SourceKind::Unknown.to_string(),
        }),
        SourceKind::Other(value) => Err(SourceParseError::UnsupportedSourceKind { kind: value }),
    }
}

/// Borrowing convenience wrapper around [`parse_source_reference`].
pub fn resolve_source_reference(
    kind: &SourceKind,
    raw_input: &str,
    install_url: Option<&str>,
) -> Result<SourceReference, SourceParseError> {
    parse_source_reference(kind.clone(), raw_input, install_url)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceReference {
    GitHub(GitSource),
    WellKnown {
        original_input: String,
        install_url: Option<String>,
    },
}

impl SourceReference {
    pub fn kind(&self) -> SourceKind {
        match self {
            Self::GitHub(_) => SourceKind::GitHub,
            Self::WellKnown { .. } => SourceKind::WellKnown,
        }
    }
}

/// Compatibility alias emphasizing that this is a reference, not a checkout.
pub type RemoteSourceReference = SourceReference;

fn validate_input(input: &str) -> Result<(), SourceParseError> {
    if input.is_empty() {
        return Err(SourceParseError::EmptyInput);
    }
    if input.chars().any(char::is_control) {
        return Err(SourceParseError::ContainsControlCharacter);
    }
    Ok(())
}

fn has_dot_path_segment(value: &str) -> bool {
    let Some(scheme_end) = value.find("://") else {
        return false;
    };
    let authority_and_path = &value[scheme_end + 3..];
    let Some(path_start) = authority_and_path.find('/') else {
        return false;
    };
    authority_and_path[path_start..]
        .split('/')
        .any(|segment| segment == "." || segment == "..")
}

fn has_url_userinfo(value: &str) -> bool {
    let Some((_, authority_and_path)) = value.split_once("://") else {
        return false;
    };
    authority_and_path
        .split('/')
        .next()
        .is_some_and(|authority| authority.contains('@'))
}

fn is_local_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with('/')
        || value.starts_with("\\\\")
        || value.starts_with("//")
        || value == "."
        || value.starts_with("./")
        || value.starts_with(".\\")
        || value == ".."
        || value.starts_with("../")
        || value.starts_with("..\\")
        || value.starts_with("~/")
        || value.starts_with("~\\")
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

fn looks_like_scp_style_ssh(value: &str) -> bool {
    let Some(at) = value.find('@') else {
        return false;
    };
    value[at + 1..].contains(':')
}

fn parse_scp_style_ssh(value: &str) -> Option<(&str, &str, &str)> {
    let at = value.find('@')?;
    if value[at + 1..].contains('@') {
        return None;
    }
    let colon = value[at + 1..].find(':').map(|offset| at + 1 + offset)?;
    let user = &value[..at];
    let host = &value[at + 1..colon];
    let path = &value[colon + 1..];
    if !is_valid_scp_user(user) || !is_valid_scp_host(host) || !is_valid_scp_path(path) {
        return None;
    }
    Some((user, host, path))
}

fn is_valid_scp_user(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|character| {
            character.is_ascii_control()
                || character.is_ascii_whitespace()
                || matches!(character, '@' | ':' | '/' | '\\')
        })
}

fn is_valid_scp_host(value: &str) -> bool {
    !value.is_empty()
        && !value.chars().any(|character| {
            character.is_ascii_control()
                || character.is_ascii_whitespace()
                || matches!(character, '@' | ':' | '/' | '\\')
        })
}

fn is_valid_scp_path(value: &str) -> bool {
    let path = value.strip_prefix('/').unwrap_or(value);
    !path.is_empty()
        && !path.starts_with('/')
        && !value.starts_with('\\')
        && !value.contains('\\')
        && !value.contains('%')
        && !value
            .chars()
            .any(|character| character.is_ascii_control() || matches!(character, ':' | '?' | '#'))
        && !path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
}

fn parse_github_shorthand(input: &str, trimmed: &str) -> Result<GitSource, SourceParseError> {
    if trimmed.contains('\\') || trimmed.chars().any(char::is_whitespace) {
        return Err(SourceParseError::InvalidShorthand);
    }

    let mut segments = trimmed.split('/');
    let owner = segments.next().filter(|segment| !segment.is_empty());
    let repo = segments.next().filter(|segment| !segment.is_empty());
    if owner.is_none() || repo.is_none() || segments.next().is_some() {
        return Err(SourceParseError::InvalidShorthand);
    }

    let owner = owner.ok_or(SourceParseError::InvalidShorthand)?;
    let repo = repo.ok_or(SourceParseError::InvalidShorthand)?;
    let (owner, repo) =
        github_repository_parts(owner, repo).ok_or(SourceParseError::InvalidShorthand)?;

    Ok(GitSource {
        original_input: input.to_owned(),
        clone_url: format!("https://github.com/{owner}/{repo}.git"),
        branch: None,
        subpath: None,
    })
}

fn github_repository_parts<'a>(owner: &'a str, repository: &'a str) -> Option<(&'a str, &'a str)> {
    let repository = repository.strip_suffix(".git").unwrap_or(repository);
    (is_valid_github_repository_segment(owner) && is_valid_github_repository_segment(repository))
        .then_some((owner, repository))
}

fn is_valid_github_repository_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.contains('%')
        && !segment.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(
                    character,
                    ':' | '~' | '^' | '?' | '*' | '[' | ']' | '@' | '/' | '\\'
                )
        })
}

fn is_github_host(url: &Url) -> bool {
    url.host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case("github.com"))
}

fn github_tree_path(url: &Url) -> Result<Option<(String, String)>, SourceParseError> {
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return Err(SourceParseError::InvalidUrl);
    }

    let segments: Vec<&str> = url
        .path_segments()
        .ok_or(SourceParseError::InvalidUrl)?
        .collect();
    if segments.len() < 3 || segments.get(2) != Some(&"tree") {
        return Ok(None);
    }
    if segments.len() < 4 {
        return Err(SourceParseError::EmptyBranch);
    }
    if segments.iter().any(|segment| segment.contains('%')) {
        return Err(SourceParseError::InvalidUrl);
    }

    let (owner, repo) =
        github_repository_parts(segments[0], segments[1]).ok_or(SourceParseError::InvalidUrl)?;

    let tree_path = segments[3..].join("/");
    if tree_path.is_empty() {
        return Err(SourceParseError::EmptyBranch);
    }

    Ok(Some((
        format!("https://github.com/{owner}/{repo}.git"),
        tree_path,
    )))
}

fn is_valid_known_branch(branch: &str) -> bool {
    !branch.is_empty()
        && branch == branch.trim()
        && !branch.contains('\\')
        && !branch.chars().any(char::is_control)
        && !branch.split('/').any(|segment| segment.is_empty())
}

fn validate_branch(branch: &str) -> Result<(), SourceParseError> {
    if branch.is_empty() {
        return Err(SourceParseError::EmptyBranch);
    }
    if !is_valid_git_ref(branch) {
        return Err(SourceParseError::InvalidBranch);
    }
    Ok(())
}

fn is_valid_git_ref(value: &str) -> bool {
    !value.is_empty()
        && value == value.trim()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains("//")
        && !value.contains("..")
        && value != "@"
        && !value.contains("@{")
        && !value.ends_with('.')
        && !value.chars().any(|character| {
            character.is_ascii_control()
                || character.is_ascii_whitespace()
                || matches!(character, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
        })
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && !segment.starts_with('.')
                && !segment.ends_with('.')
                && !segment.ends_with(".lock")
        })
}

fn is_absolute_or_drive_qualified(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with('/')
        || value.starts_with('\\')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

fn is_github_clone_url(value: &str) -> bool {
    if let Ok(url) = Url::parse(value) {
        return matches!(url.scheme(), "https" | "ssh")
            && url
                .host_str()
                .is_some_and(|host| host.eq_ignore_ascii_case("github.com"))
            && url.port().is_none()
            && url.password().is_none()
            && (url.scheme() == "ssh" || url.username().is_empty())
            && has_valid_github_repository_path(&url);
    }

    parse_scp_style_ssh(value).is_some_and(|(_, host, path)| {
        host.eq_ignore_ascii_case("github.com") && has_valid_scp_repository_path(path)
    })
}

fn has_valid_github_repository_path(url: &Url) -> bool {
    let Some(segments) = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
    else {
        return false;
    };
    if segments.len() != 2 {
        return false;
    }
    github_repository_parts(segments[0], segments[1]).is_some()
}

fn has_valid_scp_repository_path(path: &str) -> bool {
    if path.starts_with('/') {
        return false;
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() != 2 {
        return false;
    }
    github_repository_parts(segments[0], segments[1]).is_some() && is_valid_scp_path(path)
}

fn source_host_label(value: &str) -> String {
    if let Ok(url) = Url::parse(value) {
        return url.host_str().unwrap_or("unknown").to_owned();
    }
    scp_host(value).unwrap_or("unknown").to_owned()
}

fn scp_host(value: &str) -> Option<&str> {
    parse_scp_style_ssh(value).map(|(_, host, _)| host)
}

fn validate_subpath(subpath: Option<&str>) -> Result<Option<String>, SourceParseError> {
    let Some(subpath) = subpath else {
        return Ok(None);
    };
    if subpath.is_empty() {
        return Ok(None);
    }
    if is_absolute_or_drive_qualified(subpath)
        || subpath.contains('\\')
        || subpath
            .chars()
            .any(|character| character.is_control() || matches!(character, ':' | '?' | '#'))
        || subpath
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(SourceParseError::InvalidSubpath {
            subpath: subpath.to_owned(),
        });
    }
    Ok(Some(subpath.to_owned()))
}
