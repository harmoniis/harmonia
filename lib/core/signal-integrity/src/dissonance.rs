//! Severity-weighted dissonance scoring from scan reports.

use crate::scanner::ScanReport;

/// Compute dissonance score from a scan report.
/// Severity weights: Critical=0.40, High=0.25, Medium=0.12, Low=0.05.
/// Returns a value between 0.0 (clean) and 1.0 (maximum threat).
pub fn compute_dissonance(report: &ScanReport) -> f64 {
    let score = report.critical_count as f64 * 0.40
        + report.high_count as f64 * 0.25
        + report.medium_count as f64 * 0.12
        + report.low_count as f64 * 0.05;
    score.min(1.0)
}
