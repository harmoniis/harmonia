//! Core injection scanner: normalize input then match against the pattern table.

use crate::normalize::normalize_for_scan;
use crate::patterns::{InjectionPattern, Severity, PATTERNS};

/// Result of scanning text for prompt injection attempts.
#[derive(Debug, Clone)]
pub struct ScanReport {
    pub injection_detected: bool,
    /// Total number of matched patterns (preserved as u32 for API compat).
    pub injection_count: u32,
    /// Pattern strings that matched (preserved for API compat).
    pub flagged_patterns: Vec<String>,
    /// Per-severity counts.
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    /// Deduplicated category labels that fired.
    pub categories: Vec<&'static str>,
    /// Matched pattern strings with their severity (for detailed logging).
    pub matched_patterns: Vec<(&'static str, Severity)>,
}

/// Scan text for prompt injection attempts.
/// Normalizes unicode before scanning to catch fullwidth, homoglyph, and
/// combining-character evasion.
pub fn scan_for_injection(text: &str) -> ScanReport {
    let normalized = normalize_for_scan(text);

    let hits: Vec<&InjectionPattern> = PATTERNS
        .iter()
        .filter(|p| normalized.contains(p.pattern))
        .collect();

    let critical_count = hits.iter().filter(|h| h.severity == Severity::Critical).count();
    let high_count     = hits.iter().filter(|h| h.severity == Severity::High).count();
    let medium_count   = hits.iter().filter(|h| h.severity == Severity::Medium).count();
    let low_count      = hits.iter().filter(|h| h.severity == Severity::Low).count();

    let flagged_patterns: Vec<String> = hits.iter().map(|h| h.pattern.to_string()).collect();

    let matched_patterns: Vec<(&'static str, Severity)> =
        hits.iter().map(|h| (h.pattern, h.severity)).collect();

    let categories: Vec<&'static str> = {
        let mut cats: Vec<&'static str> = hits.iter().map(|h| h.category.as_str()).collect();
        cats.sort_unstable();
        cats.dedup();
        cats
    };

    ScanReport {
        injection_detected: !hits.is_empty(),
        injection_count: hits.len() as u32,
        flagged_patterns,
        critical_count,
        high_count,
        medium_count,
        low_count,
        categories,
        matched_patterns,
    }
}
