//! Signal integrity: injection detection, dissonance scoring, and boundary wrapping.

mod boundary;
mod dissonance;
pub(crate) mod normalize;
pub(crate) mod patterns;
mod scanner;

pub use boundary::wrap_secure;
pub use dissonance::compute_dissonance;
pub use normalize::normalize_unicode;
pub use patterns::{Category, Severity};
pub use scanner::{scan_for_injection, ScanReport};

#[cfg(test)]
mod tests;
