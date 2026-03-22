use crate::keywords;

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

/// Count keyword hits using std's SIMD-optimized `str::contains`.
/// Caller must pass pre-lowercased text. Keywords are lowercase constants.
#[inline]
fn keyword_hits(text: &str, keywords: &[&str]) -> usize {
    let mut count = 0;
    for kw in keywords {
        if text.contains(kw) {
            count += 1;
        }
    }
    count
}

#[inline(always)]
fn hits_to_score(hits: usize, saturation: usize) -> f64 {
    if saturation == 0 {
        return 0.0;
    }
    clamp(hits as f64 / saturation as f64, 0.0, 1.0)
}

// ── Dimension 0: Token count ────────────────────────────────────────

#[inline]
pub fn dim_token_count(text: &str) -> f64 {
    let len = text.len();
    if len < 30 {
        -0.8
    } else if len < 100 {
        -0.3
    } else if len < 300 {
        0.1
    } else if len < 800 {
        0.4
    } else if len < 2000 {
        0.7
    } else {
        1.0
    }
}

// ── Dimension 1: Code presence ──────────────────────────────────────

pub fn dim_code_presence(text: &str) -> f64 {
    let has_backtick_block = text.contains("```");
    let hits = keyword_hits(text, keywords::CODE_KEYWORDS);
    if has_backtick_block {
        clamp(0.6 + hits_to_score(hits, 6) * 0.4, 0.0, 1.0)
    } else if hits >= 4 {
        0.8
    } else {
        hits_to_score(hits, 8)
    }
}

// ── Dimension 2: Reasoning markers ──────────────────────────────────

pub fn dim_reasoning_markers(text: &str) -> f64 {
    let hits = keyword_hits(text, keywords::REASONING_KEYWORDS);
    if hits >= 3 {
        1.0
    } else {
        hits_to_score(hits, 4)
    }
}

static FORMAL_REASONING: &[&str] = &[
    "prove",
    "theorem",
    "formal verification",
    "mathematical proof",
    "derive",
    "deduce",
    "chain of thought",
];

pub fn reasoning_override(text: &str) -> bool {
    keyword_hits(text, FORMAL_REASONING) >= 2
}

// ── Dimension 3: Technical terms ────────────────────────────────────

pub fn dim_technical_terms(text: &str) -> f64 {
    hits_to_score(keyword_hits(text, keywords::TECHNICAL_TERMS), 6)
}

// ── Dimension 4: Creative markers ───────────────────────────────────

pub fn dim_creative_markers(text: &str) -> f64 {
    hits_to_score(keyword_hits(text, keywords::CREATIVE_MARKERS), 3)
}

// ── Dimension 5: Simple indicators (negative) ───────────────────────

pub fn dim_simple_indicators(text: &str) -> f64 {
    let hits = keyword_hits(text, keywords::SIMPLE_INDICATORS);
    let word_count = text.split_ascii_whitespace().count();
    if hits > 0 && word_count <= 8 {
        -1.0
    } else if hits > 0 {
        -0.4
    } else {
        0.0
    }
}

// ── Dimension 6: Multi-step patterns ────────────────────────────────

pub fn dim_multi_step_patterns(text: &str) -> f64 {
    let hits = keyword_hits(text, keywords::MULTI_STEP_PATTERNS);
    if hits >= 4 {
        1.0
    } else {
        hits_to_score(hits, 5)
    }
}

// ── Dimension 7: Question complexity ────────────────────────────────

static WH_WORDS: &[&str] = &["what", "why", "how", "when", "where", "which", "who"];

pub fn dim_question_complexity(text: &str) -> f64 {
    let qmarks = text.as_bytes().iter().filter(|&&b| b == b'?').count();
    let wh = keyword_hits(text, WH_WORDS);
    if qmarks == 0 && wh == 0 {
        0.0
    } else if qmarks == 1 && wh <= 1 {
        0.2
    } else if qmarks <= 2 {
        0.5
    } else {
        0.8
    }
}

// ── Dimension 8: Imperative verbs ───────────────────────────────────

pub fn dim_imperative_verbs(text: &str) -> f64 {
    let hits = keyword_hits(text, keywords::IMPERATIVE_VERBS);
    if hits >= 5 {
        1.0
    } else {
        hits_to_score(hits, 6)
    }
}

// ── Dimensions 9–13 ────────────────────────────────────────────────

pub fn dim_constraint_indicators(text: &str) -> f64 {
    hits_to_score(keyword_hits(text, keywords::CONSTRAINT_KEYWORDS), 5)
}

pub fn dim_output_format(text: &str) -> f64 {
    hits_to_score(keyword_hits(text, keywords::OUTPUT_FORMAT_KEYWORDS), 3)
}

pub fn dim_reference_complexity(text: &str) -> f64 {
    hits_to_score(keyword_hits(text, keywords::REFERENCE_KEYWORDS), 4)
}

pub fn dim_negation_complexity(text: &str) -> f64 {
    let hits = keyword_hits(text, keywords::NEGATION_KEYWORDS);
    if hits >= 4 {
        1.0
    } else {
        hits_to_score(hits, 5)
    }
}

pub fn dim_domain_specificity(text: &str) -> f64 {
    let hits = keyword_hits(text, keywords::DOMAIN_SPECIFIC);
    if hits >= 3 {
        1.0
    } else {
        hits_to_score(hits, 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_hello() {
        assert!(dim_simple_indicators("hello") < -0.5);
        assert!(dim_token_count("hello") < -0.5);
    }

    #[test]
    fn code_with_backticks() {
        let text = "implement this:\n```rust\nfn main() { println!(\"hello\"); }\n```";
        assert!(dim_code_presence(text) > 0.5);
    }

    #[test]
    fn reasoning_override_triggers() {
        assert!(reasoning_override(
            "prove the theorem using formal verification"
        ));
        assert!(!reasoning_override("explain the concept"));
    }

    #[test]
    fn multi_step_detection() {
        let text =
            "first, create the file. then, add the function. next, write tests. finally, run them.";
        assert!(dim_multi_step_patterns(text) > 0.5);
    }

    #[test]
    fn domain_specific_scoring() {
        assert!(
            dim_domain_specificity("implement a blockchain smart contract using solidity for defi")
                > 0.5
        );
    }

    #[test]
    fn constraint_scoring() {
        assert!(dim_constraint_indicators("must ensure thread-safe atomic operations with exactly once delivery and graceful fallback") > 0.6);
    }
}
