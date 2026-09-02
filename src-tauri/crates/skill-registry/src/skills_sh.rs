use std::{collections::HashMap, io::Read, time::Duration};

use reqwest::{
    blocking::{Client, Response},
    header::RETRY_AFTER,
    Url,
};
use serde_json::{Map, Value};

use crate::{
    error::{
        QueryValidationError, RegistryError, ResponseKind, RetryAfter, TransportKind,
        TransportOperation,
    },
    model::{
        Leaderboard, LeaderboardResult, RegistrySkillId, RemoteSkillSummary, SearchResult,
        SourceKind,
    },
};

pub const DEFAULT_SKILLS_SH_BASE_URL: &str = "https://skills.sh/";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
pub const MIN_QUERY_LENGTH: usize = 2;
pub const MIN_SEARCH_LIMIT: usize = 1;
pub const MAX_SEARCH_LIMIT: usize = 300;

/// A blocking skills.sh client.
///
/// The client does not start or own an async runtime. Callers already running
/// on an async executor should invoke it from their own `spawn_blocking` (or an
/// equivalent dedicated worker) boundary.
#[derive(Clone)]
pub struct SkillsShClient {
    http: Client,
    base_url: Url,
    timeout: Duration,
    max_response_bytes: usize,
}

impl SkillsShClient {
    pub fn new() -> Result<Self, RegistryError> {
        SkillsShClientBuilder::default().build()
    }

    pub fn builder() -> SkillsShClientBuilder {
        SkillsShClientBuilder::default()
    }

    pub fn with_base_url(base_url: &str) -> Result<Self, RegistryError> {
        Self::builder().base_url(base_url).build()
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<SearchResult, RegistryError> {
        let url = self.search_url(query, limit)?;
        let body = self.get_body(&url, TransportOperation::SearchRequest)?;
        parse_search_response(body)
    }

    pub fn leaderboard(
        &self,
        leaderboard: Leaderboard,
    ) -> Result<LeaderboardResult, RegistryError> {
        let url = self.leaderboard_url(leaderboard)?;
        let body = self.get_body(&url, TransportOperation::LeaderboardRequest)?;
        let html = String::from_utf8(body).map_err(|_| RegistryError::InvalidResponse {
            kind: ResponseKind::Leaderboard,
            message: "response body is not valid UTF-8".to_owned(),
        })?;
        let skills = parse_leaderboard_html(&html)?;
        Ok(LeaderboardResult {
            leaderboard,
            skills,
        })
    }

    pub fn search_url(&self, query: &str, limit: usize) -> Result<Url, RegistryError> {
        validate_query(query)?;
        validate_limit(limit)?;

        let mut url = self.endpoint_url("api/search")?;
        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs
                .append_pair("q", query.trim())
                .append_pair("limit", &limit.to_string());
        }
        Ok(url)
    }

    pub fn leaderboard_url(&self, leaderboard: Leaderboard) -> Result<Url, RegistryError> {
        self.endpoint_url(leaderboard.path())
    }

    fn endpoint_url(&self, path: &str) -> Result<Url, RegistryError> {
        if path.is_empty() {
            return Ok(self.base_url.clone());
        }
        self.base_url
            .join(path)
            .map_err(|_| RegistryError::InvalidBaseUrl)
    }

    fn get_body(&self, url: &Url, operation: TransportOperation) -> Result<Vec<u8>, RegistryError> {
        let mut response = self
            .http
            .get(url.clone())
            .send()
            .map_err(|error| map_request_error(error, operation))?;

        if !response.status().is_success() {
            return Err(status_error(&response));
        }

        read_limited_body(&mut response, self.max_response_bytes)
    }
}

impl std::fmt::Debug for SkillsShClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SkillsShClient")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct SkillsShClientBuilder {
    base_url: String,
    timeout: Duration,
    max_response_bytes: usize,
    proxy_url: Option<String>,
    user_agent: String,
}

impl std::fmt::Debug for SkillsShClientBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SkillsShClientBuilder")
            .field("base_url_configured", &!self.base_url.is_empty())
            .field("timeout", &self.timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .field(
                "proxy_configured",
                &self
                    .proxy_url
                    .as_ref()
                    .is_some_and(|value| !value.trim().is_empty()),
            )
            .field("user_agent_configured", &!self.user_agent.trim().is_empty())
            .finish()
    }
}

impl Default for SkillsShClientBuilder {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_SKILLS_SH_BASE_URL.to_owned(),
            timeout: DEFAULT_TIMEOUT,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            proxy_url: None,
            user_agent: "yssskills-skill-registry/0.1".to_owned(),
        }
    }
}

impl SkillsShClientBuilder {
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_response_bytes(mut self, max_response_bytes: usize) -> Self {
        self.max_response_bytes = max_response_bytes;
        self
    }

    pub fn proxy_url(mut self, proxy_url: impl Into<String>) -> Self {
        self.proxy_url = Some(proxy_url.into());
        self
    }

    pub fn proxy(mut self, proxy_url: Option<String>) -> Self {
        self.proxy_url = proxy_url;
        self
    }

    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    pub fn build(self) -> Result<SkillsShClient, RegistryError> {
        if self.timeout.is_zero() {
            return Err(RegistryError::InvalidTimeout);
        }
        if self.max_response_bytes == 0 {
            return Err(RegistryError::InvalidResponseLimit);
        }
        if self.max_response_bytes > DEFAULT_MAX_RESPONSE_BYTES {
            return Err(RegistryError::ResponseLimitTooLarge {
                requested: self.max_response_bytes,
                maximum: DEFAULT_MAX_RESPONSE_BYTES,
            });
        }

        let base_url = normalize_base_url(&self.base_url)?;
        let mut builder = Client::builder()
            .timeout(self.timeout)
            .user_agent(self.user_agent);

        if let Some(proxy_url) = self.proxy_url.filter(|value| !value.trim().is_empty()) {
            let proxy_url = normalize_proxy_url(&proxy_url)?;
            let proxy = reqwest::Proxy::all(proxy_url).map_err(|_| RegistryError::InvalidProxy)?;
            builder = builder.proxy(proxy);
        }

        let http = builder.build().map_err(|_| RegistryError::Transport {
            operation: TransportOperation::BuildClient,
            kind: TransportKind::Other,
        })?;

        Ok(SkillsShClient {
            http,
            base_url,
            timeout: self.timeout,
            max_response_bytes: self.max_response_bytes,
        })
    }
}

/// Compatibility alias for code that treats skills.sh as one registry adapter.
pub type RegistryClient = SkillsShClient;
pub type RegistryClientBuilder = SkillsShClientBuilder;

pub fn parse_search_response(body: impl AsRef<[u8]>) -> Result<SearchResult, RegistryError> {
    let body = body.as_ref();
    if body.len() > DEFAULT_MAX_RESPONSE_BYTES {
        return Err(RegistryError::ResponseTooLarge {
            limit: DEFAULT_MAX_RESPONSE_BYTES,
            observed: Some(body.len() as u64),
        });
    }

    let value: Value =
        serde_json::from_slice(body).map_err(|error| invalid_json(ResponseKind::Search, error))?;
    let array = match &value {
        Value::Array(array) => array,
        Value::Object(object) => {
            validate_empty_error_fields(object, ResponseKind::Search)?;
            let Some(skills) = object.get("skills") else {
                return Err(RegistryError::MissingResponseField {
                    kind: ResponseKind::Search,
                    field: "skills".to_owned(),
                });
            };
            skills
                .as_array()
                .ok_or_else(|| RegistryError::InvalidResponse {
                    kind: ResponseKind::Search,
                    message: "field 'skills' must be an array".to_owned(),
                })?
        }
        _ => {
            return Err(RegistryError::InvalidResponse {
                kind: ResponseKind::Search,
                message: "top-level response must be an array or object envelope".to_owned(),
            });
        }
    };

    Ok(SearchResult {
        skills: deduplicate(parse_skill_array(array, ResponseKind::Search)?),
    })
}

fn validate_empty_error_fields(
    object: &Map<String, Value>,
    kind: ResponseKind,
) -> Result<(), RegistryError> {
    validate_error_fields(object, kind, false)
}

fn validate_rsc_error_fields(
    object: &Map<String, Value>,
    kind: ResponseKind,
) -> Result<(), RegistryError> {
    validate_error_fields(object, kind, true)
}

fn validate_error_fields(
    object: &Map<String, Value>,
    kind: ResponseKind,
    accept_rsc_undefined: bool,
) -> Result<(), RegistryError> {
    for field in ["error", "errors"] {
        let Some(value) = object.get(field) else {
            continue;
        };

        let is_empty_sentinel = match value {
            Value::Null => true,
            Value::String(value) => {
                value.trim().is_empty() || (accept_rsc_undefined && value == "$undefined")
            }
            Value::Array(value) => value.is_empty(),
            Value::Object(_) | Value::Bool(_) | Value::Number(_) => false,
        };
        if is_empty_sentinel {
            continue;
        }

        let message = if matches!(value, Value::Bool(_) | Value::Number(_)) {
            format!("{kind} response envelope field '{field}' has an invalid error value")
        } else {
            format!("{kind} response envelope contains an error")
        };
        return Err(RegistryError::InvalidResponse { kind, message });
    }
    Ok(())
}

/// Parse a skills.sh leaderboard page without performing network access.
///
/// The parser accepts only the explicit skill containers in `__NEXT_DATA__` or
/// JSON payloads carried by `self.__next_f.push` frames. A page with no
/// recognized container is an error rather than an empty successful result.
pub fn parse_leaderboard_html(html: &str) -> Result<Vec<RemoteSkillSummary>, RegistryError> {
    if html.len() > DEFAULT_MAX_RESPONSE_BYTES {
        return Err(RegistryError::ResponseTooLarge {
            limit: DEFAULT_MAX_RESPONSE_BYTES,
            observed: Some(html.len() as u64),
        });
    }

    if let Some(next_data) = extract_next_data(html)? {
        let array = find_next_data_skill_array(&next_data)?;
        return Ok(deduplicate(parse_skill_array(
            array,
            ResponseKind::Leaderboard,
        )?));
    }

    let array =
        extract_rsc_skill_array(html)?.ok_or_else(|| RegistryError::MissingResponseField {
            kind: ResponseKind::Leaderboard,
            field: "initialSkills/skills/items".to_owned(),
        })?;
    Ok(deduplicate(parse_skill_array(
        &array,
        ResponseKind::Leaderboard,
    )?))
}

pub fn parse_leaderboard_result(
    leaderboard: Leaderboard,
    html: &str,
) -> Result<LeaderboardResult, RegistryError> {
    Ok(LeaderboardResult {
        leaderboard,
        skills: parse_leaderboard_html(html)?,
    })
}

fn normalize_proxy_url(value: &str) -> Result<Url, RegistryError> {
    let value = value.trim();
    let url = Url::parse(value).map_err(|_| RegistryError::InvalidProxy)?;
    if !matches!(url.scheme(), "http" | "https")
        || !has_explicit_url_host(value)
        || url.host_str().is_none()
    {
        return Err(RegistryError::InvalidProxy);
    }
    Ok(url)
}

fn has_explicit_url_host(value: &str) -> bool {
    let Some((_, authority_and_path)) = value.split_once("://") else {
        return false;
    };
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    !host.is_empty()
}

fn has_url_userinfo(value: &str) -> bool {
    let Some((_, authority_and_path)) = value.split_once("://") else {
        return false;
    };
    authority_and_path
        .split(['/', '?', '#'])
        .next()
        .is_some_and(|authority| authority.contains('@'))
}

fn normalize_base_url(value: &str) -> Result<Url, RegistryError> {
    if value.contains('?') || value.contains('#') {
        return Err(RegistryError::BaseUrlQueryOrFragment);
    }
    if value.contains('%') {
        return Err(RegistryError::InvalidBaseUrl);
    }

    let mut url = Url::parse(value).map_err(|_| RegistryError::InvalidBaseUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(RegistryError::UnsupportedBaseUrlScheme);
    }
    if !has_explicit_url_host(value) || url.host_str().is_none() {
        return Err(RegistryError::InvalidBaseUrl);
    }
    if has_url_userinfo(value) || url.password().is_some() {
        return Err(RegistryError::InvalidBaseUrl);
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(RegistryError::BaseUrlQueryOrFragment);
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

fn validate_query(query: &str) -> Result<(), RegistryError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(RegistryError::InvalidQuery {
            reason: QueryValidationError::Empty,
        });
    }
    if query.chars().any(char::is_control) {
        return Err(RegistryError::InvalidQuery {
            reason: QueryValidationError::ContainsControlCharacter,
        });
    }
    if query.chars().count() < MIN_QUERY_LENGTH {
        return Err(RegistryError::InvalidQuery {
            reason: QueryValidationError::TooShort {
                minimum: MIN_QUERY_LENGTH,
            },
        });
    }
    Ok(())
}

fn validate_limit(limit: usize) -> Result<(), RegistryError> {
    if !(MIN_SEARCH_LIMIT..=MAX_SEARCH_LIMIT).contains(&limit) {
        return Err(RegistryError::InvalidLimit {
            limit,
            min: MIN_SEARCH_LIMIT,
            max: MAX_SEARCH_LIMIT,
        });
    }
    Ok(())
}

fn map_request_error(error: reqwest::Error, operation: TransportOperation) -> RegistryError {
    let kind = classify_request_error(&error);
    if error.is_timeout() {
        RegistryError::Timeout { operation, kind }
    } else {
        RegistryError::Transport { operation, kind }
    }
}

fn classify_request_error(error: &reqwest::Error) -> TransportKind {
    if error.is_connect() {
        TransportKind::Connect
    } else if error.is_request() {
        TransportKind::Request
    } else if error.is_body() || error.is_decode() {
        TransportKind::ResponseBody
    } else {
        TransportKind::Other
    }
}

fn status_error(response: &Response) -> RegistryError {
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(RetryAfter::parse);

    match status {
        401 => RegistryError::AuthenticationRequired {
            status,
            retry_after,
        },
        429 => RegistryError::RateLimited {
            status,
            retry_after,
        },
        _ => RegistryError::HttpStatus {
            status,
            retry_after,
        },
    }
}

fn read_limited_body(response: &mut Response, limit: usize) -> Result<Vec<u8>, RegistryError> {
    if limit == 0 {
        return Err(RegistryError::InvalidResponseLimit);
    }
    if let Some(content_length) = response.content_length() {
        if content_length > limit as u64 {
            return Err(RegistryError::ResponseTooLarge {
                limit,
                observed: Some(content_length),
            });
        }
    }

    let mut body = Vec::new();
    let mut chunk = vec![0_u8; limit.min(8192)];
    loop {
        let read = response.read(&mut chunk).map_err(|error| {
            let operation = TransportOperation::ReadResponseBody;
            let kind = TransportKind::ResponseBody;
            if error.kind() == std::io::ErrorKind::TimedOut {
                RegistryError::Timeout { operation, kind }
            } else {
                RegistryError::Transport { operation, kind }
            }
        })?;
        if read == 0 {
            break;
        }
        if body.len().saturating_add(read) > limit {
            return Err(RegistryError::ResponseTooLarge {
                limit,
                observed: Some(body.len() as u64 + read as u64),
            });
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(body)
}

fn invalid_json(kind: ResponseKind, error: serde_json::Error) -> RegistryError {
    RegistryError::InvalidResponse {
        kind,
        message: format!("invalid JSON ({error})"),
    }
}

fn parse_skill_array(
    array: &[Value],
    kind: ResponseKind,
) -> Result<Vec<RemoteSkillSummary>, RegistryError> {
    array
        .iter()
        .enumerate()
        .map(|(index, value)| parse_skill_value(value, kind, index))
        .collect()
}

fn parse_skill_value(
    value: &Value,
    kind: ResponseKind,
    index: usize,
) -> Result<RemoteSkillSummary, RegistryError> {
    let object = value
        .as_object()
        .ok_or_else(|| RegistryError::InvalidResponse {
            kind,
            message: format!("skills[{index}] must be an object"),
        })?;

    let source = required_string(object, &["source"], kind, index)?;
    let skill_id = required_string(object, &["skillId", "skill_id", "id"], kind, index)?;
    let id =
        RegistrySkillId::new(source, skill_id).map_err(|error| RegistryError::InvalidResponse {
            kind,
            message: format!("skills[{index}] has an invalid identity ({error})"),
        })?;

    let fallback_name = id.skill_id.clone();
    let name = match object.get("name") {
        None | Some(Value::Null) => fallback_name,
        Some(Value::String(name)) if name.trim().is_empty() => fallback_name,
        Some(Value::String(name)) => name.trim().to_owned(),
        Some(_) => {
            return Err(RegistryError::InvalidResponse {
                kind,
                message: format!("skills[{index}].name must be a string"),
            });
        }
    };

    let installs = optional_u64(object, &["installs"], kind, index)?.unwrap_or(0);
    let source_kind = optional_string(
        object,
        &["source_kind", "sourceKind", "sourceType"],
        kind,
        index,
    )?
    .map(|value| SourceKind::parse(&value));
    let install_url = optional_string(object, &["install_url", "installUrl"], kind, index)?;
    let is_official = optional_bool(
        object,
        &["is_official", "isOfficial", "official"],
        kind,
        index,
    )?;
    let skills_sh_url = parse_skills_sh_url(object, &id, kind, index)?;
    let rank = optional_u64(object, &["rank", "position"], kind, index)?;

    Ok(RemoteSkillSummary {
        id,
        name,
        installs,
        source_kind,
        install_url,
        is_official,
        skills_sh_url,
        rank,
    })
}

fn required_string(
    object: &Map<String, Value>,
    fields: &[&'static str],
    kind: ResponseKind,
    index: usize,
) -> Result<String, RegistryError> {
    let Some((field, value)) = first_field(object, fields) else {
        return Err(RegistryError::MissingResponseField {
            kind,
            field: format!("skills[{index}].{}", fields[0]),
        });
    };
    let Some(value) = value.as_str() else {
        return Err(RegistryError::InvalidResponse {
            kind,
            message: format!("skills[{index}].{field} must be a string"),
        });
    };
    if value.trim().is_empty() {
        return Err(RegistryError::MissingResponseField {
            kind,
            field: format!("skills[{index}].{field}"),
        });
    }
    Ok(value.trim().to_owned())
}

fn optional_string(
    object: &Map<String, Value>,
    fields: &[&'static str],
    kind: ResponseKind,
    index: usize,
) -> Result<Option<String>, RegistryError> {
    let Some((field, value)) = first_field(object, fields) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(value) = value.as_str() else {
        return Err(RegistryError::InvalidResponse {
            kind,
            message: format!("skills[{index}].{field} must be a string"),
        });
    };
    let value = value.trim();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value.to_owned()))
    }
}

fn optional_bool(
    object: &Map<String, Value>,
    fields: &[&'static str],
    kind: ResponseKind,
    index: usize,
) -> Result<Option<bool>, RegistryError> {
    let Some((field, value)) = first_field(object, fields) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_bool()
        .ok_or_else(|| RegistryError::InvalidResponse {
            kind,
            message: format!("skills[{index}].{field} must be a boolean"),
        })
        .map(Some)
}

fn optional_u64(
    object: &Map<String, Value>,
    fields: &[&'static str],
    kind: ResponseKind,
    index: usize,
) -> Result<Option<u64>, RegistryError> {
    let Some((field, value)) = first_field(object, fields) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    if let Some(number) = value.as_u64() {
        return Ok(Some(number));
    }
    if let Some(string) = value.as_str() {
        return string.trim().parse::<u64>().map(Some).map_err(|_| {
            RegistryError::InvalidResponse {
                kind,
                message: format!("skills[{index}].{field} must be a non-negative integer"),
            }
        });
    }
    Err(RegistryError::InvalidResponse {
        kind,
        message: format!("skills[{index}].{field} must be a non-negative integer"),
    })
}

fn parse_skills_sh_url(
    object: &Map<String, Value>,
    id: &RegistrySkillId,
    kind: ResponseKind,
    index: usize,
) -> Result<Option<String>, RegistryError> {
    if let Some(url) = optional_string(
        object,
        &["skills_sh_url", "skillsShUrl", "skills_url", "skillsUrl"],
        kind,
        index,
    )? {
        if is_valid_skills_sh_url(&url) {
            return Ok(Some(url));
        }
        return Err(RegistryError::InvalidResponse {
            kind,
            message: format!("skills[{index}] has an invalid skills.sh URL"),
        });
    }

    if let Some(url) = object.get("url") {
        if url.is_null() {
            return Ok(Some(derived_skills_sh_url(id, kind, index)?));
        }
        let Some(url) = url.as_str() else {
            return Err(RegistryError::InvalidResponse {
                kind,
                message: format!("skills[{index}].url must be a string"),
            });
        };
        let url = url.trim();
        if is_valid_skills_sh_url(url) {
            return Ok(Some(url.to_owned()));
        }
    }

    Ok(Some(derived_skills_sh_url(id, kind, index)?))
}

fn derived_skills_sh_url(
    id: &RegistrySkillId,
    kind: ResponseKind,
    index: usize,
) -> Result<String, RegistryError> {
    let mut url =
        Url::parse(DEFAULT_SKILLS_SH_BASE_URL).map_err(|_| RegistryError::InvalidResponse {
            kind,
            message: format!("skills[{index}] has an invalid skills.sh URL base"),
        })?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| RegistryError::InvalidResponse {
                kind,
                message: format!("skills[{index}] has an invalid skills.sh URL path"),
            })?;
        for source_segment in id.source.split('/') {
            segments.push(source_segment);
        }
        segments.push(&id.skill_id);
    }
    Ok(url.to_string())
}

fn is_valid_skills_sh_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("skills.sh"))
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}

fn first_field<'a>(
    object: &'a Map<String, Value>,
    fields: &[&'static str],
) -> Option<(&'static str, &'a Value)> {
    fields
        .iter()
        .find_map(|field| object.get(*field).map(|value| (*field, value)))
}

fn deduplicate(skills: Vec<RemoteSkillSummary>) -> Vec<RemoteSkillSummary> {
    let mut positions = HashMap::new();
    let mut result = Vec::with_capacity(skills.len());

    for skill in skills {
        let key = skill.id.clone();
        if let Some(position) = positions.get(&key).copied() {
            merge_summary(&mut result[position], skill);
        } else {
            positions.insert(key, result.len());
            result.push(skill);
        }
    }
    result
}

fn merge_summary(existing: &mut RemoteSkillSummary, incoming: RemoteSkillSummary) {
    if existing.name == existing.id.skill_id && incoming.name != incoming.id.skill_id {
        existing.name = incoming.name;
    }
    if existing.installs == 0 && incoming.installs != 0 {
        existing.installs = incoming.installs;
    }
    if existing.source_kind.is_none() {
        existing.source_kind = incoming.source_kind;
    }
    if existing.install_url.is_none() {
        existing.install_url = incoming.install_url;
    }
    if existing.is_official.is_none() {
        existing.is_official = incoming.is_official;
    }
    if existing.skills_sh_url.is_none() {
        existing.skills_sh_url = incoming.skills_sh_url;
    }
    if existing.rank.is_none() {
        existing.rank = incoming.rank;
    }
}

fn extract_next_data(html: &str) -> Result<Option<Value>, RegistryError> {
    let mut cursor = 0_usize;
    while let Some(script) = next_script_element(html, &mut cursor) {
        if !script.is_next_data {
            continue;
        }
        let content_end = script
            .close_start
            .ok_or_else(|| RegistryError::InvalidResponse {
                kind: ResponseKind::Leaderboard,
                message: "__NEXT_DATA__ script is not closed".to_owned(),
            })?;
        let payload = &html[script.content_start..content_end];
        return serde_json::from_str(payload)
            .map(Some)
            .map_err(|error| invalid_json(ResponseKind::Leaderboard, error));
    }
    Ok(None)
}

fn find_next_data_skill_array(value: &Value) -> Result<&[Value], RegistryError> {
    let object = value
        .as_object()
        .ok_or_else(|| RegistryError::InvalidResponse {
            kind: ResponseKind::Leaderboard,
            message: "__NEXT_DATA__ payload must be an object".to_owned(),
        })?;
    validate_empty_error_fields(object, ResponseKind::Leaderboard)?;

    let props = object
        .get("props")
        .ok_or_else(|| RegistryError::MissingResponseField {
            kind: ResponseKind::Leaderboard,
            field: "props.pageProps".to_owned(),
        })?
        .as_object()
        .ok_or_else(|| RegistryError::InvalidResponse {
            kind: ResponseKind::Leaderboard,
            message: "__NEXT_DATA__.props must be an object".to_owned(),
        })?;
    validate_empty_error_fields(props, ResponseKind::Leaderboard)?;

    let page_props = props
        .get("pageProps")
        .ok_or_else(|| RegistryError::MissingResponseField {
            kind: ResponseKind::Leaderboard,
            field: "props.pageProps".to_owned(),
        })?
        .as_object()
        .ok_or_else(|| RegistryError::InvalidResponse {
            kind: ResponseKind::Leaderboard,
            message: "__NEXT_DATA__.props.pageProps must be an object".to_owned(),
        })?;
    validate_empty_error_fields(page_props, ResponseKind::Leaderboard)?;

    find_explicit_skill_array(page_props)?.ok_or_else(|| RegistryError::MissingResponseField {
        kind: ResponseKind::Leaderboard,
        field: "initialSkills/skills/items".to_owned(),
    })
}

fn find_rsc_skill_array(value: &Value) -> Result<Option<&[Value]>, RegistryError> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    validate_rsc_error_fields(object, ResponseKind::Leaderboard)?;

    let Some(props_value) = object.get("props") else {
        return Ok(None);
    };
    let props = props_value
        .as_object()
        .ok_or_else(|| RegistryError::InvalidResponse {
            kind: ResponseKind::Leaderboard,
            message: "RSC props must be an object".to_owned(),
        })?;
    validate_rsc_error_fields(props, ResponseKind::Leaderboard)?;

    let Some(page_props_value) = props.get("pageProps") else {
        return Ok(None);
    };
    let page_props =
        page_props_value
            .as_object()
            .ok_or_else(|| RegistryError::InvalidResponse {
                kind: ResponseKind::Leaderboard,
                message: "RSC props.pageProps must be an object".to_owned(),
            })?;
    validate_rsc_error_fields(page_props, ResponseKind::Leaderboard)?;

    find_explicit_skill_array(page_props)
}

fn find_explicit_skill_array(
    page_props: &Map<String, Value>,
) -> Result<Option<&[Value]>, RegistryError> {
    let mut selected = None;
    for field in ["initialSkills", "skills", "items"] {
        let Some(value) = page_props.get(field) else {
            continue;
        };
        let array = value
            .as_array()
            .ok_or_else(|| RegistryError::InvalidResponse {
                kind: ResponseKind::Leaderboard,
                message: format!("leaderboard pageProps field '{field}' must be an array"),
            })?;
        if selected.is_none() {
            selected = Some(array.as_slice());
        }
    }
    Ok(selected)
}

fn extract_rsc_skill_array(html: &str) -> Result<Option<Vec<Value>>, RegistryError> {
    const MARKER: &str = "self.__next_f.push";
    let mut cursor = 0_usize;

    while let Some(relative_start) = html[cursor..].find(MARKER) {
        let marker_start = cursor + relative_start;
        let marker_end = marker_start + MARKER.len();
        if !is_javascript_call_marker(html, marker_start, MARKER) {
            cursor = marker_end;
            continue;
        }

        let parenthesis = skip_json_whitespace(html, marker_end);
        let value_start = skip_json_whitespace(html, parenthesis + 1);
        if html.as_bytes().get(parenthesis) != Some(&b'(')
            || html.as_bytes().get(value_start) != Some(&b'[')
        {
            cursor = marker_end;
            continue;
        }
        let segment = extract_balanced_segment(html, value_start, b'[', b']').ok_or_else(|| {
            RegistryError::InvalidResponse {
                kind: ResponseKind::Leaderboard,
                message: "RSC frame is not a valid JSON array".to_owned(),
            }
        })?;
        let call_end = skip_json_whitespace(html, value_start + segment.len());
        if html.as_bytes().get(call_end) != Some(&b')') {
            cursor = value_start.saturating_add(segment.len());
            continue;
        }
        let frame = serde_json::from_str::<Value>(segment)
            .map_err(|error| invalid_json(ResponseKind::Leaderboard, error))?;

        if let Some(payload) = rsc_frame_payload(&frame) {
            for record in parse_rsc_payload(payload)? {
                if let Some(array) = find_rsc_record_skill_array(&record)? {
                    return Ok(Some(array.to_owned()));
                }
            }
        }
        cursor = value_start.saturating_add(segment.len());
    }

    Ok(None)
}

fn rsc_frame_payload(frame: &Value) -> Option<&str> {
    frame.as_array()?.get(1)?.as_str()
}

const MAX_RSC_STRING_DEPTH: usize = 32;

fn parse_rsc_payload(payload: &str) -> Result<Vec<Value>, RegistryError> {
    parse_rsc_payload_at_depth(payload, 0)
}

fn parse_rsc_payload_at_depth(payload: &str, depth: usize) -> Result<Vec<Value>, RegistryError> {
    let payload = payload.trim();
    match serde_json::from_str::<Value>(payload) {
        Ok(Value::String(nested)) => {
            if depth >= MAX_RSC_STRING_DEPTH {
                return Err(RegistryError::InvalidResponse {
                    kind: ResponseKind::Leaderboard,
                    message: "RSC payload string nesting exceeds the maximum depth".to_owned(),
                });
            }
            parse_rsc_payload_at_depth(&nested, depth + 1)
        }
        Ok(value) => Ok(vec![value]),
        Err(_) => Ok(parse_rsc_record_stream(payload)),
    }
}

fn parse_rsc_record_stream(payload: &str) -> Vec<Value> {
    let mut records = Vec::new();
    let mut cursor = 0_usize;

    while cursor < payload.len() {
        let record_start = skip_json_whitespace(payload, cursor);
        if record_start >= payload.len() {
            break;
        }
        let line_end = next_line_start(payload, record_start);
        let Some((value, value_end)) = parse_rsc_record(payload, record_start) else {
            cursor = line_end;
            continue;
        };
        let record_end = next_line_start(payload, value_end);
        if payload[value_end..record_end].trim().is_empty() {
            records.push(value);
        }
        cursor = record_end;
    }

    records
}

fn parse_rsc_record(payload: &str, start: usize) -> Option<(Value, usize)> {
    let bytes = payload.as_bytes();
    let mut position = start;
    while bytes.get(position).is_some_and(u8::is_ascii_hexdigit) {
        position += 1;
    }
    if position == start || bytes.get(position) != Some(&b':') {
        return None;
    }

    let value_start = skip_json_whitespace(payload, position + 1);
    let first = bytes.get(value_start).copied()?;
    match first {
        b'{' => {
            let segment = extract_balanced_segment(payload, value_start, b'{', b'}')?;
            serde_json::from_str(segment)
                .ok()
                .map(|value| (value, value_start + segment.len()))
        }
        b'[' => {
            let segment = extract_balanced_segment(payload, value_start, b'[', b']')?;
            serde_json::from_str(segment)
                .ok()
                .map(|value| (value, value_start + segment.len()))
        }
        b'"' => {
            let segment = extract_json_string(payload, value_start)?;
            serde_json::from_str(segment)
                .ok()
                .map(|value| (value, value_start + segment.len()))
        }
        _ => {
            let line_end = payload[value_start..]
                .find('\n')
                .map_or(payload.len(), |offset| value_start + offset);
            let value_text = payload[value_start..line_end].trim();
            serde_json::from_str(value_text)
                .ok()
                .map(|value| (value, line_end))
        }
    }
}

fn find_rsc_record_skill_array(value: &Value) -> Result<Option<&[Value]>, RegistryError> {
    if let Some(tuple) = value.as_array() {
        return find_rsc_tuple_skill_array(tuple);
    }
    if value
        .as_object()
        .is_some_and(|object| object.contains_key("props"))
    {
        return find_rsc_skill_array(value);
    }
    Ok(None)
}

fn find_rsc_tuple_skill_array(tuple: &[Value]) -> Result<Option<&[Value]>, RegistryError> {
    if tuple.len() < 4 || tuple.first().and_then(Value::as_str) != Some("$") {
        return Ok(None);
    }
    let Some(props) = tuple.get(3).and_then(Value::as_object) else {
        return Ok(None);
    };
    find_rsc_props_skill_array(props)
}

fn find_rsc_props_skill_array(
    props: &Map<String, Value>,
) -> Result<Option<&[Value]>, RegistryError> {
    validate_rsc_error_fields(props, ResponseKind::Leaderboard)?;
    if let Some(array) = find_explicit_skill_array(props)? {
        return Ok(Some(array));
    }

    let Some(page_props_value) = props.get("pageProps") else {
        return Ok(None);
    };
    let page_props =
        page_props_value
            .as_object()
            .ok_or_else(|| RegistryError::InvalidResponse {
                kind: ResponseKind::Leaderboard,
                message: "RSC props.pageProps must be an object".to_owned(),
            })?;
    validate_rsc_error_fields(page_props, ResponseKind::Leaderboard)?;
    find_explicit_skill_array(page_props)
}

fn is_javascript_call_marker(html: &str, marker_start: usize, marker: &str) -> bool {
    let Some(script_start) = script_content_start(html, marker_start) else {
        return false;
    };
    let bytes = html.as_bytes();
    let mut position = script_start;
    let mut state = JavaScriptLexicalState::Code;
    let mut escaped = false;
    let mut can_start_regex = true;
    let mut pending_control_paren = false;
    let mut previous_was_dot = false;
    let mut paren_contexts = Vec::new();

    while position < marker_start {
        let byte = bytes[position];
        match state {
            JavaScriptLexicalState::Code => {
                if byte == b'/' && bytes.get(position + 1) == Some(&b'/') {
                    state = JavaScriptLexicalState::LineComment;
                    position += 2;
                } else if byte == b'/' && bytes.get(position + 1) == Some(&b'*') {
                    state = JavaScriptLexicalState::BlockComment;
                    position += 2;
                } else if byte == b'<' && bytes.get(position..position + 4) == Some(b"<!--") {
                    state = JavaScriptLexicalState::LineComment;
                    position += 4;
                } else if bytes.get(position..position + 3) == Some(b"-->") {
                    // JavaScript keeps the HTML close-comment marker as a
                    // legacy line comment in script bodies.
                    state = JavaScriptLexicalState::LineComment;
                    position += 3;
                } else if matches!(byte, b'\'' | b'"' | b'`') {
                    state = match byte {
                        b'\'' => JavaScriptLexicalState::SingleQuote,
                        b'"' => JavaScriptLexicalState::DoubleQuote,
                        _ => JavaScriptLexicalState::Template,
                    };
                    escaped = false;
                    can_start_regex = false;
                    pending_control_paren = false;
                    previous_was_dot = false;
                    position += 1;
                } else if byte == b'/' {
                    if can_start_regex {
                        state = JavaScriptLexicalState::RegularExpression {
                            in_character_class: false,
                        };
                        escaped = false;
                    } else {
                        can_start_regex = true;
                        pending_control_paren = false;
                    }
                    previous_was_dot = false;
                    position += 1;
                } else if byte.is_ascii_whitespace() {
                    position += 1;
                } else if is_javascript_identifier_start(byte) {
                    let token_start = position;
                    position += 1;
                    while bytes
                        .get(position)
                        .is_some_and(|byte| is_javascript_identifier_continue(*byte))
                    {
                        position += 1;
                    }
                    let token = &bytes[token_start..position];
                    can_start_regex = javascript_keyword_allows_regex(token);
                    pending_control_paren =
                        !previous_was_dot && javascript_control_paren_keyword(token);
                    previous_was_dot = false;
                } else if byte.is_ascii_digit() {
                    position += 1;
                    while bytes
                        .get(position)
                        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'.')
                    {
                        position += 1;
                    }
                    can_start_regex = false;
                    pending_control_paren = false;
                    previous_was_dot = false;
                } else {
                    match byte {
                        b'(' => {
                            paren_contexts.push(pending_control_paren);
                            can_start_regex = true;
                            pending_control_paren = false;
                            previous_was_dot = false;
                        }
                        b')' => {
                            can_start_regex = paren_contexts.pop().unwrap_or(false);
                            pending_control_paren = false;
                            previous_was_dot = false;
                        }
                        b'[' | b'{' | b',' | b';' | b':' | b'?' | b'=' | b'!' | b'~' | b'+'
                        | b'-' | b'*' | b'%' | b'&' | b'|' | b'^' | b'<' | b'>' => {
                            can_start_regex = true;
                            pending_control_paren = false;
                            previous_was_dot = false;
                        }
                        b']' | b'}' => {
                            // A closing array/object can be followed by a
                            // regular-expression literal (`{} /.../`).
                            can_start_regex = true;
                            pending_control_paren = false;
                            previous_was_dot = false;
                        }
                        b'.' => {
                            can_start_regex = false;
                            pending_control_paren = false;
                            previous_was_dot = true;
                        }
                        _ => {
                            can_start_regex = true;
                            pending_control_paren = false;
                            previous_was_dot = false;
                        }
                    }
                    position += 1;
                }
            }
            JavaScriptLexicalState::SingleQuote
            | JavaScriptLexicalState::DoubleQuote
            | JavaScriptLexicalState::Template => {
                let quote = if state == JavaScriptLexicalState::SingleQuote {
                    b'\''
                } else if state == JavaScriptLexicalState::DoubleQuote {
                    b'"'
                } else {
                    b'`'
                };
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    state = JavaScriptLexicalState::Code;
                    can_start_regex = false;
                }
                position += 1;
            }
            JavaScriptLexicalState::RegularExpression { in_character_class } => {
                let mut next_in_character_class = in_character_class;
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'[' {
                    next_in_character_class = true;
                } else if byte == b']' {
                    next_in_character_class = false;
                } else if byte == b'/' && !in_character_class {
                    state = JavaScriptLexicalState::Code;
                    can_start_regex = false;
                    position += 1;
                    continue;
                }
                state = JavaScriptLexicalState::RegularExpression {
                    in_character_class: next_in_character_class,
                };
                position += 1;
            }
            JavaScriptLexicalState::LineComment => {
                if matches!(byte, b'\r' | b'\n') {
                    state = JavaScriptLexicalState::Code;
                }
                position += 1;
            }
            JavaScriptLexicalState::BlockComment => {
                if byte == b'*' && bytes.get(position + 1) == Some(&b'/') {
                    state = JavaScriptLexicalState::Code;
                    position += 2;
                } else {
                    position += 1;
                }
            }
        }
    }

    if state != JavaScriptLexicalState::Code
        || !html[marker_start..].starts_with(marker)
        || marker_start > 0
            && html[..marker_start]
                .chars()
                .next_back()
                .is_some_and(|character| {
                    !character.is_ascii()
                        || character.is_alphanumeric()
                        || matches!(character, '_' | '$' | '.')
                })
    {
        return false;
    }

    let call_start = skip_json_whitespace(html, marker_start + marker.len());
    bytes.get(call_start) == Some(&b'(')
}

fn is_javascript_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

fn is_javascript_identifier_continue(byte: u8) -> bool {
    is_javascript_identifier_start(byte) || byte.is_ascii_digit()
}

fn javascript_keyword_allows_regex(token: &[u8]) -> bool {
    token == b"case"
        || token == b"default"
        || token == b"delete"
        || token == b"do"
        || token == b"else"
        || token == b"in"
        || token == b"instanceof"
        || token == b"new"
        || token == b"of"
        || token == b"return"
        || token == b"throw"
        || token == b"typeof"
        || token == b"void"
        || token == b"yield"
        || token == b"await"
}

fn javascript_control_paren_keyword(token: &[u8]) -> bool {
    token == b"catch"
        || token == b"for"
        || token == b"if"
        || token == b"switch"
        || token == b"while"
        || token == b"with"
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JavaScriptLexicalState {
    Code,
    SingleQuote,
    DoubleQuote,
    Template,
    RegularExpression { in_character_class: bool },
    LineComment,
    BlockComment,
}

fn script_content_start(html: &str, position: usize) -> Option<usize> {
    let (content_start, content_end) = script_bounds_at(html, position)?;
    (position >= content_start && position < content_end).then_some(content_start)
}

fn script_bounds_at(html: &str, position: usize) -> Option<(usize, usize)> {
    let mut cursor = 0_usize;
    while let Some(script) = next_script_element(html, &mut cursor) {
        if position >= script.content_start && position < script.content_end {
            return Some((script.content_start, script.content_end));
        }
    }
    None
}

#[derive(Clone, Copy)]
struct ScriptElement {
    content_start: usize,
    content_end: usize,
    close_start: Option<usize>,
    is_next_data: bool,
}

fn next_script_element(html: &str, cursor: &mut usize) -> Option<ScriptElement> {
    let bytes = html.as_bytes();
    while *cursor < bytes.len() {
        let relative_start = html[*cursor..].find('<')?;
        let open_start = *cursor + relative_start;

        if bytes.get(open_start..open_start + 4) == Some(b"<!--") {
            *cursor = html[open_start + 4..]
                .find("-->")
                .map_or(bytes.len(), |offset| open_start + 4 + offset + 3);
            continue;
        }

        if is_script_open_tag_at(bytes, open_start) {
            let script_name_end = open_start + 1 + b"script".len();
            let open_end = find_html_tag_end(bytes, open_start)?;
            let content_start = open_end + 1;
            let close = find_script_close(bytes, content_start);
            let (content_end, close_start, next_cursor) = close.map_or(
                (bytes.len(), None, bytes.len()),
                |(close_start, close_end)| (close_start, Some(close_start), close_end),
            );
            let is_next_data = has_next_data_id(bytes, script_name_end, open_end);
            *cursor = next_cursor;
            return Some(ScriptElement {
                content_start,
                content_end,
                close_start,
                is_next_data,
            });
        }

        if let Some(raw_text_tag) = non_script_raw_text_tag_at(bytes, open_start) {
            let tag_end = find_html_tag_end(bytes, open_start)?;
            let content_start = tag_end + 1;
            *cursor = if raw_text_tag == b"plaintext" {
                bytes.len()
            } else {
                find_html_tag_close(bytes, content_start, raw_text_tag).unwrap_or(bytes.len())
            };
            continue;
        }

        if is_html_tag_start(bytes, open_start) {
            let tag_end = find_html_tag_end(bytes, open_start)?;
            *cursor = tag_end + 1;
        } else {
            *cursor = open_start + 1;
        }
    }
    None
}

fn is_script_open_tag_at(bytes: &[u8], position: usize) -> bool {
    bytes
        .get(position..position + b"<script".len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(b"<script"))
        && bytes
            .get(position + b"<script".len())
            .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(byte, b'>' | b'/'))
}

fn non_script_raw_text_tag_at(bytes: &[u8], position: usize) -> Option<&'static [u8]> {
    [
        b"textarea".as_slice(),
        b"style".as_slice(),
        b"title".as_slice(),
        b"xmp".as_slice(),
        b"iframe".as_slice(),
        b"noembed".as_slice(),
        b"noframes".as_slice(),
        b"noscript".as_slice(),
        b"plaintext".as_slice(),
    ]
    .into_iter()
    .find(|tag| is_named_open_tag_at(bytes, position, tag))
}

fn is_named_open_tag_at(bytes: &[u8], position: usize, name: &[u8]) -> bool {
    let name_start = position + 1;
    bytes.get(position) == Some(&b'<')
        && bytes
            .get(name_start..name_start + name.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
        && bytes
            .get(name_start + name.len())
            .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(byte, b'>' | b'/'))
}

fn find_html_tag_close(bytes: &[u8], mut position: usize, name: &[u8]) -> Option<usize> {
    while position < bytes.len() {
        let relative_start = bytes[position..].iter().position(|byte| *byte == b'<')?;
        let close_start = position + relative_start;
        let name_start = close_start + 2;
        if bytes.get(close_start..name_start) == Some(b"</")
            && bytes
                .get(name_start..name_start + name.len())
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            && bytes
                .get(name_start + name.len())
                .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'>')
        {
            return find_html_tag_end(bytes, close_start).map(|end| end + 1);
        }
        position = close_start + 1;
    }
    None
}

fn is_html_tag_start(bytes: &[u8], position: usize) -> bool {
    bytes
        .get(position + 1)
        .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'/' | b'!' | b'?'))
}

fn find_html_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut quote = None;
    let mut position = start + 1;
    while position < bytes.len() {
        let byte = bytes[position];
        match quote {
            Some(expected) if byte == expected => quote = None,
            Some(_) => {}
            None if matches!(byte, b'\'' | b'"') => quote = Some(byte),
            None if byte == b'>' => return Some(position),
            None => {}
        }
        position += 1;
    }
    None
}

fn has_next_data_id(bytes: &[u8], mut position: usize, tag_end: usize) -> bool {
    while position < tag_end {
        while position < tag_end && bytes[position].is_ascii_whitespace() {
            position += 1;
        }
        if position >= tag_end {
            break;
        }
        if bytes[position] == b'/' {
            position += 1;
            continue;
        }

        let name_start = position;
        while position < tag_end
            && !bytes[position].is_ascii_whitespace()
            && !matches!(bytes[position], b'=' | b'/' | b'>')
        {
            position += 1;
        }
        if name_start == position {
            position += 1;
            continue;
        }

        let name = &bytes[name_start..position];
        while position < tag_end && bytes[position].is_ascii_whitespace() {
            position += 1;
        }

        let mut value = None;
        if bytes.get(position) == Some(&b'=') {
            position += 1;
            while position < tag_end && bytes[position].is_ascii_whitespace() {
                position += 1;
            }
            let Some(&first) = bytes.get(position) else {
                break;
            };
            if matches!(first, b'\'' | b'"') {
                position += 1;
                let value_start = position;
                while position < tag_end && bytes[position] != first {
                    position += 1;
                }
                if position >= tag_end {
                    break;
                }
                value = Some(&bytes[value_start..position]);
                position += 1;
            } else {
                let value_start = position;
                while position < tag_end
                    && !bytes[position].is_ascii_whitespace()
                    && bytes[position] != b'>'
                {
                    position += 1;
                }
                value = Some(&bytes[value_start..position]);
            }
        }

        if name.eq_ignore_ascii_case(b"id") && value == Some(b"__NEXT_DATA__") {
            return true;
        }
    }
    false
}

fn find_script_close(bytes: &[u8], mut position: usize) -> Option<(usize, usize)> {
    while position < bytes.len() {
        let relative_start = bytes[position..].iter().position(|byte| *byte == b'<')?;
        let close_start = position + relative_start;
        if bytes
            .get(close_start..close_start + b"</script".len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(b"</script"))
            && bytes
                .get(close_start + b"</script".len())
                .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'>')
        {
            let close_end = find_html_tag_end(bytes, close_start)?;
            return Some((close_start, close_end + 1));
        }
        position = close_start + 1;
    }
    None
}

fn next_line_start(text: &str, position: usize) -> usize {
    text[position..]
        .find('\n')
        .map_or(text.len(), |offset| position + offset + 1)
}

fn extract_json_string(text: &str, start: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }

    let mut position = start + 1;
    let mut escaped = false;
    while position < bytes.len() {
        let byte = bytes[position];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return text.get(start..=position);
        }
        position += 1;
    }
    None
}

fn skip_json_whitespace(text: &str, mut position: usize) -> usize {
    let bytes = text.as_bytes();
    while bytes.get(position).is_some_and(u8::is_ascii_whitespace) {
        position += 1;
    }
    position
}

fn extract_balanced_segment(text: &str, start: usize, opening: u8, closing: u8) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.get(start).copied()? != opening {
        return None;
    }

    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut position = start;
    while position < bytes.len() {
        let byte = bytes[position];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            position += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            position += 1;
            continue;
        }

        if byte == opening {
            depth += 1;
        } else if byte == closing {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return text.get(start..=position);
            }
        }
        position += 1;
    }
    None
}
