use serde::{Deserialize, Serialize};

use crate::sexp::{sexp_bool, sexp_string};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Authenticated => "authenticated",
            Self::Anonymous => "anonymous",
            Self::Untrusted => "untrusted",
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
}

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub label: SecurityLabel,
    pub source: String,
    pub fingerprint_valid: bool,
}

impl SecurityContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:label {} :source {} :fingerprint-valid {})",
            sexp_string(self.label.as_str()),
            sexp_string(&self.source),
            sexp_bool(self.fingerprint_valid)
        )
    }
}
