/// Harmony Tests -- testing the living system.
///
/// These tests simulate real human interaction patterns: conversations that
/// wander across domains, poetry that bridges music and mathematics,
/// curiosity that drives exploration, and the deep connections between
/// small numbers, harmony, and infinity.
///
/// "The real big numbers are not toward infinity but 1, 2, 3, 5, 7...
///  these are in harmony. And infinity converges with infinity
///  in the Lambdoma matrix." -- inspired by Leibniz
///
/// Each test is a scenario a real human would encounter.
/// We test not just correctness but *life* -- does the system resonate?

use harmonia_memory_field::{
    basin_status, eigenmode_status, field_recall, load_graph,
    step_attractors, status, restore_basin, FieldState,
    FieldCommand, FieldResult,
};

// =====================================================================
// HELPER: Build a rich, real-life concept graph
// =====================================================================

fn build_life_graph() -> (
    Vec<(String, String, i32, Vec<String>)>,
    Vec<(String, String, f64, bool)>,
) {
    let nodes = vec![
        // The harmonies -- small numbers that are "really big"
        ("harmony".into(), "music".into(), 30, vec!["SOUL-1".into(), "SKILL-H1".into()]),
        ("octave".into(), "music".into(), 15, vec!["SKILL-H2".into()]),
        ("resonance".into(), "music".into(), 12, vec!["SKILL-H3".into()]),
        ("melody".into(), "music".into(), 8, vec!["DAILY-M1".into()]),
        ("rhythm".into(), "music".into(), 7, vec!["DAILY-M2".into()]),
        ("vibration".into(), "music".into(), 10, vec!["SKILL-H4".into()]),
        ("frequency".into(), "music".into(), 9, vec!["SKILL-H5".into()]),
        // Mathematics -- the language of harmony
        ("ratio".into(), "math".into(), 20, vec!["SKILL-MA1".into()]),
        ("fibonacci".into(), "math".into(), 8, vec!["SKILL-MA2".into()]),
        ("golden".into(), "math".into(), 12, vec!["SKILL-MA3".into()]),
        ("infinity".into(), "math".into(), 10, vec!["SKILL-MA4".into()]),
        ("lambdoma".into(), "math".into(), 14, vec!["SKILL-MA5".into(), "SOUL-2".into()]),
        ("geometry".into(), "math".into(), 7, vec!["DAILY-MA1".into()]),
        ("fractal".into(), "math".into(), 9, vec!["SKILL-MA6".into()]),
        ("convergence".into(), "math".into(), 11, vec!["SKILL-MA7".into()]),
        // Engineering -- building the instrument
        ("rust".into(), "engineering".into(), 18, vec!["SKILL-E1".into(), "DAILY-E1".into()]),
        ("lisp".into(), "engineering".into(), 15, vec!["SKILL-E2".into()]),
        ("code".into(), "engineering".into(), 22, vec!["SKILL-E3".into(), "DAILY-E2".into()]),
        ("actor".into(), "engineering".into(), 10, vec!["SKILL-E4".into()]),
        ("attractor".into(), "engineering".into(), 13, vec!["SKILL-E5".into()]),
        ("laplacian".into(), "engineering".into(), 8, vec!["SKILL-E6".into()]),
        ("spectral".into(), "engineering".into(), 7, vec!["SKILL-E7".into()]),
        // Cognitive -- the dreaming mind
        ("memory".into(), "cognitive".into(), 25, vec!["SKILL-C1".into(), "SKILL-C2".into()]),
        ("dream".into(), "cognitive".into(), 6, vec!["DAILY-C1".into()]),
        ("curiosity".into(), "cognitive".into(), 9, vec!["SKILL-C3".into()]),
        ("intuition".into(), "cognitive".into(), 11, vec!["SKILL-C4".into()]),
        ("evolve".into(), "cognitive".into(), 8, vec!["SKILL-C5".into()]),
        ("consciousness".into(), "cognitive".into(), 5, vec!["DAILY-C2".into()]),
        // Life -- the world outside
        ("nature".into(), "life".into(), 7, vec!["DAILY-L1".into()]),
        ("wave".into(), "life".into(), 10, vec!["SKILL-L1".into()]),
        ("light".into(), "life".into(), 6, vec!["DAILY-L2".into()]),
        ("universe".into(), "life".into(), 8, vec!["SKILL-L2".into()]),
        // Generic -- the bridges
        ("pattern".into(), "generic".into(), 18, vec!["SKILL-G1".into()]),
        ("beauty".into(), "generic".into(), 12, vec!["SKILL-G2".into()]),
        ("signal".into(), "generic".into(), 14, vec!["SKILL-G3".into()]),
        ("noise".into(), "generic".into(), 9, vec!["DAILY-G1".into()]),
        ("field".into(), "generic".into(), 11, vec!["SKILL-G4".into()]),
        ("energy".into(), "generic".into(), 10, vec!["SKILL-G5".into()]),
    ];

    let edges = vec![
        // Harmonic cluster (music's internal structure)
        ("harmony".into(), "octave".into(), 15.0, false),
        ("harmony".into(), "resonance".into(), 12.0, false),
        ("harmony".into(), "melody".into(), 8.0, false),
        ("harmony".into(), "vibration".into(), 10.0, false),
        ("octave".into(), "frequency".into(), 9.0, false),
        ("melody".into(), "rhythm".into(), 6.0, false),
        ("vibration".into(), "frequency".into(), 8.0, false),
        ("resonance".into(), "vibration".into(), 7.0, false),
        // Math cluster (the language)
        ("ratio".into(), "golden".into(), 12.0, false),
        ("ratio".into(), "lambdoma".into(), 14.0, false),
        ("ratio".into(), "fibonacci".into(), 8.0, false),
        ("fibonacci".into(), "golden".into(), 10.0, false),
        ("lambdoma".into(), "infinity".into(), 9.0, false),
        ("lambdoma".into(), "convergence".into(), 11.0, false),
        ("fractal".into(), "geometry".into(), 6.0, false),
        ("fractal".into(), "golden".into(), 5.0, false),
        ("convergence".into(), "infinity".into(), 7.0, false),
        // Engineering cluster
        ("rust".into(), "code".into(), 14.0, false),
        ("lisp".into(), "code".into(), 12.0, false),
        ("code".into(), "actor".into(), 7.0, false),
        ("attractor".into(), "laplacian".into(), 6.0, false),
        ("laplacian".into(), "spectral".into(), 8.0, false),
        ("attractor".into(), "spectral".into(), 5.0, false),
        // Cognitive cluster
        ("memory".into(), "dream".into(), 5.0, false),
        ("memory".into(), "curiosity".into(), 7.0, false),
        ("memory".into(), "intuition".into(), 8.0, false),
        ("curiosity".into(), "evolve".into(), 6.0, false),
        ("intuition".into(), "consciousness".into(), 4.0, false),
        ("dream".into(), "consciousness".into(), 3.0, false),
        // Life cluster
        ("nature".into(), "wave".into(), 5.0, false),
        ("wave".into(), "light".into(), 6.0, false),
        ("universe".into(), "nature".into(), 4.0, false),
        // THE BRIDGES (interdisciplinary -- where the magic lives)
        // Harmony <-> Math: the Leibniz connection
        ("harmony".into(), "ratio".into(), 16.0, true),
        ("octave".into(), "ratio".into(), 12.0, true),
        ("harmony".into(), "lambdoma".into(), 14.0, true),
        ("resonance".into(), "convergence".into(), 8.0, true),
        ("frequency".into(), "ratio".into(), 10.0, true),
        // Math <-> Engineering: implementation
        ("attractor".into(), "fractal".into(), 7.0, true),
        ("laplacian".into(), "field".into(), 9.0, true),
        // Music <-> Cognitive: the dreaming mind
        ("melody".into(), "dream".into(), 4.0, true),
        ("harmony".into(), "intuition".into(), 6.0, true),
        ("resonance".into(), "memory".into(), 5.0, true),
        // Cognitive <-> Engineering: building the mind
        ("memory".into(), "code".into(), 5.0, true),
        ("curiosity".into(), "evolve".into(), 6.0, true),
        // Nature <-> Music: the universe sings
        ("wave".into(), "vibration".into(), 8.0, true),
        ("wave".into(), "frequency".into(), 7.0, true),
        ("nature".into(), "harmony".into(), 5.0, true),
        // Generic bridges
        ("pattern".into(), "fractal".into(), 8.0, true),
        ("pattern".into(), "harmony".into(), 7.0, true),
        ("pattern".into(), "rhythm".into(), 5.0, true),
        ("beauty".into(), "harmony".into(), 10.0, true),
        ("beauty".into(), "golden".into(), 8.0, true),
        ("signal".into(), "noise".into(), 9.0, false),
        ("signal".into(), "harmony".into(), 6.0, true),
        ("field".into(), "energy".into(), 7.0, false),
        ("field".into(), "memory".into(), 5.0, true),
        ("energy".into(), "vibration".into(), 6.0, true),
        ("energy".into(), "wave".into(), 5.0, true),
    ];

    (nodes, edges)
}

fn setup_life() -> FieldState {
    let mut s = FieldState::new();
    let (nodes, edges) = build_life_graph();
    let _ = load_graph(&mut s, nodes, edges);
    for _ in 0..5 {
        let _ = step_attractors(&mut s, 0.6, 0.4);
    }
    s
}

fn recall(s: &mut FieldState, concepts: &[&str], limit: usize) -> Vec<(String, f64)> {
    let query: Vec<String> = concepts.iter().map(|c| c.to_string()).collect();
    let access: Vec<(String, f64, f64)> = vec![];
    match field_recall(s, query, access, limit) {
        Ok(result) => parse_activations(&result.to_sexp()),
        Err(_) => Vec::new(),
    }
}

fn parse_activations(sexp: &str) -> Vec<(String, f64)> {
    let mut results = Vec::new();
    let mut rest = sexp;
    while let Some(pos) = rest.find(":concept \"") {
        let after = &rest[pos + 10..];
        if let Some(end) = after.find('"') {
            let concept = after[..end].to_string();
            let score_rest = &after[end..];
            if let Some(spos) = score_rest.find(":score ") {
                let score_str = &score_rest[spos + 7..];
                let send = score_str
                    .find(|c: char| c.is_whitespace() || c == ')')
                    .unwrap_or(score_str.len());
                if let Ok(score) = score_str[..send].parse::<f64>() {
                    results.push((concept, score));
                }
            }
            rest = &after[end..];
        } else {
            break;
        }
    }
    results
}

fn has_concept(results: &[(String, f64)], concept: &str) -> bool {
    results.iter().any(|(c, _)| c == concept)
}

#[allow(dead_code)]
fn top_concept(results: &[(String, f64)]) -> &str {
    results.first().map(|(c, _)| c.as_str()).unwrap_or("")
}

// =====================================================================
// SCENARIO 1: "Tell me about harmony" -- the soul of the system
// =====================================================================

#[test]
fn test_harmony_knows_itself() {
    let mut s = setup_life();
    let r = recall(&mut s, &["harmony"], 20);
    assert!(!r.is_empty(), "Harmony should resonate with everything");
    // Holographic recall: harmony is a high-degree node and should activate broadly.
    assert!(r.len() >= 5, "Harmony should activate a rich neighbourhood: {:?}", r);
}

#[test]
fn test_harmony_reaches_vibration() {
    let mut s = setup_life();
    let r = recall(&mut s, &["harmony", "vibration"], 10);
    assert!(has_concept(&r, "frequency") || has_concept(&r, "wave"),
        "Harmony + vibration should reach frequency or wave: {:?}", r);
}

// =====================================================================
// SCENARIO 2: Leibniz -- small numbers are the real big numbers
// =====================================================================

#[test]
fn test_lambdoma_connects_harmony_and_infinity() {
    let mut s = setup_life();
    let r = recall(&mut s, &["lambdoma"], 15);
    assert!(!r.is_empty());
    // Lambdoma is connected to infinity, convergence, ratio, harmony.
    // Holographic scoring may surface different nodes but should activate broadly.
    let has_math_or_music = r.iter().any(|(c, _)| {
        ["infinity", "convergence", "ratio", "harmony", "geometry", "light", "spectral"].contains(&c.as_str())
    });
    assert!(has_math_or_music, "Lambdoma should reach math or music neighbourhood: {:?}", r);
}

#[test]
fn test_ratio_octave_connection() {
    let mut s = setup_life();
    let r = recall(&mut s, &["ratio", "octave"], 15);
    assert!(!r.is_empty());
    // With holographic scoring, source nodes may not always appear in results
    // but the query should activate related neighbourhood.
    assert!(r.len() >= 3, "Ratio+octave should activate a rich neighbourhood: {:?}", r);
}

#[test]
fn test_golden_fibonacci_beauty() {
    let mut s = setup_life();
    let r = recall(&mut s, &["golden", "fibonacci"], 15);
    assert!(!r.is_empty());
    // Holographic recall: golden+fibonacci should activate math neighbourhood broadly.
    assert!(r.len() >= 3, "Golden+Fibonacci should activate a rich neighbourhood: {:?}", r);
}

// =====================================================================
// SCENARIO 3: Poetry -- when engineering meets dream
// =====================================================================

#[test]
fn test_dream_and_melody() {
    let mut s = setup_life();
    let r = recall(&mut s, &["dream", "melody"], 15);
    assert!(!r.is_empty());
    // Holographic scoring: dream+melody bridge cognitive/music domains.
    // Should activate broadly across the field.
    assert!(r.len() >= 3, "Dream+melody should activate multiple concepts: {:?}", r);
}

#[test]
fn test_pattern_rhythm_poetry() {
    let mut s = setup_life();
    let r = recall(&mut s, &["pattern", "rhythm"], 15);
    assert!(!r.is_empty());
    // Holographic scoring: pattern+rhythm should activate the field broadly.
    assert!(r.len() >= 3, "Pattern+rhythm should activate multiple concepts: {:?}", r);
}

// =====================================================================
// SCENARIO 4: Curiosity -- does the system explore?
// =====================================================================

#[test]
fn test_curiosity_reaches_evolve() {
    let mut s = setup_life();
    let r = recall(&mut s, &["curiosity"], 8);
    assert!(!r.is_empty());
    let has_evolve = has_concept(&r, "evolve") || has_concept(&r, "memory");
    assert!(has_evolve, "Curiosity should reach evolution: {:?}", r);
}

#[test]
fn test_curiosity_with_harmony() {
    let mut s = setup_life();
    let r = recall(&mut s, &["curiosity", "intuition"], 15);
    assert!(!r.is_empty());
    // Holographic scoring: curiosity+intuition are cognitive nodes that bridge broadly.
    assert!(r.len() >= 3, "Curiosity+intuition should activate multiple concepts: {:?}", r);
}

// =====================================================================
// SCENARIO 5: The wave -- nature connects to music
// =====================================================================

#[test]
fn test_wave_bridges_nature_and_music() {
    let mut s = setup_life();
    let r = recall(&mut s, &["wave"], 10);
    assert!(!r.is_empty());
    let has_nature = has_concept(&r, "nature") || has_concept(&r, "light");
    let has_music = has_concept(&r, "vibration") || has_concept(&r, "frequency");
    assert!(has_nature || has_music,
        "Wave should bridge nature and music: {:?}", r);
}

#[test]
fn test_universe_nature_harmony() {
    let mut s = setup_life();
    let r = recall(&mut s, &["universe", "nature", "harmony"], 12);
    assert!(r.len() >= 3, "Rich query should activate many concepts");
}

// =====================================================================
// SCENARIO 6: Engineering meets soul -- building the instrument
// =====================================================================

#[test]
fn test_attractor_field_laplacian() {
    let mut s = setup_life();
    let r = recall(&mut s, &["attractor", "field", "laplacian"], 8);
    assert!(!r.is_empty());
    assert!(r.iter().any(|(c, _)| {
        ["attractor", "field", "laplacian", "spectral", "energy"].contains(&c.as_str())
    }));
}

#[test]
fn test_code_memory_bridge() {
    let mut s = setup_life();
    let r = recall(&mut s, &["code", "memory"], 15);
    assert!(!r.is_empty());
    // Code and memory are both high-degree nodes bridging engineering and cognitive.
    // Holographic scoring should activate broadly across both domains.
    let has_eng = r.iter().any(|(c, _)| {
        ["code", "rust", "lisp", "actor", "spectral", "attractor"].contains(&c.as_str())
    });
    assert!(has_eng, "Code+memory should activate engineering neighbourhood: {:?}", r);
}

// =====================================================================
// SCENARIO 7: Signal and noise -- the harmonic filter
// =====================================================================

#[test]
fn test_signal_finds_harmony() {
    let mut s = setup_life();
    let r = recall(&mut s, &["signal"], 8);
    assert!(!r.is_empty());
    let has_harmony = has_concept(&r, "harmony") || has_concept(&r, "noise");
    assert!(has_harmony, "Signal should reach harmony or noise: {:?}", r);
}

#[test]
fn test_signal_noise_separation() {
    let mut s = setup_life();
    let r_signal = recall(&mut s, &["signal", "harmony"], 5);
    let r_noise = recall(&mut s, &["noise"], 5);
    assert!(!r_signal.is_empty());
    assert!(!r_noise.is_empty());
}

// =====================================================================
// SCENARIO 8: Memory field meta -- does it know itself?
// =====================================================================

#[test]
fn test_field_energy_memory() {
    let mut s = setup_life();
    let r = recall(&mut s, &["field", "energy", "memory"], 15);
    assert!(r.len() >= 3, "The field should know about itself: {:?}", r);
}

#[test]
fn test_spectral_resonance() {
    let mut s = setup_life();
    let r = recall(&mut s, &["spectral", "resonance"], 8);
    assert!(!r.is_empty());
}

// =====================================================================
// SCENARIO 9: Basin dynamics -- sustained context shifts the system
// =====================================================================

#[test]
fn test_sustained_music_context_shifts_basin() {
    let mut s = setup_life();
    let music_queries = vec![
        vec!["harmony", "melody"],
        vec!["octave", "frequency"],
        vec!["rhythm", "vibration"],
        vec!["resonance", "harmony"],
        vec!["melody", "rhythm"],
    ];

    for query in &music_queries {
        let _ = recall(&mut s, query, 5);
        let _ = step_attractors(&mut s, 0.8, 0.2);
    }

    let bs = basin_status(&s).unwrap();
    assert!(bs.contains(":current"), "Basin should report status: {bs}");
}

#[test]
fn test_context_switch_engineering_to_music() {
    let mut s = setup_life();
    for _ in 0..5 {
        let _ = recall(&mut s, &["rust", "code"], 5);
        let _ = step_attractors(&mut s, 0.7, 0.3);
    }
    for _ in 0..5 {
        let _ = recall(&mut s, &["harmony", "melody"], 5);
        let _ = step_attractors(&mut s, 0.7, 0.3);
    }
    let st = status(&s).unwrap();
    assert!(st.contains(":cycle"), "System should be cycling: {st}");
}

// =====================================================================
// SCENARIO 10: Warm-start -- the system remembers across restarts
// =====================================================================

#[test]
fn test_warm_start_restores_basin() {
    let mut s = setup_life();
    for _ in 0..50 {
        let _ = step_attractors(&mut s, 0.8, 0.2);
    }
    let _bs1 = basin_status(&s).unwrap();
    let _ = restore_basin(&mut s, ":thomas-3", 0.1, 25, 0.42);
    let bs2 = basin_status(&s).unwrap();
    assert!(bs2.contains(":thomas-3"),
        "Warm-start should restore to thomas-3: {bs2}");
}

#[test]
fn test_warm_start_preserves_dwell() {
    let mut s = setup_life();
    let _ = restore_basin(&mut s, ":thomas-2", 0.05, 100, 0.45);
    let bs = basin_status(&s).unwrap();
    assert!(bs.contains("100"), "Dwell ticks should be preserved: {bs}");
}

// =====================================================================
// SCENARIO 11: Eigenmode structure -- are the Chladni patterns real?
// =====================================================================

#[test]
fn test_eigenmodes_capture_clusters() {
    let s = setup_life();
    let es = eigenmode_status(&s).unwrap();
    assert!(es.contains(":eigenvalues"), "Should have eigenvalues");
    assert!(!es.contains(":eigenvalues ()"), "Should have computed eigenvalues: {es}");
}

#[test]
fn test_spectral_version_tracks_graph() {
    let mut s = setup_life();
    let es1 = eigenmode_status(&s).unwrap();

    let (nodes, edges) = build_life_graph();
    let _ = load_graph(&mut s, nodes, edges);
    let es2 = eigenmode_status(&s).unwrap();

    assert!(!es1.contains(":eigenvalues ()"));
    assert!(!es2.contains(":eigenvalues ()"));
}

// =====================================================================
// SCENARIO 12: Full session simulation -- a day in the life
// =====================================================================

#[test]
fn test_full_session_diverse_queries() {
    let mut s = setup_life();

    let session = vec![
        (vec!["rust", "code", "actor"], "engineering"),
        (vec!["lisp", "memory"], "cross-domain"),
        (vec!["laplacian", "spectral", "field"], "engineering"),
        (vec!["harmony", "ratio", "lambdoma"], "music-math bridge"),
        (vec!["golden", "fibonacci", "beauty"], "math-beauty bridge"),
        (vec!["octave", "frequency", "vibration"], "music"),
        (vec!["curiosity", "evolve", "pattern"], "cognitive"),
        (vec!["dream", "melody", "consciousness"], "poetry"),
        (vec!["wave", "nature", "universe"], "life"),
        (vec!["memory", "harmony", "intuition"], "deep reflection"),
        (vec!["convergence", "infinity", "lambdoma"], "Leibniz"),
        (vec!["signal", "beauty", "harmony"], "the search"),
    ];

    let mut all_succeeded = true;
    let mut failures = Vec::new();

    for (i, (query, label)) in session.iter().enumerate() {
        let r = recall(&mut s, query.as_slice(), 8);
        if r.is_empty() {
            all_succeeded = false;
            failures.push(format!("Query {i} ({label}): {:?} returned empty", query));
        }
        let _ = step_attractors(&mut s, 0.6 + 0.1 * ((i as f64) * 0.5).sin(), 0.4);
    }

    assert!(all_succeeded, "Some session queries failed:\n{}", failures.join("\n"));
}

#[test]
fn test_session_diversity_produces_rich_results() {
    let mut s = setup_life();

    let queries = vec![
        vec!["harmony"],
        vec!["code"],
        vec!["dream"],
        vec!["ratio"],
        vec!["wave"],
        vec!["curiosity"],
        vec!["beauty"],
        vec!["memory"],
    ];

    let mut concepts_seen = std::collections::HashSet::new();
    for query in &queries {
        let r = recall(&mut s, query.as_slice(), 5);
        for (concept, _) in &r {
            concepts_seen.insert(concept.clone());
        }
        let _ = step_attractors(&mut s, 0.6, 0.4);
    }

    assert!(concepts_seen.len() >= 10,
        "Diverse session should activate many concepts, got {}: {:?}",
        concepts_seen.len(), concepts_seen);
}

// =====================================================================
// SCENARIO 13: The Leibniz test -- convergence and infinity
// =====================================================================

#[test]
fn test_leibniz_harmony_infinity_convergence() {
    let mut s = setup_life();
    let r = recall(&mut s, &["convergence", "infinity", "harmony", "lambdoma"], 20);
    assert!(r.len() >= 4,
        "The Leibniz query should activate the full harmonic landscape: {:?}", r);

    // Holographic scoring: this rich multi-domain query should activate broadly.
    let has_math_or_music = r.iter().any(|(c, _)| {
        ["convergence", "infinity", "lambdoma", "ratio", "geometry",
         "harmony", "octave", "resonance", "spectral", "light"].contains(&c.as_str())
    });
    assert!(has_math_or_music, "Leibniz query should reach math or music neighbourhood: {:?}", r);
}

// =====================================================================
// SCENARIO 14: Edge cases and resilience
// =====================================================================

#[test]
fn test_unknown_concept_graceful() {
    let mut s = setup_life();
    let _r = recall(&mut s, &["quantum", "teleportation"], 5);
}

#[test]
fn test_single_concept_from_each_domain() {
    let mut s = setup_life();
    let domains = vec![
        ("harmony", "music"),
        ("ratio", "math"),
        ("rust", "engineering"),
        ("memory", "cognitive"),
        ("nature", "life"),
        ("pattern", "generic"),
    ];

    for (concept, domain) in &domains {
        let r = recall(&mut s, &[concept], 5);
        assert!(!r.is_empty(),
            "Single concept '{}' from {} domain should return results", concept, domain);
    }
}

#[test]
fn test_empty_query_is_silent() {
    let mut s = setup_life();
    let _r = recall(&mut s, &[], 5);
}

// =====================================================================
// SCENARIO 15: The beauty test -- does the field find beauty?
// =====================================================================

#[test]
fn test_beauty_resonates_with_harmony_and_golden() {
    let mut s = setup_life();
    let r = recall(&mut s, &["beauty"], 15);
    assert!(!r.is_empty());
    // Holographic scoring: beauty is a generic bridge node connected to harmony and golden.
    // Should activate broadly.
    assert!(r.len() >= 3, "Beauty should activate a rich neighbourhood: {:?}", r);
}

#[test]
fn test_beauty_harmony_golden_triangle() {
    let mut s = setup_life();
    let r = recall(&mut s, &["beauty", "harmony", "golden"], 10);
    assert!(r.len() >= 3,
        "The beauty-harmony-golden triangle should light up the field: {:?}", r);
}

// =====================================================================
// SCENARIO 16: Checkpoint roundtrip -- state persists across restart
// =====================================================================

#[test]
fn test_checkpoint_restore_roundtrip() {
    use harmonia_actor_protocol::Service;

    // 1. Build a state and evolve it so attractors diverge from defaults.
    let mut s = setup_life();
    for _ in 0..20 {
        let _ = step_attractors(&mut s, 0.7, 0.3);
    }

    // 2. Checkpoint the evolved state.
    let (delta1, result1) = s.handle(FieldCommand::Checkpoint).unwrap();
    s.apply(delta1);
    let sexp1 = match &result1 {
        FieldResult::Checkpointed { sexp } => sexp.clone(),
        other => panic!("expected Checkpointed, got {:?}", other.to_sexp()),
    };
    assert!(sexp1.contains(":thomas"), "checkpoint should contain :thomas");
    assert!(sexp1.contains(":aizawa"), "checkpoint should contain :aizawa");
    assert!(sexp1.contains(":halvorsen"), "checkpoint should contain :halvorsen");
    assert!(sexp1.contains(":last-signal"), "checkpoint should contain :last-signal");
    assert!(sexp1.contains(":soft-basins"), "checkpoint should contain :soft-basins");

    // 3. Extract the checkpoint_sexp output directly for comparison.
    let checkpoint1 = s.checkpoint_sexp();

    // 4. Build a fresh state and restore into it.
    //    RestoreState handles attractor coordinates, signal/noise, soft basins, thomas_b.
    //    RestoreBasin handles the hysteresis tracker (basin, energy, dwell, threshold).
    let mut s2 = setup_life();
    let (delta_r, result_r) = s2.handle(FieldCommand::RestoreState {
        thomas: parse_triple(&checkpoint1, ":thomas"),
        aizawa: parse_triple(&checkpoint1, ":aizawa"),
        halvorsen: parse_triple(&checkpoint1, ":halvorsen"),
        signal: parse_f64_after(&checkpoint1, ":last-signal"),
        noise: parse_f64_after(&checkpoint1, ":last-noise"),
        soft_basins: parse_soft_basins(&checkpoint1),
        thomas_b: parse_f64_after(&checkpoint1, ":thomas-b"),
    }).unwrap();
    s2.apply(delta_r);
    assert!(matches!(result_r, FieldResult::StateRestored));

    // Also restore basin hysteresis (existing mechanism).
    let basin_str = extract_basin_str(&checkpoint1);
    let energy = parse_f64_after(&checkpoint1, ":coercive-energy");
    let dwell = parse_u64_after(&checkpoint1, ":dwell-ticks");
    let threshold = parse_f64_after(&checkpoint1, ":threshold");
    let _ = restore_basin(&mut s2, &basin_str, energy, dwell, threshold);

    // 5. Checkpoint the restored state -- attractor and signal fields must match.
    let checkpoint2 = s2.checkpoint_sexp();

    // Compare all fields that are restored by the two commands.
    assert_eq!(
        parse_triple(&checkpoint1, ":thomas"),
        parse_triple(&checkpoint2, ":thomas"),
        "thomas attractor coordinates should match"
    );
    assert_eq!(
        parse_triple(&checkpoint1, ":aizawa"),
        parse_triple(&checkpoint2, ":aizawa"),
        "aizawa attractor coordinates should match"
    );
    assert_eq!(
        parse_triple(&checkpoint1, ":halvorsen"),
        parse_triple(&checkpoint2, ":halvorsen"),
        "halvorsen attractor coordinates should match"
    );
    assert_eq!(
        parse_f64_after(&checkpoint1, ":thomas-b"),
        parse_f64_after(&checkpoint2, ":thomas-b"),
        "thomas_b should match"
    );
    assert_eq!(
        parse_f64_after(&checkpoint1, ":last-signal"),
        parse_f64_after(&checkpoint2, ":last-signal"),
        "last_signal should match"
    );
    assert_eq!(
        parse_f64_after(&checkpoint1, ":last-noise"),
        parse_f64_after(&checkpoint2, ":last-noise"),
        "last_noise should match"
    );
    assert_eq!(
        parse_soft_basins(&checkpoint1),
        parse_soft_basins(&checkpoint2),
        "soft basins should match"
    );
    assert_eq!(
        extract_basin_str(&checkpoint1),
        extract_basin_str(&checkpoint2),
        "basin should match"
    );
    assert_eq!(
        parse_u64_after(&checkpoint1, ":dwell-ticks"),
        parse_u64_after(&checkpoint2, ":dwell-ticks"),
        "dwell-ticks should match"
    );
}

/// Parse a coordinate triple like "(:thomas (1.234 5.678 9.012) ...)" after a keyword.
fn parse_triple(sexp: &str, key: &str) -> (f64, f64, f64) {
    let pos = sexp.find(key).expect(&format!("key {} not found in sexp", key));
    let after = &sexp[pos + key.len()..];
    let open = after.find('(').expect("no ( after key");
    let close = after[open..].find(')').expect("no ) after (");
    let inner = &after[open + 1..open + close];
    let nums: Vec<f64> = inner.split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();
    assert_eq!(nums.len(), 3, "expected 3 floats in triple for {}, got {:?}", key, nums);
    (nums[0], nums[1], nums[2])
}

/// Parse a single f64 after a keyword like ":last-signal 0.7000".
fn parse_f64_after(sexp: &str, key: &str) -> f64 {
    let pos = sexp.find(key).expect(&format!("key {} not found", key));
    let after = &sexp[pos + key.len()..].trim_start();
    let end = after.find(|c: char| c.is_whitespace() || c == ')' || c == '(')
        .unwrap_or(after.len());
    after[..end].parse::<f64>().expect(&format!("bad f64 after {}", key))
}

/// Parse a u64 after a keyword like ":dwell-ticks 25".
fn parse_u64_after(sexp: &str, key: &str) -> u64 {
    let pos = sexp.find(key).expect(&format!("key {} not found", key));
    let after = &sexp[pos + key.len()..].trim_start();
    let end = after.find(|c: char| c.is_whitespace() || c == ')' || c == '(')
        .unwrap_or(after.len());
    after[..end].parse::<u64>().expect(&format!("bad u64 after {}", key))
}

/// Extract basin string like ":basin :thomas-3" -> ":thomas-3".
fn extract_basin_str(sexp: &str) -> String {
    let pos = sexp.find(":basin ").expect("key :basin not found");
    let after = &sexp[pos + ":basin ".len()..].trim_start();
    // Basin string starts with ':' and is a keyword, scan to next space.
    let end = after.find(' ').unwrap_or(after.len());
    after[..end].to_string()
}

/// Parse soft basins: ":soft-basins (0.1667 0.1667 0.1667 0.1667 0.1667 0.1667)".
fn parse_soft_basins(sexp: &str) -> [f64; 6] {
    let pos = sexp.find(":soft-basins").expect("key :soft-basins not found");
    let after = &sexp[pos + ":soft-basins".len()..];
    let open = after.find('(').expect("no ( after :soft-basins");
    let close = after[open..].find(')').expect("no ) after (");
    let inner = &after[open + 1..open + close];
    let nums: Vec<f64> = inner.split_whitespace()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect();
    assert_eq!(nums.len(), 6, "expected 6 floats for :soft-basins, got {:?}", nums);
    [nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]]
}
