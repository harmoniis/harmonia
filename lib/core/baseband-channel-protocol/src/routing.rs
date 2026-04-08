/// Number of scoring dimensions in the complexity encoder.
pub const ROUTING_DIMS: usize = 14;

/// Complexity tier -- zero-size discriminant, no heap allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityTier {
    Simple,
    Medium,
    Complex,
    Reasoning,
}

impl ComplexityTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Medium => "medium",
            Self::Complex => "complex",
            Self::Reasoning => "reasoning",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "simple" => Self::Simple,
            "medium" => Self::Medium,
            "complex" => Self::Complex,
            "reasoning" => Self::Reasoning,
            _ => Self::Medium,
        }
    }
}

/// User routing tier -- zero-size discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserTier {
    Auto,
    Eco,
    Premium,
    Free,
}

impl UserTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Eco => "eco",
            Self::Premium => "premium",
            Self::Free => "free",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "eco" => Self::Eco,
            "premium" => Self::Premium,
            "free" => Self::Free,
            _ => Self::Auto,
        }
    }
}

/// Routing metadata on every signal -- fully stack-allocated (130 bytes).
/// No String, no Vec, no heap indirection.
#[derive(Debug, Clone, Copy)]
pub struct RoutingContext {
    pub tier: ComplexityTier,
    pub score: f64,
    pub confidence: f64,
    pub active_tier: UserTier,
    pub dimensions: [f64; ROUTING_DIMS],
}

impl RoutingContext {
    pub fn to_sexp(&self) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(256);
        let _ = write!(
            out,
            "(:tier \"{}\" :score {:.4} :confidence {:.4} :active-tier \"{}\" :dimensions (",
            self.tier.as_str(),
            self.score,
            self.confidence,
            self.active_tier.as_str()
        );
        for (i, d) in self.dimensions.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let _ = write!(out, "{:.4}", d);
        }
        out.push_str("))");
        out
    }
}
