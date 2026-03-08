use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLabel {
    Owner,
    Authenticated,
    Anonymous,
    Untrusted,
}

impl SecurityLabel {
    pub fn from_str(s: &str) -> Self {
        match s {
            "owner" => Self::Owner,
            "authenticated" => Self::Authenticated,
            "anonymous" => Self::Anonymous,
            _ => Self::Untrusted,
        }
    }

    pub fn weight(&self) -> f64 {
        match self {
            Self::Owner => 1.0,
            Self::Authenticated => 0.8,
            Self::Anonymous => 0.4,
            Self::Untrusted => 0.1,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Authenticated => "authenticated",
            Self::Anonymous => "anonymous",
            Self::Untrusted => "untrusted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelId {
    pub frontend: String,
    pub sub_channel: String,
}

impl ChannelId {
    pub fn new(frontend: impl Into<String>, sub_channel: impl Into<String>) -> Self {
        Self {
            frontend: frontend.into(),
            sub_channel: sub_channel.into(),
        }
    }

    pub fn to_sexp(&self) -> String {
        format!(
            "(:frontend \"{}\" :sub-channel \"{}\")",
            self.frontend, self.sub_channel
        )
    }
}

impl fmt::Display for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.frontend, self.sub_channel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub id: u64,
    pub channel: ChannelId,
    pub security: SecurityLabel,
    pub payload: String,
    pub timestamp_ms: u64,
    pub direction: SignalDirection,
    /// Per-message metadata emitted by the frontend (s-expr string).
    /// Example: `(:platform "ios" :device-id "uuid-123" :a2ui-version "1.0")`
    pub metadata: Option<String>,
    /// Frontend-level capabilities from baseband config (s-expr string).
    /// Example: `(:a2ui "1.0" :push "t")`
    pub capabilities: Option<String>,
    /// Wave 3.3: Dissonance score from injection scan (0.0 = clean, 0.95 = suspicious).
    pub dissonance: f64,
}

impl Signal {
    pub fn to_sexp(&self) -> String {
        let meta = match &self.metadata {
            Some(m) => format!(" :metadata {}", m),
            None => String::new(),
        };
        let caps = match &self.capabilities {
            Some(c) if c != "nil" => format!(" :capabilities {}", c),
            _ => String::new(),
        };
        format!(
            "(:id {} :channel {} :security \"{}\" :direction \"{}\" :timestamp {} :dissonance {:.4} :payload \"{}\"{}{})",
            self.id,
            self.channel.to_sexp(),
            self.security.as_str(),
            match self.direction {
                SignalDirection::Inbound => "inbound",
                SignalDirection::Outbound => "outbound",
            },
            self.timestamp_ms,
            self.dissonance,
            self.payload.replace('\\', "\\\\").replace('"', "\\\""),
            caps,
            meta,
        )
    }
}

#[derive(Debug, Clone)]
pub struct BasebandBatch {
    pub signals: Vec<Signal>,
    pub poll_timestamp_ms: u64,
}

impl BasebandBatch {
    pub fn to_sexp(&self) -> String {
        if self.signals.is_empty() {
            return "nil".to_string();
        }
        let items: Vec<String> = self.signals.iter().map(|s| s.to_sexp()).collect();
        format!("({})", items.join(" "))
    }
}
