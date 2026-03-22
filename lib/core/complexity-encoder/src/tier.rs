/// Complexity classification tier — maps to routing model pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityTier {
    Simple,    // score < 0.25
    Medium,    // 0.25 <= score < 0.50
    Complex,   // 0.50 <= score < 0.75
    Reasoning, // score >= 0.75
}

impl ComplexityTier {
    pub fn from_score(score: f64) -> Self {
        if score >= 0.75 {
            Self::Reasoning
        } else if score >= 0.50 {
            Self::Complex
        } else if score >= 0.25 {
            Self::Medium
        } else {
            Self::Simple
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Medium => "medium",
            Self::Complex => "complex",
            Self::Reasoning => "reasoning",
        }
    }

    /// Lower boundary of this tier for confidence calculation.
    pub fn lower_boundary(&self) -> f64 {
        match self {
            Self::Simple => 0.0,
            Self::Medium => 0.25,
            Self::Complex => 0.50,
            Self::Reasoning => 0.75,
        }
    }
}

/// Output of the 14-dimension encoder.
#[derive(Debug, Clone)]
pub struct ComplexityProfile {
    pub tier: ComplexityTier,
    pub score: f64,
    pub confidence: f64,
    pub dimensions: [f64; 14],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_from_score_boundaries() {
        assert_eq!(ComplexityTier::from_score(0.0), ComplexityTier::Simple);
        assert_eq!(ComplexityTier::from_score(0.24), ComplexityTier::Simple);
        assert_eq!(ComplexityTier::from_score(0.25), ComplexityTier::Medium);
        assert_eq!(ComplexityTier::from_score(0.49), ComplexityTier::Medium);
        assert_eq!(ComplexityTier::from_score(0.50), ComplexityTier::Complex);
        assert_eq!(ComplexityTier::from_score(0.74), ComplexityTier::Complex);
        assert_eq!(ComplexityTier::from_score(0.75), ComplexityTier::Reasoning);
        assert_eq!(ComplexityTier::from_score(1.0), ComplexityTier::Reasoning);
    }

    #[test]
    fn tier_roundtrip() {
        for tier in [
            ComplexityTier::Simple,
            ComplexityTier::Medium,
            ComplexityTier::Complex,
            ComplexityTier::Reasoning,
        ] {
            let s = tier.as_str();
            assert!(!s.is_empty());
        }
    }
}
