//! SQLite-backed metrics store for model performance, catalogue, and tmux agent tracking.
//!
//! Single unified database: `{HARMONIA_STATE_ROOT}/metrics.db`
//! Tables:
//!   - `models`         -- full model catalogue (synced from OpenRouter API + hardcoded)
//!   - `llm_perf`       -- every LLM backend call with latency/success/pricing
//!   - `parallel_tasks` -- parallel-agent task completions
//!   - `tmux_events`    -- tmux CLI agent lifecycle events
//!
//! The agent can run arbitrary SELECT queries via `query_sql()` to get any data it needs.

mod bridge;
mod catalogue;
mod db;
mod query;
mod record;

pub use bridge::*;
pub use catalogue::*;
pub use query::*;
pub use record::*;
