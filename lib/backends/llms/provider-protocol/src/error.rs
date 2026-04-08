//! Structured error types for LLM provider backends.
//! Each error variant carries enough information for the circuit breaker
//! and signalograd to make adaptive decisions.

/// Structured LLM completion error — not an opaque string.
/// The conductor uses this to decide: cascade, retry, or give up.
#[derive(Debug, Clone)]
pub enum CompletionError {
    /// Connection failed instantly (DNS, refused, TLS). Cascade immediately.
    ConnectionFailed { provider: String, detail: String },
    /// Server returned error status (401, 403, 429, 500, 503). Cascade.
    ServerError { provider: String, status: u16, detail: String },
    /// Request sent but response timed out. Model may be thinking or stalled.
    Timeout { provider: String, elapsed_ms: u64 },
    /// Response received but couldn't parse. Provider returned garbage.
    ParseError { provider: String, detail: String },
    /// Rate limited. Don't cascade — just wait or use different key.
    RateLimited { provider: String, retry_after_secs: Option<u64> },
    /// Model explicitly returned an error in its response body.
    ModelError { provider: String, model: String, detail: String },
}

impl CompletionError {
    /// Should the system cascade to the next model? Pure functional decision.
    pub fn should_cascade(&self) -> bool {
        match self {
            Self::ConnectionFailed { .. } => true,
            Self::ServerError { status, .. } => *status >= 500,
            Self::Timeout { .. } => true,
            Self::ModelError { .. } => true,
            Self::RateLimited { .. } => false,
            Self::ParseError { .. } => true,
        }
    }

    /// Should the circuit breaker trip for this model?
    pub fn should_trip_breaker(&self) -> bool {
        match self {
            Self::ConnectionFailed { .. } => true,
            Self::ServerError { status, .. } => *status >= 500,
            Self::Timeout { .. } => true,
            _ => false,
        }
    }

    /// Is this a rate limit that we should back off from (not cascade)?
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, Self::RateLimited { .. })
    }

    /// Serialize to sexp for Lisp consumption via IPC.
    pub fn to_sexp(&self) -> String {
        match self {
            Self::ConnectionFailed { provider, detail } =>
                format!("(:error :kind :connection-failed :provider \"{}\" :detail \"{}\")",
                    provider, detail),
            Self::ServerError { provider, status, detail } =>
                format!("(:error :kind :server-error :provider \"{}\" :status {} :detail \"{}\")",
                    provider, status, detail),
            Self::Timeout { provider, elapsed_ms } =>
                format!("(:error :kind :timeout :provider \"{}\" :elapsed-ms {})",
                    provider, elapsed_ms),
            Self::ParseError { provider, detail } =>
                format!("(:error :kind :parse-error :provider \"{}\" :detail \"{}\")",
                    provider, detail),
            Self::RateLimited { provider, retry_after_secs } =>
                format!("(:error :kind :rate-limited :provider \"{}\" :retry-after {})",
                    provider, retry_after_secs.unwrap_or(0)),
            Self::ModelError { provider, model, detail } =>
                format!("(:error :kind :model-error :provider \"{}\" :model \"{}\" :detail \"{}\")",
                    provider, model, detail),
        }
    }
}

impl std::fmt::Display for CompletionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed { provider, detail } =>
                write!(f, "{}: connection failed: {}", provider, detail),
            Self::ServerError { provider, status, detail } =>
                write!(f, "{}: HTTP {}: {}", provider, status, detail),
            Self::Timeout { provider, elapsed_ms } =>
                write!(f, "{}: timeout after {}ms", provider, elapsed_ms),
            Self::ParseError { provider, detail } =>
                write!(f, "{}: parse error: {}", provider, detail),
            Self::RateLimited { provider, retry_after_secs } =>
                write!(f, "{}: rate limited (retry after {}s)", provider, retry_after_secs.unwrap_or(0)),
            Self::ModelError { provider, model, detail } =>
                write!(f, "{}/{}: {}", provider, model, detail),
        }
    }
}
