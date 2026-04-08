//! Crash ledger — dual-tier: recovery.log (file) + chronicle (SQLite).
//! Pure functional: state passed in, no globals.

use std::io::Write;
use crate::OuroborosState;

/// A single crash entry.
pub struct CrashEntry {
    pub timestamp: u64,
    pub kind: String,
    pub detail: String,
}

impl std::fmt::Display for CrashEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\t{}\t{}", self.timestamp, self.kind, self.detail)
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_secs()
}

/// Record a crash to both recovery.log and chronicle.
pub fn record_crash(state: &OuroborosState, component: &str, detail: &str) -> Result<(), String> {
    let kind = format!("ouroboros/{}", component);
    // Tier 1: Append to recovery.log
    let path = std::path::Path::new(&state.recovery_log_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true).append(true).open(path)
        .map_err(|e| format!("open log: {e}"))?;
    writeln!(file, "{}\t{}\t{}", now_secs(), kind, detail)
        .map_err(|e| format!("write log: {e}"))?;
    // Tier 2: Record to chronicle
    let _ = harmonia_chronicle::ouroboros::record("crash", Some(component), Some(detail), None, false);
    Ok(())
}

/// Get the most recent crash entry.
pub fn last_crash(state: &OuroborosState) -> Result<CrashEntry, String> {
    let content = std::fs::read_to_string(&state.recovery_log_path)
        .map_err(|e| format!("read log: {e}"))?;
    content.lines().rev()
        .find(|l| !l.trim().is_empty())
        .map(parse_entry)
        .unwrap_or_else(|| Err("no crash events".into()))
}

/// Get the last N crash entries.
pub fn history(state: &OuroborosState, limit: usize) -> Result<Vec<CrashEntry>, String> {
    let content = std::fs::read_to_string(&state.recovery_log_path)
        .map_err(|e| format!("read log: {e}"))?;
    Ok(content.lines().rev()
        .filter(|l| !l.trim().is_empty())
        .take(limit)
        .filter_map(|l| parse_entry(l).ok())
        .collect::<Vec<_>>()
        .into_iter().rev().collect())
}

fn parse_entry(line: &str) -> Result<CrashEntry, String> {
    let parts: Vec<&str> = line.splitn(3, '\t').collect();
    if parts.len() < 3 { return Err("malformed entry".into()); }
    Ok(CrashEntry {
        timestamp: parts[0].parse().unwrap_or(0),
        kind: parts[1].to_string(),
        detail: parts[2].to_string(),
    })
}
