#[derive(Debug, Clone)]
pub struct AuditContext {
    pub timestamp_ms: u64,
    pub dissonance: f64,
}

impl AuditContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:timestamp-ms {} :dissonance {:.4})",
            self.timestamp_ms, self.dissonance
        )
    }
}
