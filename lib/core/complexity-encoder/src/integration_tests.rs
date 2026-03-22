/// Integration tests for the full routing pipeline:
/// encoder → ComplexityProfile → tier mapping → model pool filtering

#[cfg(test)]
mod tests {
    use crate::profile_to_sexp;
    use crate::scorer::score;
    use crate::tier::ComplexityTier;

    // ── Encoder accuracy across prompt categories ─────────────────────

    #[test]
    fn greetings_are_simple() {
        let prompts = ["hello", "hi", "hey there", "good morning", "thanks!", "bye"];
        for prompt in &prompts {
            let p = score(prompt);
            assert!(
                p.tier == ComplexityTier::Simple || p.tier == ComplexityTier::Medium,
                "greeting '{}' should be Simple/Medium, got {:?} (score={:.3})",
                prompt,
                p.tier,
                p.score
            );
        }
    }

    #[test]
    fn simple_questions_are_not_reasoning() {
        let prompts = [
            "what time is it",
            "who is the president",
            "how are you",
            "what is your name",
        ];
        for prompt in &prompts {
            let p = score(prompt);
            assert!(
                p.tier != ComplexityTier::Reasoning,
                "simple question '{}' should NOT be Reasoning, got {:?} (score={:.3})",
                prompt,
                p.tier,
                p.score
            );
        }
    }

    #[test]
    fn code_tasks_are_complex_or_higher() {
        let prompts = [
            "implement a binary search tree in Rust with insert, delete, and search operations",
            "write a function that merges two sorted linked lists",
            "refactor this code:\n```python\ndef foo(x):\n    return x*2\n```\nto handle edge cases",
            "debug this async function that deadlocks when multiple threads access the shared mutex",
        ];
        for prompt in &prompts {
            let p = score(prompt);
            assert!(
                p.tier == ComplexityTier::Complex || p.tier == ComplexityTier::Reasoning,
                "code task should be Complex+, got {:?} for '{}' (score={:.3})",
                p.tier,
                &prompt[..prompt.len().min(60)],
                p.score
            );
        }
    }

    #[test]
    fn reasoning_tasks_detected() {
        let prompts = [
            "prove that the square root of 2 is irrational using formal verification",
            "derive the time complexity of quicksort using the master theorem and prove the average case",
            "analyze the trade-offs between consistency and availability in distributed systems, deduce which CAP theorem configuration is optimal for our use case",
        ];
        for prompt in &prompts {
            let p = score(prompt);
            assert!(
                p.tier == ComplexityTier::Reasoning || p.tier == ComplexityTier::Complex,
                "reasoning task should be Reasoning/Complex, got {:?} for '{}' (score={:.3})",
                p.tier,
                &prompt[..prompt.len().min(60)],
                p.score
            );
        }
    }

    #[test]
    fn multi_step_tasks_are_complex() {
        let prompt = "First, create a new database migration. Then, update the API endpoints to use the new schema. Next, write integration tests for the updated endpoints. Finally, deploy to staging and run the smoke tests.";
        let p = score(prompt);
        assert!(
            p.tier == ComplexityTier::Complex || p.tier == ComplexityTier::Reasoning,
            "multi-step task should be Complex+, got {:?} (score={:.3})",
            p.tier,
            p.score
        );
    }

    #[test]
    fn domain_specific_tasks_score_high() {
        let prompts = [
            "implement a cryptographic hash function with collision resistance for blockchain smart contracts",
            "design a neural network architecture for genomic sequence classification using transformer embeddings",
            "analyze the thermodynamics of a fluid dynamics simulation with stochastic differential equations",
        ];
        for prompt in &prompts {
            let p = score(prompt);
            assert!(
                p.tier == ComplexityTier::Complex || p.tier == ComplexityTier::Reasoning,
                "domain-specific should be Complex+, got {:?} for '{}' (score={:.3})",
                p.tier,
                &prompt[..prompt.len().min(60)],
                p.score
            );
        }
    }

    // ── Confidence calibration ────────────────────────────────────────

    #[test]
    fn confidence_always_valid_range() {
        let prompts = [
            "",
            "hi",
            "explain quantum computing",
            "implement a distributed consensus protocol with Raft, ensuring linearizability and leader election with formal verification of the safety properties",
        ];
        for prompt in &prompts {
            let p = score(prompt);
            assert!(
                (0.5..=1.0).contains(&p.confidence),
                "confidence {:.3} out of [0.5, 1.0] for '{}'",
                p.confidence,
                &prompt[..prompt.len().min(40)]
            );
        }
    }

    #[test]
    fn reasoning_override_gives_high_confidence() {
        let p = score("prove the theorem and derive the formal verification");
        assert!(
            p.confidence >= 0.90,
            "reasoning override should have confidence >= 0.90, got {:.3}",
            p.confidence
        );
    }

    // ── Score monotonicity ────────────────────────────────────────────

    #[test]
    fn longer_complex_prompts_score_higher_than_short_simple() {
        let simple = score("hello");
        let complex = score(
            "implement a distributed key-value store with Raft consensus, \
             automatic sharding, conflict resolution using vector clocks, \
             and a query optimizer that handles joins across partitions",
        );
        assert!(
            complex.score > simple.score,
            "complex (score={:.3}) should score higher than simple (score={:.3})",
            complex.score,
            simple.score
        );
    }

    // ── Sexp output format ────────────────────────────────────────────

    #[test]
    fn sexp_output_parseable() {
        let p = score("implement a REST API with authentication");
        let sexp = profile_to_sexp(&p);

        // Must contain all required fields
        assert!(sexp.contains(":tier"), "sexp missing :tier");
        assert!(sexp.contains(":score"), "sexp missing :score");
        assert!(sexp.contains(":confidence"), "sexp missing :confidence");
        assert!(sexp.contains(":dimensions"), "sexp missing :dimensions");

        // Tier value must be one of the valid strings
        let valid_tiers = ["\"simple\"", "\"medium\"", "\"complex\"", "\"reasoning\""];
        assert!(
            valid_tiers.iter().any(|t| sexp.contains(t)),
            "sexp tier not one of valid values: {}",
            sexp
        );

        // Dimensions should have 14 values
        let dims_start = sexp.find(":dimensions (").unwrap() + 13;
        let dims_end = sexp[dims_start..].find(')').unwrap() + dims_start;
        let dims_str = &sexp[dims_start..dims_end];
        let dim_count = dims_str.split_whitespace().count();
        assert_eq!(dim_count, 14, "expected 14 dimensions, got {}", dim_count);
    }

    // ── FFI safety ────────────────────────────────────────────────────

    #[test]
    fn ffi_handles_empty_string() {
        let input = std::ffi::CString::new("").unwrap();
        let result = crate::harmonia_complexity_encoder_score(input.as_ptr());
        assert!(!result.is_null());
        crate::harmonia_complexity_encoder_free_string(result);
    }

    #[test]
    fn ffi_handles_unicode() {
        let input =
            std::ffi::CString::new("实现一个分布式系统 implement distributed system").unwrap();
        let result = crate::harmonia_complexity_encoder_score(input.as_ptr());
        assert!(!result.is_null());
        let output = unsafe { std::ffi::CStr::from_ptr(result) }
            .to_str()
            .unwrap();
        assert!(output.contains(":tier"));
        crate::harmonia_complexity_encoder_free_string(result);
    }

    #[test]
    fn ffi_handles_long_input() {
        let long_text = "implement ".repeat(5000);
        let input = std::ffi::CString::new(long_text).unwrap();
        let result = crate::harmonia_complexity_encoder_score(input.as_ptr());
        assert!(!result.is_null());
        crate::harmonia_complexity_encoder_free_string(result);
    }

    // ── Performance sanity ────────────────────────────────────────────

    #[test]
    fn encoder_is_fast() {
        let prompt = "implement a distributed consensus protocol with automatic failover, \
                      leader election, and log replication. Ensure linearizability and \
                      handle network partitions gracefully. Add comprehensive integration tests.";
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = score(prompt);
        }
        let elapsed = start.elapsed();
        let per_call = elapsed / 1000;
        // In debug mode (~10x slower), allow up to 5ms; release target is <100μs
        let limit_us = if cfg!(debug_assertions) { 5000 } else { 200 };
        assert!(
            per_call.as_micros() < limit_us,
            "encoder too slow: {}μs per call (limit {}μs)",
            per_call.as_micros(),
            limit_us,
        );
        eprintln!(
            "[perf] encoder: {}μs per call (1000 iterations, debug={})",
            per_call.as_micros(),
            cfg!(debug_assertions),
        );
    }
}
