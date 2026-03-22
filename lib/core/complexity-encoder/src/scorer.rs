use crate::dimensions;
use crate::tier::{ComplexityProfile, ComplexityTier};

/// Dimension weights — sum to 1.0.
const WEIGHTS: [f64; 14] = [
    0.08, 0.12, 0.14, 0.08, 0.04, 0.06, 0.10, 0.06, 0.06, 0.08, 0.04, 0.04, 0.04, 0.06,
];

#[inline(always)]
fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
fn sigmoid_confidence(score: f64, tier: ComplexityTier) -> f64 {
    let boundary = tier.lower_boundary();
    let upper = match tier {
        ComplexityTier::Simple => 0.25,
        ComplexityTier::Medium => 0.50,
        ComplexityTier::Complex => 0.75,
        ComplexityTier::Reasoning => 1.0,
    };
    let distance = (score - boundary).abs().min((score - upper).abs());
    0.5 + 0.5 / (1.0 + (-12.0 * distance).exp())
}

/// Score a prompt across all 14 dimensions.
///
/// One allocation: `to_ascii_lowercase()` for the input text.
/// Then 14 dimension scores using std's SIMD-optimized `str::contains`.
/// All keyword sets are compile-time `&[&str]` — zero indirection.
pub fn score(text: &str) -> ComplexityProfile {
    // Single allocation: lowercase buffer for case-insensitive matching.
    // std's to_ascii_lowercase is branchless/vectorized on modern CPUs.
    let lower = text.to_ascii_lowercase();

    let dims = [
        dimensions::dim_token_count(&lower),
        dimensions::dim_code_presence(&lower),
        dimensions::dim_reasoning_markers(&lower),
        dimensions::dim_technical_terms(&lower),
        dimensions::dim_creative_markers(&lower),
        dimensions::dim_simple_indicators(&lower),
        dimensions::dim_multi_step_patterns(&lower),
        dimensions::dim_question_complexity(&lower),
        dimensions::dim_imperative_verbs(&lower),
        dimensions::dim_constraint_indicators(&lower),
        dimensions::dim_output_format(&lower),
        dimensions::dim_reference_complexity(&lower),
        dimensions::dim_negation_complexity(&lower),
        dimensions::dim_domain_specificity(&lower),
    ];

    // Weighted aggregate
    let mut raw = 0.0_f64;
    let mut i = 0;
    while i < 14 {
        raw += dims[i] * WEIGHTS[i];
        i += 1;
    }
    let score = clamp((raw + 1.0) * 0.5, 0.0, 1.0);

    // Special overrides
    let (tier, confidence) = if dimensions::reasoning_override(&lower) {
        (ComplexityTier::Reasoning, 0.97)
    } else if dims[5] < -0.8 && text.len() < 50 {
        (ComplexityTier::Simple, 0.95)
    } else {
        let mut tier = ComplexityTier::from_score(score);
        if dims[1] > 0.5 && dims[3] > 0.3 {
            tier = match tier {
                ComplexityTier::Simple => ComplexityTier::Medium,
                ComplexityTier::Medium => ComplexityTier::Complex,
                other => other,
            };
        }
        (tier, sigmoid_confidence(score, tier))
    };

    ComplexityProfile {
        tier,
        score,
        confidence,
        dimensions: dims,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_is_simple() {
        let p = score("hello");
        assert_eq!(p.tier, ComplexityTier::Simple);
        assert!(p.confidence > 0.7);
    }

    #[test]
    fn what_is_2_plus_2_is_simple_or_medium() {
        let p = score("what is 2+2");
        assert!(p.tier == ComplexityTier::Simple || p.tier == ComplexityTier::Medium);
    }

    #[test]
    fn implement_btree_is_complex_or_reasoning() {
        let p = score("implement a distributed B-tree with Raft consensus, ensure thread-safe concurrent access, and add comprehensive test coverage");
        assert!(p.tier == ComplexityTier::Complex || p.tier == ComplexityTier::Reasoning);
    }

    #[test]
    fn prove_theorem_triggers_reasoning_override() {
        let p = score("prove the theorem using formal verification techniques and derive bounds");
        assert_eq!(p.tier, ComplexityTier::Reasoning);
        assert!(p.confidence >= 0.95);
    }

    #[test]
    fn score_always_in_range() {
        let long = "a".repeat(10000);
        for prompt in ["", "hi", long.as_str(), "implement everything from scratch"] {
            let p = score(prompt);
            assert!((0.0..=1.0).contains(&p.score));
            assert!((0.5..=1.0).contains(&p.confidence));
        }
    }
}
