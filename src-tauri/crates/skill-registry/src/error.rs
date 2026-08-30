use std::time::{Duration, SystemTime};

use thiserror::Error;

use crate::{model::RegistrySkillIdError, source::SourceParseError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseKind {
    Search,
    Leaderboard,
    Detail,
}

impl std::fmt::Display for ResponseKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Search => "search",
            Self::Leaderboard => "leaderboard",
            Self::Detail => "detail",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryAfter {
    Delay(Duration),
    At(SystemTime),
}

impl RetryAfter {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if !value.is_empty() && value.chars().all(|character| character.is_ascii_digit()) {
            return value
                .parse::<u64>()
                .ok()
                .map(|seconds| Self::Delay(Duration::from_secs(seconds)));
        }
        httpdate::parse_http_date(value).ok().map(Self::At)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportOperation {
    BuildClient,
    SearchRequest,
    LeaderboardRequest,
    ReadResponseBody,
}

impl std::fmt::Display for TransportOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::BuildClient => "build client",
            Self::SearchRequest => "search request",
            Self::LeaderboardRequest => "leaderboard request",
            Self::ReadResponseBody => "read response body",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Connect,
    Request,
    ResponseBody,
    Other,
}

impl std::fmt::Display for TransportKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Connect => "connect",
            Self::Request => "request",
            Self::ResponseBody => "response body",
            Self::Other => "other",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum QueryValidationError {
    #[error("query must not be empty")]
    Empty,
    #[error("query must not contain control characters")]
    ContainsControlCharacter,
    #[error("query must contain at least {minimum} characters")]
    TooShort { minimum: usize },
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("invalid search query: {reason}")]
    InvalidQuery { reason: QueryValidationError },
    #[error("search limit {limit} is outside the supported range {min}..={max}")]
    InvalidLimit {
        limit: usize,
        min: usize,
        max: usize,
    },
    #[error("registry client timeout must be greater than zero")]
    InvalidTimeout,
    #[error("registry response body limit must be greater than zero")]
    InvalidResponseLimit,
    #[error("registry response body limit {requested} exceeds the global maximum {maximum}")]
    ResponseLimitTooLarge { requested: usize, maximum: usize },
    #[error("registry base URL is invalid")]
    InvalidBaseUrl,
    #[error("registry base URL must use HTTP or HTTPS")]
    UnsupportedBaseUrlScheme,
    #[error("registry base URL must not contain a query or fragment")]
    BaseUrlQueryOrFragment,
    #[error("registry proxy configuration is invalid")]
    InvalidProxy,
    #[error("registry request timed out during {operation} ({kind})")]
    Timeout {
        operation: TransportOperation,
        kind: TransportKind,
    },
    #[error("registry transport failed during {operation} ({kind})")]
    Transport {
        operation: TransportOperation,
        kind: TransportKind,
    },
    #[error("registry returned HTTP status {status}")]
    HttpStatus {
        status: u16,
        retry_after: Option<RetryAfter>,
    },
    #[error("registry authentication is required (HTTP {status})")]
    AuthenticationRequired {
        status: u16,
        retry_after: Option<RetryAfter>,
    },
    #[error("registry rate limit exceeded (HTTP {status})")]
    RateLimited {
        status: u16,
        retry_after: Option<RetryAfter>,
    },
    #[error("registry response body is too large (limit {limit} bytes)")]
    ResponseTooLarge { limit: usize, observed: Option<u64> },
    #[error("invalid {kind} response: {message}")]
    InvalidResponse { kind: ResponseKind, message: String },
    #[error("{kind} response is missing field '{field}'")]
    MissingResponseField { kind: ResponseKind, field: String },
    #[error(transparent)]
    InvalidRegistrySkillId(#[from] RegistrySkillIdError),
    #[error(transparent)]
    Source(#[from] SourceParseError),
}
