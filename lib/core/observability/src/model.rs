//! Core types and global state for observability.

use std::time::{SystemTime, UNIX_EPOCH};

// ─── State ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ObservabilityState {
    pub enabled: bool,
    pub initialized: bool,
    pub trace_level: TraceLevel,
    pub sample_rate: f64,
    pub project_name: String,
    pub api_url: String,
    pub api_key: String,
    /// Monotonic handle counter. 0 = disabled/sampled-out.
    pub next_handle: i64,
}

impl Default for ObservabilityState {
    fn default() -> Self {
        Self {
            enabled: false,
            initialized: false,
            trace_level: TraceLevel::Standard,
            sample_rate: 1.0,
            project_name: "harmonia".to_string(),
            api_url: "https://api.smith.langchain.com".to_string(),
            api_key: String::new(),
            next_handle: 1,
        }
    }
}

impl ObservabilityState {
    pub fn alloc_handle(&mut self) -> i64 {
        if !self.enabled || self.api_key.is_empty() {
            return 0;
        }
        // Probabilistic sampling at root span level
        if self.sample_rate < 1.0 {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            let r: f64 = (nanos % 10000) as f64 / 10000.0;
            if r > self.sample_rate {
                return 0;
            }
        }
        let h = self.next_handle;
        self.next_handle += 1;
        h
    }
}

// ─── Trace types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceLevel {
    Minimal,
    Standard,
    Verbose,
}

impl TraceLevel {
    pub fn from_str(s: &str) -> Self {
        match s {
            "minimal" => Self::Minimal,
            "verbose" => Self::Verbose,
            _ => Self::Standard,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Standard => "standard",
            Self::Verbose => "verbose",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceSpan {
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub trace_id: String,
    pub dotted_order: String,
    pub name: String,
    pub run_type: String, // "chain", "llm", "tool", "agent"
    pub start_time: String,
    pub end_time: Option<String>,
    pub status: Option<String>, // "success", "error"
    pub inputs: serde_json::Value,
    pub outputs: Option<serde_json::Value>,
    pub extra: serde_json::Value,
    pub project_name: String,
}

#[derive(Debug, Clone)]
pub struct TraceEvent {
    pub name: String,
    pub run_type: String,
    pub metadata: serde_json::Value,
    pub project_name: String,
    pub trace_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub dotted_order: Option<String>,
}

/// Messages sent to the background sender thread.
#[derive(Debug)]
pub enum TraceMessage {
    /// Create/start a new run (span).
    StartRun(TraceSpan),
    /// End/update a run with outputs and status.
    EndRun {
        run_id: String,
        status: String,
        outputs: serde_json::Value,
        end_time: String,
    },
    /// Fire-and-forget event (creates a completed run).
    Event(TraceEvent),
    /// Flush pending batches immediately.
    Flush,
    /// Shut down the sender thread.
    Shutdown,
}

// ─── Helpers ─────────────────────────────────────────────────────────

pub fn now_iso() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    // ISO 8601 UTC
    let (s, m, h, day, month, year) = secs_to_utc(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, h, m, s, millis
    )
}

fn secs_to_utc(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    // Simple UTC decomposition (no leap seconds)
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let mut days = secs / 86400;
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for md in &month_days {
        if days < *md {
            break;
        }
        days -= *md;
        month += 1;
    }
    let day = days + 1;
    (s, m, h, day, month, year)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

