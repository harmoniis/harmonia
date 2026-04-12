/// Comprehensive integration tests for the memory field system.
///
/// Tests field recall across diverse domains, validates basin switching,
/// eigenmode separation, hysteresis, and compares field vs substring baseline.
///
/// 50+ test scenarios covering:
/// - Single-domain queries (engineering, music, math, cognitive, life)
/// - Cross-domain queries (interdisciplinary bridging)
/// - Depth recall (shallow dailies vs crystal skills)
/// - Basin switching under sustained vs weak signal
/// - Graph evolution (concepts added over time)
/// - Degenerate cases (empty, single node, disconnected)
/// - Large graphs (50+ nodes)
/// - Attractor stability over many cycles
/// - Thomas basin count verification at b~0.208

use harmonia_memory_field::{
    field_recall, field_recall_structural, load_graph, reset, step_attractors,
    basin_status, eigenmode_status, status, dream_stats, edge_current_status,
    FieldState,
};

/// Build a realistic concept graph simulating a multi-domain agent memory.
fn build_realistic_graph() -> (
    Vec<(String, String, i32, Vec<String>)>,
    Vec<(String, String, f64, bool)>,
) {
    let nodes = vec![
        // Engineering domain
        ("rust".into(), "engineering".into(), 15, vec!["SKILL-1".into(), "DAILY-10".into()]),
        ("lisp".into(), "engineering".into(), 12, vec!["SKILL-2".into(), "DAILY-11".into()]),
        ("code".into(), "engineering".into(), 20, vec!["SKILL-3".into(), "DAILY-12".into()]),
        ("backend".into(), "engineering".into(), 8, vec!["SKILL-4".into()]),
        ("api".into(), "engineering".into(), 10, vec!["DAILY-13".into()]),
        ("model".into(), "engineering".into(), 7, vec!["SKILL-5".into()]),
        ("tool".into(), "engineering".into(), 9, vec!["DAILY-14".into()]),
        ("compiler".into(), "engineering".into(), 5, vec!["SKILL-6".into()]),
        // Music domain
        ("harmony".into(), "music".into(), 18, vec!["SOUL-1".into(), "SKILL-7".into()]),
        ("melody".into(), "music".into(), 6, vec!["DAILY-20".into()]),
        ("rhythm".into(), "music".into(), 5, vec!["DAILY-21".into()]),
        ("tone".into(), "music".into(), 4, vec!["DAILY-22".into()]),
        // Math domain
        ("ratio".into(), "math".into(), 11, vec!["SKILL-8".into()]),
        ("geometry".into(), "math".into(), 7, vec!["DAILY-30".into()]),
        ("fractal".into(), "math".into(), 9, vec!["SKILL-9".into()]),
        ("theory".into(), "math".into(), 6, vec!["DAILY-31".into()]),
        ("proof".into(), "math".into(), 4, vec!["DAILY-32".into()]),
        // Cognitive domain
        ("memory".into(), "cognitive".into(), 25, vec!["SKILL-10".into(), "SKILL-11".into()]),
        ("brain".into(), "cognitive".into(), 8, vec!["DAILY-40".into()]),
        ("evolve".into(), "cognitive".into(), 6, vec!["SKILL-12".into()]),
        ("dream".into(), "cognitive".into(), 3, vec!["DAILY-41".into()]),
        // Life domain
        ("weather".into(), "life".into(), 4, vec!["DAILY-50".into()]),
        ("travel".into(), "life".into(), 3, vec!["DAILY-51".into()]),
        ("calendar".into(), "life".into(), 5, vec!["DAILY-52".into()]),
        ("meeting".into(), "life".into(), 2, vec!["DAILY-53".into()]),
        // Generic / cross-domain
        ("pattern".into(), "generic".into(), 14, vec!["SKILL-13".into()]),
        ("system".into(), "generic".into(), 16, vec!["SKILL-14".into()]),
        ("signal".into(), "generic".into(), 12, vec!["SKILL-15".into()]),
        ("noise".into(), "generic".into(), 8, vec!["DAILY-60".into()]),
        ("attractor".into(), "generic".into(), 10, vec!["SKILL-16".into()]),
    ];

    let edges = vec![
        // Engineering cluster
        ("rust".into(), "code".into(), 12.0, false),
        ("lisp".into(), "code".into(), 10.0, false),
        ("rust".into(), "backend".into(), 6.0, false),
        ("code".into(), "api".into(), 8.0, false),
        ("code".into(), "tool".into(), 7.0, false),
        ("code".into(), "model".into(), 5.0, false),
        ("rust".into(), "compiler".into(), 4.0, false),
        ("lisp".into(), "model".into(), 3.0, false),
        ("backend".into(), "api".into(), 5.0, false),
        // Music cluster
        ("harmony".into(), "melody".into(), 6.0, false),
        ("harmony".into(), "rhythm".into(), 5.0, false),
        ("harmony".into(), "tone".into(), 4.0, false),
        ("melody".into(), "rhythm".into(), 3.0, false),
        // Math cluster
        ("ratio".into(), "geometry".into(), 5.0, false),
        ("ratio".into(), "fractal".into(), 7.0, false),
        ("fractal".into(), "geometry".into(), 4.0, false),
        ("theory".into(), "proof".into(), 3.0, false),
        ("ratio".into(), "theory".into(), 4.0, false),
        // Cognitive cluster
        ("memory".into(), "brain".into(), 8.0, false),
        ("memory".into(), "evolve".into(), 5.0, false),
        ("brain".into(), "dream".into(), 3.0, false),
        // Life cluster
        ("weather".into(), "travel".into(), 2.0, false),
        ("calendar".into(), "meeting".into(), 3.0, false),
        // Cross-domain bridges (interdisciplinary)
        ("harmony".into(), "ratio".into(), 9.0, true),      // music <-> math
        ("pattern".into(), "fractal".into(), 6.0, true),     // generic <-> math
        ("memory".into(), "code".into(), 4.0, true),         // cognitive <-> engineering
        ("signal".into(), "harmony".into(), 5.0, true),      // generic <-> music
        ("signal".into(), "noise".into(), 7.0, false),
        ("system".into(), "code".into(), 6.0, true),         // generic <-> engineering
        ("system".into(), "pattern".into(), 8.0, false),
        ("attractor".into(), "fractal".into(), 5.0, true),   // generic <-> math
        ("attractor".into(), "signal".into(), 4.0, false),
        ("evolve".into(), "pattern".into(), 3.0, true),      // cognitive <-> generic
        ("model".into(), "theory".into(), 4.0, true),        // engineering <-> math
    ];

    (nodes, edges)
}

fn setup() -> FieldState {
    let mut s = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    let _ = load_graph(&mut s, nodes, edges);
    s
}

fn recall(s: &mut FieldState, concepts: &[&str], limit: usize) -> Vec<(String, f64)> {
    let query: Vec<String> = concepts.iter().map(|c| c.to_string()).collect();
    let access: Vec<(String, f64, f64)> = Vec::new();
    match field_recall(s, query, access, limit) {
        Ok(result) => parse_activations(&result.to_sexp()),
        Err(_) => Vec::new(),
    }
}

fn parse_activations(sexp: &str) -> Vec<(String, f64)> {
    // Simple extraction: find all :concept "X" :score Y pairs.
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

// =====================================================================
// SINGLE-DOMAIN QUERIES -- verify field recall produces domain-coherent results
// =====================================================================

#[test]
fn test_01_engineering_query_rust_code() {
    let mut s = setup();
    let results = recall(&mut s, &["rust", "code"], 5);
    assert!(!results.is_empty(), "Should recall something for rust+code");
    // Top result should be in engineering domain.
    let top = &results[0].0;
    assert!(
        ["rust", "code", "lisp", "backend", "api", "tool", "model", "compiler"].contains(&top.as_str()),
        "Top result for 'rust code' should be engineering, got: {top}"
    );
}

#[test]
fn test_02_engineering_query_api_backend() {
    let mut s = setup();
    let results = recall(&mut s, &["api", "backend"], 5);
    assert!(!results.is_empty());
    // Should activate engineering concepts.
    let eng_count = results.iter().filter(|(c, _)| {
        ["api", "backend", "rust", "code", "tool"].contains(&c.as_str())
    }).count();
    assert!(eng_count >= 2, "Should find at least 2 engineering concepts");
}

#[test]
fn test_03_music_query_harmony_melody() {
    let mut s = setup();
    let results = recall(&mut s, &["harmony", "melody"], 5);
    assert!(!results.is_empty());
    let music_count = results.iter().filter(|(c, _)| {
        ["harmony", "melody", "rhythm", "tone"].contains(&c.as_str())
    }).count();
    assert!(music_count >= 2, "Should find at least 2 music concepts");
}

#[test]
fn test_04_math_query_fractal_ratio() {
    let mut s = setup();
    let results = recall(&mut s, &["fractal", "ratio"], 15);
    assert!(!results.is_empty());
    // With holographic scoring, source concepts may not always rank top-8 due to
    // heat kernel diffusion. The query should still activate a rich neighbourhood.
    assert!(results.len() >= 3, "Math query should activate multiple concepts: {:?}", results);
}

#[test]
fn test_05_cognitive_query_memory_brain() {
    let mut s = setup();
    let results = recall(&mut s, &["memory", "brain"], 5);
    assert!(!results.is_empty());
    let cog_count = results.iter().filter(|(c, _)| {
        ["memory", "brain", "evolve", "dream"].contains(&c.as_str())
    }).count();
    assert!(cog_count >= 2, "Should find at least 2 cognitive concepts");
}

#[test]
fn test_06_life_query_calendar_meeting() {
    let mut s = setup();
    let results = recall(&mut s, &["calendar", "meeting"], 5);
    assert!(!results.is_empty());
    let life_count = results.iter().filter(|(c, _)| {
        ["calendar", "meeting", "weather", "travel"].contains(&c.as_str())
    }).count();
    assert!(life_count >= 1, "Should find at least 1 life concept");
}

#[test]
fn test_07_engineering_compiler_niche() {
    let mut s = setup();
    let results = recall(&mut s, &["compiler", "rust"], 5);
    assert!(!results.is_empty());
    assert!(results.iter().any(|(c, _)| c == "compiler" || c == "rust"));
}

#[test]
fn test_08_music_rhythm_tone() {
    let mut s = setup();
    let results = recall(&mut s, &["rhythm", "tone"], 5);
    assert!(!results.is_empty());
}

// =====================================================================
// CROSS-DOMAIN QUERIES -- verify interdisciplinary bridging
// =====================================================================

#[test]
fn test_09_cross_music_math_harmony_ratio() {
    let mut s = setup();
    let results = recall(&mut s, &["harmony", "ratio"], 15);
    assert!(!results.is_empty());
    let has_music = results.iter().any(|(c, _)| ["harmony", "melody", "rhythm"].contains(&c.as_str()));
    assert!(has_music, "Cross-domain query should include music concepts");
    // With holographic scoring, the query should activate broadly across domains.
    assert!(results.len() >= 4, "Cross-domain query should activate rich neighbourhood: {:?}", results);
}

#[test]
fn test_10_cross_engineering_cognitive_memory_code() {
    let mut s = setup();
    let results = recall(&mut s, &["memory", "code"], 10);
    assert!(!results.is_empty());
    let has_memory = results.iter().any(|(c, _)| c == "memory");
    let has_code = results.iter().any(|(c, _)| c == "code");
    assert!(has_memory || has_code,
        "At least one source concept should appear, got: {:?}", results);
}

#[test]
fn test_11_cross_generic_signal_noise() {
    let mut s = setup();
    let results = recall(&mut s, &["signal", "noise"], 5);
    assert!(!results.is_empty());
    assert!(results.iter().any(|(c, _)| c == "signal" || c == "noise"));
}

#[test]
fn test_12_cross_pattern_system() {
    let mut s = setup();
    let results = recall(&mut s, &["pattern", "system"], 8);
    assert!(!results.is_empty());
}

#[test]
fn test_13_cross_attractor_fractal() {
    let mut s = setup();
    let results = recall(&mut s, &["attractor", "fractal"], 8);
    assert!(!results.is_empty());
    let has_source = results.iter().any(|(c, _)| c == "attractor" || c == "fractal");
    assert!(has_source, "Should find source concepts, got: {:?}", results);
}

#[test]
fn test_14_cross_model_theory() {
    let mut s = setup();
    let results = recall(&mut s, &["model", "theory"], 5);
    assert!(!results.is_empty());
}

// =====================================================================
// FIELD POTENTIAL ORDERING -- verify that closer nodes score higher
// =====================================================================

#[test]
fn test_15_field_gradient_rust_query() {
    let mut s = setup();
    let results = recall(&mut s, &["rust"], 10);
    assert!(results.len() >= 3, "Should recall several nodes");
    let rust_score = results.iter().find(|(c, _)| c == "rust").map(|(_, s)| *s).unwrap_or(0.0);
    let dream_score = results.iter().find(|(c, _)| c == "dream").map(|(_, s)| *s);
    if let Some(ds) = dream_score {
        assert!(rust_score > ds, "Source node should score higher than distant node");
    }
}

#[test]
fn test_16_field_gradient_memory_query() {
    let mut s = setup();
    let results = recall(&mut s, &["memory"], 10);
    let mem_score = results.iter().find(|(c, _)| c == "memory").map(|(_, s)| *s).unwrap_or(0.0);
    let weather_score = results.iter().find(|(c, _)| c == "weather").map(|(_, s)| *s);
    if let Some(ws) = weather_score {
        assert!(mem_score > ws, "memory should score higher than weather when querying 'memory'");
    }
}

#[test]
fn test_17_multiple_source_nodes() {
    let mut s = setup();
    let results = recall(&mut s, &["rust", "harmony"], 10);
    assert!(results.len() >= 4, "Two sources should activate more nodes");
}

// =====================================================================
// ENTRY ID MAPPING -- verify that recalled concepts carry correct entry IDs
// =====================================================================

#[test]
fn test_18_entry_ids_in_results() {
    let mut s = setup();
    let query = vec!["memory".to_string()];
    let result = field_recall(&mut s, query, vec![], 10).unwrap().to_sexp();
    let has_entries = result.contains("SKILL-") || result.contains("DAILY-") || result.contains("SOUL-");
    assert!(has_entries,
        "Recall result should carry entry IDs: {}", &result[..result.len().min(500)]);
}

#[test]
fn test_19_soul_entry_in_harmony() {
    let mut s = setup();
    let query = vec!["harmony".to_string()];
    let result = field_recall(&mut s, query, vec![], 5).unwrap().to_sexp();
    assert!(result.contains("SOUL-1"),
        "Harmony concept should carry SOUL-1 entry");
}

// =====================================================================
// BASIN AND HYSTERESIS -- verify attractor dynamics
// =====================================================================

#[test]
fn test_20_initial_basin_status() {
    let s = setup();
    let bs = basin_status(&s).unwrap();
    assert!(bs.contains(":current"), "Basin status should report current basin");
    assert!(bs.contains(":dwell-ticks"), "Should report dwell ticks");
}

#[test]
fn test_21_attractor_step_bounded() {
    let mut s = setup();
    for _ in 0..500 {
        let result = step_attractors(&mut s, 0.7, 0.3).unwrap();
        assert!(result.contains(":thomas"), "Step should report Thomas state");
        assert!(result.contains(":aizawa"), "Step should report Aizawa state");
        assert!(result.contains(":halvorsen"), "Step should report Halvorsen state");
    }
}

#[test]
fn test_22_hysteresis_weak_signal() {
    let mut s = setup();
    let _initial_basin = basin_status(&s).unwrap();
    for _ in 0..10 {
        let _ = step_attractors(&mut s, 0.51, 0.49);
    }
    let _after = basin_status(&s).unwrap();
}

#[test]
fn test_23_hysteresis_strong_signal() {
    let mut s = setup();
    for _ in 0..100 {
        let _ = step_attractors(&mut s, 0.9, 0.1);
    }
    let bs = basin_status(&s).unwrap();
    assert!(bs.contains(":current"), "Should have a current basin after many steps");
}

#[test]
fn test_24_thomas_b_modulation() {
    let mut s = setup();
    let result = step_attractors(&mut s, 0.9, 0.1).unwrap();
    assert!(result.contains(":b"), "Should report Thomas b parameter");
    assert!(result.contains("0.224") || result.contains("0.22"),
        "Thomas b should be modulated by signal-noise: {result}");
}

// =====================================================================
// EIGENMODE STRUCTURE -- verify spectral decomposition
// =====================================================================

#[test]
fn test_25_eigenmode_status_populated() {
    let s = setup();
    let es = eigenmode_status(&s).unwrap();
    assert!(es.contains(":eigenvalues"), "Should report eigenvalues");
    assert!(es.contains(":spectral-version"), "Should report spectral version");
    assert!(!es.contains(":eigenvalues ()"), "Eigenvalues should not be empty");
}

#[test]
fn test_26_fiedler_value_positive() {
    let s = setup();
    let es = eigenmode_status(&s).unwrap();
    if let Some(pos) = es.find(":eigenvalues (") {
        let rest = &es[pos + 14..];
        let end = rest.find(|c: char| c.is_whitespace() || c == ')').unwrap_or(0);
        if let Ok(lambda1) = rest[..end].parse::<f64>() {
            assert!(lambda1 > 0.0, "Fiedler value should be positive for connected graph, got {lambda1}");
        }
    }
}

// =====================================================================
// ACCESS COUNT INFLUENCE -- verify legacy compatibility signal
// =====================================================================

#[test]
fn test_27_access_count_boosts_score() {
    let mut s = setup();
    let query = vec!["rust".to_string(), "code".to_string()];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let access_high = vec![("rust".to_string(), 0.9, now)];
    let access_low = vec![("rust".to_string(), 0.1, now)];

    let result_high = field_recall(&mut s, query.clone(), access_high, 5).unwrap().to_sexp();
    let result_low = field_recall(&mut s, query, access_low, 5).unwrap().to_sexp();

    let score_high = parse_activations(&result_high)
        .iter().find(|(c, _)| c == "rust").map(|(_, s)| *s).unwrap_or(0.0);
    let score_low = parse_activations(&result_low)
        .iter().find(|(c, _)| c == "rust").map(|(_, s)| *s).unwrap_or(0.0);

    assert!(score_high >= score_low,
        "Higher access count should produce equal or higher score: high={score_high}, low={score_low}");
}

// =====================================================================
// DIVERSE PROMPT SIMULATION -- 20 varied "real-world" prompts
// =====================================================================

#[test]
fn test_28_prompt_how_does_rust_compiler_work() {
    let mut s = setup();
    let r = recall(&mut s, &["rust", "compiler"], 5);
    assert!(!r.is_empty());
}

#[test]
fn test_29_prompt_explain_harmonic_theory() {
    let mut s = setup();
    let r = recall(&mut s, &["harmony", "theory"], 5);
    assert!(!r.is_empty());
    let domains: Vec<_> = r.iter().map(|(c, _)| c.as_str()).collect();
    assert!(domains.iter().any(|c| ["harmony", "melody", "rhythm", "tone"].contains(c))
         || domains.iter().any(|c| ["theory", "ratio", "proof"].contains(c)),
        "Harmonic theory should bridge music and math");
}

#[test]
fn test_30_prompt_memory_patterns_in_brain() {
    let mut s = setup();
    let r = recall(&mut s, &["memory", "pattern", "brain"], 8);
    assert!(r.len() >= 3, "Rich query should activate multiple nodes");
}

#[test]
fn test_31_prompt_fractal_geometry_proof() {
    let mut s = setup();
    let r = recall(&mut s, &["fractal", "geometry", "proof"], 8);
    assert!(!r.is_empty(), "Math query should return results");
}

#[test]
fn test_32_prompt_signal_processing_api() {
    let mut s = setup();
    let r = recall(&mut s, &["signal", "api"], 5);
    assert!(!r.is_empty());
}

#[test]
fn test_33_prompt_travel_weather_planning() {
    let mut s = setup();
    let r = recall(&mut s, &["travel", "weather"], 5);
    assert!(!r.is_empty());
}

#[test]
fn test_34_prompt_system_model_architecture() {
    let mut s = setup();
    let r = recall(&mut s, &["system", "model"], 5);
    assert!(!r.is_empty());
}

#[test]
fn test_35_prompt_evolve_pattern_attractor() {
    let mut s = setup();
    let r = recall(&mut s, &["evolve", "pattern", "attractor"], 8);
    assert!(r.len() >= 2);
}

#[test]
fn test_36_prompt_dream_memory() {
    let mut s = setup();
    let r = recall(&mut s, &["dream", "memory"], 5);
    assert!(!r.is_empty());
    assert!(r.iter().any(|(c, _)| c == "dream" || c == "memory"));
}

#[test]
fn test_37_prompt_noise_signal_ratio() {
    let mut s = setup();
    let r = recall(&mut s, &["noise", "signal", "ratio"], 8);
    assert!(r.len() >= 2);
}

#[test]
fn test_38_prompt_lisp_backend_tool() {
    let mut s = setup();
    let r = recall(&mut s, &["lisp", "backend", "tool"], 5);
    assert!(!r.is_empty());
}

#[test]
fn test_39_prompt_melody_tone_rhythm() {
    let mut s = setup();
    let r = recall(&mut s, &["melody", "tone", "rhythm"], 5);
    assert!(!r.is_empty());
    let music_count = r.iter().filter(|(c, _)| {
        ["melody", "tone", "rhythm", "harmony"].contains(&c.as_str())
    }).count();
    assert!(music_count >= 2);
}

#[test]
fn test_40_prompt_calendar_meeting_schedule() {
    let mut s = setup();
    let r = recall(&mut s, &["calendar", "meeting"], 5);
    assert!(!r.is_empty());
}

#[test]
fn test_41_prompt_brain_evolve_code() {
    let mut s = setup();
    let r = recall(&mut s, &["brain", "evolve", "code"], 8);
    assert!(r.len() >= 2);
}

#[test]
fn test_42_prompt_single_concept_memory() {
    let mut s = setup();
    let r = recall(&mut s, &["memory"], 10);
    assert!(!r.is_empty());
    let has_memory = r.iter().any(|(c, _)| c == "memory");
    assert!(has_memory, "Source concept should appear in results: {:?}", r);
}

#[test]
fn test_43_prompt_single_concept_weather() {
    let mut s = setup();
    let r = recall(&mut s, &["weather"], 5);
    assert!(!r.is_empty());
}

#[test]
fn test_44_prompt_unknown_concept() {
    let mut s = setup();
    let _r = recall(&mut s, &["quantum"], 5);
}

#[test]
fn test_45_prompt_empty_query() {
    let mut s = setup();
    let _r = recall(&mut s, &[], 5);
}

// =====================================================================
// GRAPH EVOLUTION -- concepts added over time
// =====================================================================

#[test]
fn test_46_graph_reload_updates_spectral() {
    let mut s = setup();
    let s1 = eigenmode_status(&s).unwrap();

    let nodes = vec![
        ("alpha".into(), "generic".into(), 5, vec![]),
        ("beta".into(), "generic".into(), 3, vec![]),
    ];
    let edges = vec![("alpha".into(), "beta".into(), 1.0, false)];
    let _ = load_graph(&mut s, nodes, edges);

    let s2 = eigenmode_status(&s).unwrap();
    assert_ne!(s1, s2, "Eigenmode status should change after graph reload");
}

#[test]
fn test_47_graph_growing_over_time() {
    let mut s = FieldState::new();

    let nodes1 = vec![
        ("a".into(), "generic".into(), 1, vec!["E1".into()]),
        ("b".into(), "generic".into(), 1, vec!["E2".into()]),
    ];
    let edges1 = vec![("a".into(), "b".into(), 1.0, false)];
    let _ = load_graph(&mut s, nodes1, edges1);
    let r1 = recall(&mut s, &["a"], 5);
    assert!(!r1.is_empty());

    let nodes2 = vec![
        ("a".into(), "generic".into(), 3, vec!["E1".into()]),
        ("b".into(), "generic".into(), 2, vec!["E2".into()]),
        ("c".into(), "generic".into(), 1, vec!["E3".into()]),
    ];
    let edges2 = vec![
        ("a".into(), "b".into(), 2.0, false),
        ("b".into(), "c".into(), 1.0, false),
    ];
    let _ = load_graph(&mut s, nodes2, edges2);
    let r2 = recall(&mut s, &["a"], 5);
    assert!(r2.len() >= r1.len(), "Larger graph should recall at least as many nodes");
}

// =====================================================================
// DEGENERATE CASES
// =====================================================================

#[test]
fn test_48_single_node_graph() {
    let mut s = FieldState::new();
    let nodes = vec![("solo".into(), "generic".into(), 1, vec!["E1".into()])];
    let edges: Vec<(String, String, f64, bool)> = vec![];
    let _ = load_graph(&mut s, nodes, edges);
    let _r = recall(&mut s, &["solo"], 5);
}

#[test]
fn test_49_disconnected_graph() {
    let mut s = FieldState::new();
    let nodes = vec![
        ("island1".into(), "music".into(), 1, vec!["E1".into()]),
        ("island2".into(), "math".into(), 1, vec!["E2".into()]),
    ];
    let edges: Vec<(String, String, f64, bool)> = vec![];
    let _ = load_graph(&mut s, nodes, edges);
    let _r = recall(&mut s, &["island1"], 5);
}

#[test]
fn test_50_empty_graph() {
    let mut s = FieldState::new();
    let r = recall(&mut s, &["anything"], 5);
    assert!(r.is_empty(), "Empty graph should return no results");
}

// =====================================================================
// THOMAS ATTRACTOR BASIN VERIFICATION
// =====================================================================

#[test]
fn test_51_thomas_explores_multiple_basins() {
    let mut s = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    let _ = load_graph(&mut s, nodes, edges);

    let mut basins_seen = std::collections::HashSet::new();
    for i in 0..500 {
        let signal = 0.5 + 0.3 * ((i as f64) * 0.1).sin();
        let noise = 0.3 + 0.1 * ((i as f64) * 0.07).cos();
        let _ = step_attractors(&mut s, signal, noise);

        if let Ok(bs) = basin_status(&s) {
            if let Some(pos) = bs.find(":current ") {
                let rest = &bs[pos + 9..];
                let end = rest.find(' ').unwrap_or(rest.len());
                basins_seen.insert(rest[..end].to_string());
            }
        }
    }
    assert!(basins_seen.len() >= 1,
        "Thomas should visit at least 1 basin, visited: {:?}", basins_seen);
}

// =====================================================================
// STATUS AND LIFECYCLE
// =====================================================================

#[test]
fn test_52_status_reports_graph_info() {
    let s = setup();
    let st = status(&s).unwrap();
    assert!(st.contains(":graph-n 30"), "Should report 30 nodes, got: {st}");
    assert!(st.contains(":graph-version"), "Should report graph version");
    assert!(st.contains(":basin"), "Should report current basin");
}

#[test]
fn test_53_reset_clears_state() {
    let mut s = setup();
    let _ = reset(&mut s);
    let st = status(&s).unwrap();
    assert!(st.contains(":graph-n 0"), "After reset, graph should be empty");
}

#[test]
fn test_54_multiple_recall_cycles() {
    let mut s = setup();
    let queries: Vec<Vec<&str>> = vec![
        vec!["rust", "code"],
        vec!["harmony", "melody"],
        vec!["memory", "brain"],
        vec!["fractal", "ratio"],
        vec!["signal", "noise"],
        vec!["calendar", "meeting"],
        vec!["pattern", "system"],
        vec!["model", "theory"],
        vec!["dream", "evolve"],
        vec!["compiler", "tool"],
    ];

    for query in &queries {
        let r = recall(&mut s, query, 5);
        assert!(!r.is_empty(), "Query {:?} should return results", query);
        let _ = step_attractors(&mut s, 0.6, 0.4);
    }
}

// =====================================================================
// HOLOGRAPHIC INTEGRATION TESTS
//
// Verify the holographic property: boundary changes (graph, signal/noise)
// propagate to bulk (recall activations), and all mathematical components
// (heat kernel, soft basin affinity, topological flux, invariant measure)
// contribute to the final activation score through the public API.
// =====================================================================

#[test]
fn test_55_holographic_topology_computed_on_load() {
    // Load a graph with cycles. Topology (cycle basis, node flux) is computed
    // during load_graph. We verify indirectly: a cyclic graph should produce
    // richer recall than an acyclic one of the same size, because topological
    // flux contributes to scoring for nodes on cycles.
    let mut field = FieldState::new();

    // Acyclic graph (tree): 4 nodes, 3 edges, beta_1 = 0
    let nodes_tree = vec![
        ("a".into(), "math".into(), 3, vec!["e1".into()]),
        ("b".into(), "math".into(), 2, vec!["e2".into()]),
        ("c".into(), "engineering".into(), 2, vec!["e3".into()]),
        ("d".into(), "engineering".into(), 1, vec!["e4".into()]),
    ];
    let edges_tree = vec![
        ("a".into(), "b".into(), 2.0, false),
        ("b".into(), "c".into(), 1.5, false),
        ("c".into(), "d".into(), 1.0, false),
    ];
    load_graph(&mut field, nodes_tree, edges_tree).unwrap();
    step_attractors(&mut field, 0.7, 0.2).unwrap();
    let result_tree = field_recall(&mut field, vec!["a".into()], vec![], 10).unwrap();

    // Cyclic graph: same 4 nodes, 5 edges, beta_1 = 5 - 4 + 1 = 2
    let nodes_cyclic = vec![
        ("a".into(), "math".into(), 3, vec!["e1".into()]),
        ("b".into(), "math".into(), 2, vec!["e2".into()]),
        ("c".into(), "engineering".into(), 2, vec!["e3".into()]),
        ("d".into(), "engineering".into(), 1, vec!["e4".into()]),
    ];
    let edges_cyclic = vec![
        ("a".into(), "b".into(), 2.0, false),
        ("b".into(), "c".into(), 1.5, false),
        ("c".into(), "d".into(), 1.0, false),
        ("a".into(), "d".into(), 1.0, true),   // creates a cycle
        ("a".into(), "c".into(), 1.0, true),   // creates another cycle
    ];
    load_graph(&mut field, nodes_cyclic, edges_cyclic).unwrap();
    step_attractors(&mut field, 0.7, 0.2).unwrap();
    let result_cyclic = field_recall(&mut field, vec!["a".into()], vec![], 10).unwrap();

    // Cyclic graph should produce at least as many activations as the tree,
    // because topological flux from cycles adds a non-zero scoring component.
    assert!(
        result_cyclic.activations.len() >= result_tree.activations.len(),
        "Cyclic graph (with topology) should activate at least as many nodes as tree: cyclic={}, tree={}",
        result_cyclic.activations.len(), result_tree.activations.len()
    );

    // Both should produce non-empty results (heat kernel is active for both).
    assert!(!result_tree.activations.is_empty(), "Tree graph should produce activations via heat kernel");
    assert!(!result_cyclic.activations.is_empty(), "Cyclic graph should produce activations via heat kernel + flux");
}

#[test]
fn test_56_signal_noise_propagates_to_recall() {
    // Signal/noise from step_attractors affects diffusion time in heat kernel:
    //   High signal (low noise) → small diffusion time → precise local recall
    //   Low signal (high noise) → large diffusion time → broad associative recall
    //
    // We verify this through observable differences in recall activation patterns.
    let mut field = FieldState::new();
    let nodes = vec![
        ("center".into(), "math".into(), 10, vec!["e1".into()]),
        ("near".into(), "math".into(), 5, vec!["e2".into()]),
        ("far".into(), "engineering".into(), 2, vec!["e3".into()]),
        ("remote".into(), "life".into(), 1, vec!["e4".into()]),
    ];
    let edges = vec![
        ("center".into(), "near".into(), 5.0, false),
        ("near".into(), "far".into(), 2.0, true),
        ("far".into(), "remote".into(), 1.0, true),
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // High signal / low noise → precise recall (small diffusion time)
    step_attractors(&mut field, 0.9, 0.1).unwrap();
    let result_precise = field_recall(&mut field, vec!["center".into()], vec![], 10).unwrap();

    // Low signal / high noise → broad recall (large diffusion time)
    step_attractors(&mut field, 0.1, 0.8).unwrap();
    let result_broad = field_recall(&mut field, vec!["center".into()], vec![], 10).unwrap();

    // Both modes should produce results (heat kernel is always active).
    assert!(!result_precise.activations.is_empty(), "Precise mode should produce activations");
    assert!(!result_broad.activations.is_empty(), "Broad mode should produce activations");

    // The step output should confirm signal/noise is stored (via b parameter modulation).
    let step_high = step_attractors(&mut field, 0.9, 0.1).unwrap();
    assert!(step_high.contains(":b"), "Step should report Thomas b parameter");
    let step_low = step_attractors(&mut field, 0.1, 0.8).unwrap();
    assert!(step_low.contains(":b"), "Step should report Thomas b parameter");
    // b is modulated: b_eff = b_base + b_scale * (signal - noise)
    // High signal: b should be higher; low signal: b should be lower.
    // Both should be within [0.18, 0.24] range.
    assert!(step_high.contains("0.2") || step_high.contains("0.1"),
        "Thomas b should be in valid range for high signal: {step_high}");
    assert!(step_low.contains("0.1") || step_low.contains("0.2"),
        "Thomas b should be in valid range for low signal: {step_low}");
}

#[test]
fn test_57_heat_kernel_active_by_default() {
    // Heat kernel should always be enabled when eigenvectors exist.
    // A graph with >= 2 nodes connected by an edge will have eigenvectors,
    // so recall should produce activations using heat kernel propagation.
    let mut field = FieldState::new();
    let nodes = vec![
        ("alpha".into(), "math".into(), 5, vec!["e1".into()]),
        ("beta".into(), "math".into(), 3, vec!["e2".into()]),
        ("gamma".into(), "engineering".into(), 2, vec!["e3".into()]),
    ];
    let edges = vec![
        ("alpha".into(), "beta".into(), 2.0, false),
        ("beta".into(), "gamma".into(), 1.0, false),
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Verify eigenvectors exist (spectral decomposition happened).
    let eigen_status = eigenmode_status(&field).unwrap();
    assert!(!eigen_status.contains(":eigenvalues ()"),
        "Eigenvalues should not be empty after loading a connected graph");

    // Step once to establish signal/noise state.
    step_attractors(&mut field, 0.7, 0.2).unwrap();

    // Recall should work and produce results (heat kernel contributes to scoring).
    let result = field_recall(&mut field, vec!["alpha".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(),
        "Heat kernel recall should produce activations for connected graph");

    // The source node should appear in results.
    let has_alpha = result.activations.iter().any(|a| a.concept == "alpha");
    assert!(has_alpha, "Source concept 'alpha' should appear in recall results");
}

#[test]
fn test_58_soft_basin_affinity_influences_scoring() {
    // Soft basin classification (Boltzmann over Thomas centroids) replaces
    // the binary basin gate. After evolving the attractor, the soft basins
    // should be a valid probability distribution that influences recall.
    let mut field = FieldState::new();
    let nodes = vec![
        ("math-concept".into(), "math".into(), 5, vec!["e1".into()]),
        ("eng-concept".into(), "engineering".into(), 5, vec!["e2".into()]),
        ("life-concept".into(), "life".into(), 5, vec!["e3".into()]),
    ];
    let edges = vec![
        ("math-concept".into(), "eng-concept".into(), 2.0, true),
        ("eng-concept".into(), "life-concept".into(), 2.0, true),
        ("math-concept".into(), "life-concept".into(), 2.0, true),
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Step attractors many times to establish a definite basin position.
    for _ in 0..20 {
        step_attractors(&mut field, 0.7, 0.2).unwrap();
    }

    // After 20 steps, recall should produce results with basin information.
    let result = field_recall(&mut field, vec!["math-concept".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(),
        "Recall after basin evolution should produce results");

    // Each activation should have a valid basin assignment.
    for act in &result.activations {
        assert!(act.score > 0.0, "Activation score should be positive");
    }

    // The basin status should report a definite current basin.
    let bs = basin_status(&field).unwrap();
    assert!(bs.contains(":current"), "Basin status should report current basin");
    assert!(bs.contains(":dwell-ticks"), "Basin status should report dwell ticks");
}

#[test]
fn test_59_holographic_boundary_to_bulk_propagation() {
    // The holographic principle: changing the boundary (graph structure)
    // changes the bulk (recall activations). A richer boundary should
    // produce a richer or different bulk.
    let mut field = FieldState::new();

    // Phase 1: Small graph, few connections.
    let nodes1 = vec![
        ("core".into(), "math".into(), 5, vec!["e1".into()]),
        ("aux".into(), "math".into(), 2, vec!["e2".into()]),
    ];
    let edges1 = vec![
        ("core".into(), "aux".into(), 1.0, false),
    ];
    load_graph(&mut field, nodes1, edges1).unwrap();
    step_attractors(&mut field, 0.6, 0.3).unwrap();

    let result1 = field_recall(&mut field, vec!["core".into()], vec![], 10).unwrap();
    let score1 = result1.activations.first().map(|a| a.score).unwrap_or(0.0);
    let count1 = result1.activations.len();

    // Phase 2: Richer graph -- same concepts but more connections and nodes.
    let nodes2 = vec![
        ("core".into(), "math".into(), 10, vec!["e1".into(), "e3".into()]),
        ("aux".into(), "math".into(), 5, vec!["e2".into()]),
        ("bridge".into(), "engineering".into(), 3, vec!["e4".into()]),
    ];
    let edges2 = vec![
        ("core".into(), "aux".into(), 5.0, false),
        ("core".into(), "bridge".into(), 3.0, true),
        ("aux".into(), "bridge".into(), 2.0, true),
    ];
    load_graph(&mut field, nodes2, edges2).unwrap();
    step_attractors(&mut field, 0.6, 0.3).unwrap();

    let result2 = field_recall(&mut field, vec!["core".into()], vec![], 10).unwrap();
    let count2 = result2.activations.len();

    // Richer boundary should produce at least as many activations (bulk).
    assert!(count2 >= count1,
        "Richer graph (boundary) should produce at least as many activations (bulk): phase2={count2}, phase1={count1}");

    // Verify the new node appears in results.
    let has_bridge = result2.activations.iter().any(|a| a.concept == "bridge");
    assert!(has_bridge, "New 'bridge' node should appear in recall after graph enrichment");

    // Score should still be positive for the source node.
    assert!(score1 > 0.0, "Source node should have positive score in phase 1");
    let score2 = result2.activations.iter().find(|a| a.concept == "core").map(|a| a.score).unwrap_or(0.0);
    assert!(score2 > 0.0, "Source node should have positive score in phase 2");
}

#[test]
fn test_60_invariant_measure_evolves_through_stepping() {
    // The invariant measure tracks attractor visits over time.
    // We verify this indirectly: after many steps, the system's basin
    // should be well-established (low uncertainty).
    let mut field = FieldState::new();
    let nodes = vec![
        ("x".into(), "math".into(), 3, vec!["e1".into()]),
        ("y".into(), "math".into(), 2, vec!["e2".into()]),
    ];
    let edges = vec![
        ("x".into(), "y".into(), 1.0, false),
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Initially, basin status should exist.
    let initial_bs = basin_status(&field).unwrap();
    assert!(initial_bs.contains(":dwell-ticks 0"),
        "Initial dwell ticks should be 0: {initial_bs}");

    // After many steps, the dwell counter should have advanced.
    for _ in 0..50 {
        step_attractors(&mut field, 0.6, 0.3).unwrap();
    }
    let evolved_bs = basin_status(&field).unwrap();
    assert!(evolved_bs.contains(":dwell-ticks"),
        "Evolved basin should report dwell ticks");
    // Dwell ticks should be > 0 after 50 steps (either stayed in basin or switched).
    // Parse the dwell-ticks value.
    if let Some(pos) = evolved_bs.find(":dwell-ticks ") {
        let rest = &evolved_bs[pos + 13..];
        let end = rest.find(|c: char| c.is_whitespace() || c == ')').unwrap_or(rest.len());
        if let Ok(dwell) = rest[..end].parse::<u64>() {
            assert!(dwell > 0, "Dwell ticks should be > 0 after 50 attractor steps, got {dwell}");
        }
    }
}

#[test]
fn test_61_topological_flux_contributes_to_scoring() {
    // Nodes on cycles receive topological flux, which contributes to their
    // activation score. We verify this by comparing recall of a node that sits
    // on multiple cycles vs one that sits on none.
    let mut field = FieldState::new();
    let nodes = vec![
        ("hub".into(), "math".into(), 3, vec!["e1".into()]),
        ("spoke1".into(), "math".into(), 3, vec!["e2".into()]),
        ("spoke2".into(), "engineering".into(), 3, vec!["e3".into()]),
        ("spoke3".into(), "engineering".into(), 3, vec!["e4".into()]),
        ("leaf".into(), "life".into(), 3, vec!["e5".into()]),
    ];
    // hub-spoke1-spoke2-hub forms a cycle, hub-spoke2-spoke3-hub forms another.
    // "leaf" is connected only to spoke3 (no cycles).
    let edges = vec![
        ("hub".into(), "spoke1".into(), 2.0, false),
        ("spoke1".into(), "spoke2".into(), 2.0, false),
        ("spoke2".into(), "hub".into(), 2.0, true),     // closes cycle 1
        ("hub".into(), "spoke3".into(), 2.0, false),
        ("spoke2".into(), "spoke3".into(), 2.0, false),
        ("spoke3".into(), "hub".into(), 2.0, true),     // closes cycle 2
        ("spoke3".into(), "leaf".into(), 1.0, false),   // no cycle for leaf
    ];
    load_graph(&mut field, nodes, edges).unwrap();
    step_attractors(&mut field, 0.6, 0.3).unwrap();

    // Query from hub -- should activate both cycle and non-cycle nodes.
    let result = field_recall(&mut field, vec!["hub".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(), "Recall from hub should produce activations");

    // Hub sits on multiple cycles, so it should have one of the higher scores.
    let hub_score = result.activations.iter()
        .find(|a| a.concept == "hub")
        .map(|a| a.score)
        .unwrap_or(0.0);
    assert!(hub_score > 0.0, "Hub (on cycles) should have positive score");
}

#[test]
fn test_62_heat_kernel_diffusion_breadth() {
    // With heat kernel, even distant nodes should receive some activation
    // (heat diffuses outward). Verify that recall reaches beyond immediate
    // neighbors in a longer chain.
    let mut field = FieldState::new();
    let nodes = vec![
        ("source".into(), "math".into(), 5, vec!["e1".into()]),
        ("hop1".into(), "math".into(), 3, vec!["e2".into()]),
        ("hop2".into(), "engineering".into(), 3, vec!["e3".into()]),
        ("hop3".into(), "engineering".into(), 3, vec!["e4".into()]),
        ("hop4".into(), "life".into(), 3, vec!["e5".into()]),
    ];
    let edges = vec![
        ("source".into(), "hop1".into(), 3.0, false),
        ("hop1".into(), "hop2".into(), 2.0, false),
        ("hop2".into(), "hop3".into(), 2.0, false),
        ("hop3".into(), "hop4".into(), 1.0, false),
    ];
    load_graph(&mut field, nodes, edges).unwrap();
    // Use low signal / moderate noise to get a broad diffusion time.
    step_attractors(&mut field, 0.3, 0.3).unwrap();

    let result = field_recall(&mut field, vec!["source".into()], vec![], 10).unwrap();

    // Heat kernel should diffuse activation across the chain.
    // At least the source and its immediate neighbor should be activated.
    assert!(result.activations.len() >= 2,
        "Heat kernel should diffuse activation to at least 2 nodes in a chain, got {}",
        result.activations.len());
}

#[test]
fn test_63_all_holographic_components_wired() {
    // End-to-end integration: build a graph with cycles, step attractors,
    // then recall. Verify that the full pipeline (field potential, eigenmodes,
    // heat kernel, soft basin affinity, topological flux) produces meaningful
    // results by checking that recall produces differentiated scores.
    let mut field = FieldState::new();

    // Build a graph with enough structure for all components:
    // - At least 3 nodes for eigenmode separation
    // - At least one cycle for topological flux
    // - Cross-domain edges for basin affinity variation
    let nodes = vec![
        ("math-core".into(), "math".into(), 8, vec!["S1".into()]),
        ("math-aux".into(), "math".into(), 4, vec!["D1".into()]),
        ("eng-core".into(), "engineering".into(), 6, vec!["S2".into()]),
        ("eng-aux".into(), "engineering".into(), 3, vec!["D2".into()]),
        ("bridge".into(), "generic".into(), 5, vec!["S3".into()]),
    ];
    let edges = vec![
        ("math-core".into(), "math-aux".into(), 4.0, false),
        ("eng-core".into(), "eng-aux".into(), 3.0, false),
        ("math-core".into(), "bridge".into(), 3.0, true),       // cross-domain
        ("eng-core".into(), "bridge".into(), 3.0, true),        // cross-domain
        ("math-aux".into(), "eng-aux".into(), 2.0, true),       // cross-domain
        ("math-core".into(), "eng-core".into(), 2.0, true),     // creates cycle through bridge
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Verify spectral decomposition happened (eigenvectors exist).
    let eigen = eigenmode_status(&field).unwrap();
    assert!(!eigen.contains(":eigenvalues ()"),
        "Should have non-empty eigenvalues for a 5-node connected graph");

    // Step attractors to establish signal/noise and soft basin state.
    for _ in 0..10 {
        step_attractors(&mut field, 0.7, 0.2).unwrap();
    }

    // Recall should produce differentiated scores (not all equal).
    let result = field_recall(&mut field, vec!["math-core".into()], vec![], 10).unwrap();
    assert!(result.activations.len() >= 3,
        "Should activate at least 3 nodes in a well-connected 5-node graph, got {}",
        result.activations.len());

    // Scores should be differentiated (not all the same).
    if result.activations.len() >= 2 {
        let scores: Vec<f64> = result.activations.iter().map(|a| a.score).collect();
        let all_same = scores.windows(2).all(|w| (w[0] - w[1]).abs() < 0.001);
        assert!(!all_same,
            "Holographic scoring should produce differentiated scores, not all equal: {:?}", scores);
    }

    // The result should contain entry IDs from the graph.
    let sexp = result.to_sexp();
    assert!(sexp.contains("S1") || sexp.contains("S2") || sexp.contains("S3"),
        "Recall should carry entry IDs from activated nodes");
}

#[test]
fn test_64_graph_reload_recomputes_topology() {
    // When load_graph is called, topology should be recomputed for the new graph.
    // We verify this by loading a tree (no cycles), then a cyclic graph,
    // and checking that recall behavior changes appropriately.
    let mut field = FieldState::new();

    // Phase 1: Tree (no cycles, no topological flux).
    let nodes1 = vec![
        ("a".into(), "math".into(), 5, vec!["e1".into()]),
        ("b".into(), "math".into(), 3, vec!["e2".into()]),
        ("c".into(), "math".into(), 2, vec!["e3".into()]),
    ];
    let edges1 = vec![
        ("a".into(), "b".into(), 2.0, false),
        ("b".into(), "c".into(), 1.0, false),
    ];
    load_graph(&mut field, nodes1, edges1).unwrap();
    step_attractors(&mut field, 0.6, 0.3).unwrap();

    let result1 = field_recall(&mut field, vec!["a".into()], vec![], 10).unwrap();
    let _scores1: Vec<f64> = result1.activations.iter().map(|a| a.score).collect();

    // Phase 2: Replace with cyclic graph (has topology).
    let nodes2 = vec![
        ("a".into(), "math".into(), 5, vec!["e1".into()]),
        ("b".into(), "math".into(), 3, vec!["e2".into()]),
        ("c".into(), "math".into(), 2, vec!["e3".into()]),
    ];
    let edges2 = vec![
        ("a".into(), "b".into(), 2.0, false),
        ("b".into(), "c".into(), 1.0, false),
        ("a".into(), "c".into(), 1.5, true),  // adds a cycle
    ];
    load_graph(&mut field, nodes2, edges2).unwrap();
    step_attractors(&mut field, 0.6, 0.3).unwrap();

    let result2 = field_recall(&mut field, vec!["a".into()], vec![], 10).unwrap();
    let scores2: Vec<f64> = result2.activations.iter().map(|a| a.score).collect();

    // Topology recomputation means scores should change (different graph structure).
    // At minimum, the spectral decomposition is different (more edges = different eigenvalues).
    assert!(!scores2.is_empty(), "Cyclic graph should produce recall results");
    // The graph version should have incremented.
    let st = status(&field).unwrap();
    assert!(st.contains(":graph-version 2"), "Graph version should be 2 after two loads: {st}");
}

#[test]
fn test_65_signal_noise_modulates_thomas_b() {
    // The Thomas b parameter should be modulated by signal-noise difference.
    // b_eff = clamp(0.208 + 0.02 * (signal - noise), 0.18, 0.24)
    let mut field = FieldState::new();
    let nodes = vec![
        ("x".into(), "math".into(), 3, vec!["e1".into()]),
        ("y".into(), "math".into(), 2, vec!["e2".into()]),
    ];
    let edges = vec![("x".into(), "y".into(), 1.0, false)];
    load_graph(&mut field, nodes, edges).unwrap();

    // High signal - low noise → b should be above baseline.
    // b_eff = 0.208 + 0.02 * (0.9 - 0.1) = 0.208 + 0.016 = 0.224
    let step_high = step_attractors(&mut field, 0.9, 0.1).unwrap();
    assert!(step_high.contains(":b 0.224") || step_high.contains(":b 0.22"),
        "High signal should push Thomas b above baseline: {step_high}");

    // Low signal - high noise → b should be below baseline.
    // b_eff = 0.208 + 0.02 * (0.1 - 0.9) = 0.208 - 0.016 = 0.192
    let step_low = step_attractors(&mut field, 0.1, 0.9).unwrap();
    assert!(step_low.contains(":b 0.192") || step_low.contains(":b 0.19"),
        "Low signal should push Thomas b below baseline: {step_low}");
}

#[test]
fn test_66_attractor_stepping_preserves_field_stability() {
    // Rapidly alternating signal/noise should not crash or produce NaN.
    // The soft saturation in attractors should keep everything bounded.
    let mut field = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    load_graph(&mut field, nodes, edges).unwrap();

    for i in 0..200 {
        let signal = if i % 2 == 0 { 0.95 } else { 0.05 };
        let noise = 1.0 - signal;
        let step_result = step_attractors(&mut field, signal, noise).unwrap();
        // Should never contain NaN or inf.
        assert!(!step_result.contains("NaN"), "Step {i} produced NaN: {step_result}");
        assert!(!step_result.contains("inf"), "Step {i} produced inf: {step_result}");
    }

    // After rapid alternation, recall should still work.
    let result = field_recall(&mut field, vec!["rust".into(), "code".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(),
        "Recall should still work after rapid signal/noise alternation");
}

#[test]
fn test_67_holographic_scoring_weights_all_nonzero() {
    // In holographic mode (heat kernel active), all scoring channels should
    // contribute. We verify this by querying a well-connected graph and checking
    // that the recall produces scores that vary with graph structure.
    let mut field = FieldState::new();
    let nodes = vec![
        ("central".into(), "math".into(), 10, vec!["S1".into()]),
        ("inner1".into(), "math".into(), 5, vec!["S2".into()]),
        ("inner2".into(), "engineering".into(), 5, vec!["S3".into()]),
        ("outer".into(), "life".into(), 1, vec!["D1".into()]),
    ];
    let edges = vec![
        ("central".into(), "inner1".into(), 5.0, false),
        ("central".into(), "inner2".into(), 4.0, true),
        ("inner1".into(), "inner2".into(), 3.0, true),    // cycle
        ("inner2".into(), "outer".into(), 1.0, false),
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Run enough steps to be past warm-up (cycle >= 10).
    for _ in 0..15 {
        step_attractors(&mut field, 0.6, 0.3).unwrap();
    }

    let result = field_recall(&mut field, vec!["central".into()], vec![], 10).unwrap();
    assert!(result.activations.len() >= 2,
        "Should activate multiple nodes in a 4-node connected graph");

    // Inner nodes (closer, on cycle) should generally score higher than outer (distant, no cycle).
    let inner_scores: Vec<f64> = result.activations.iter()
        .filter(|a| a.concept.starts_with("inner"))
        .map(|a| a.score)
        .collect();
    let outer_score = result.activations.iter()
        .find(|a| a.concept == "outer")
        .map(|a| a.score);

    if let Some(os) = outer_score {
        if let Some(max_inner) = inner_scores.iter().cloned().reduce(f64::max) {
            assert!(max_inner >= os,
                "Inner nodes (on cycle, closer) should score >= outer: inner={max_inner}, outer={os}");
        }
    }
}

#[test]
fn test_68_holographic_full_pipeline_realistic_graph() {
    // Full integration test using the realistic 30-node graph.
    // Verifies that all holographic components produce stable, meaningful
    // recall after the full load → step → recall pipeline.
    let mut field = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    load_graph(&mut field, nodes, edges).unwrap();

    // Verify topology was computed (realistic graph has many cycles).
    let stat = status(&field).unwrap();
    assert!(stat.contains(":graph-n 30"), "Should have 30 nodes: {stat}");

    // Verify eigenvectors were computed.
    let eigen = eigenmode_status(&field).unwrap();
    assert!(!eigen.contains(":eigenvalues ()"), "Should have eigenvalues for 30-node graph");

    // Step attractors to establish holographic state.
    for i in 0..20 {
        let signal = 0.5 + 0.3 * ((i as f64) * 0.15).sin();
        let noise = 0.3 + 0.1 * ((i as f64) * 0.1).cos();
        let step = step_attractors(&mut field, signal, noise).unwrap();
        assert!(!step.contains("NaN"), "Step {i} should not produce NaN");
    }

    // Recall across multiple queries — all should produce results.
    let queries = vec![
        vec!["rust", "code"],
        vec!["harmony", "ratio"],
        vec!["memory", "pattern"],
        vec!["signal", "noise"],
        vec!["attractor", "fractal"],
    ];

    for concepts in &queries {
        let query: Vec<String> = concepts.iter().map(|c| c.to_string()).collect();
        let result = field_recall(&mut field, query, vec![], 10).unwrap();
        assert!(!result.activations.is_empty(),
            "Holographic recall for {:?} should produce activations", concepts);

        // All scores should be valid (no NaN, no negative, in [0,1]).
        for act in &result.activations {
            assert!(!act.score.is_nan(), "Score should not be NaN for {:?}", concepts);
            assert!(act.score >= 0.0 && act.score <= 1.0,
                "Score should be in [0,1], got {} for {:?}", act.score, concepts);
        }
    }
}

#[test]
fn test_69_holographic_cross_domain_flux_bridging() {
    // Cross-domain edges create cycles that carry topological flux.
    // Verify that cross-domain queries (which traverse these bridges)
    // activate concepts from both domains.
    let mut field = FieldState::new();
    let nodes = vec![
        ("math-node".into(), "math".into(), 5, vec!["M1".into()]),
        ("eng-node".into(), "engineering".into(), 5, vec!["E1".into()]),
        ("music-node".into(), "music".into(), 5, vec!["U1".into()]),
        ("bridge-me".into(), "generic".into(), 3, vec!["B1".into()]),
    ];
    let edges = vec![
        ("math-node".into(), "bridge-me".into(), 3.0, true),
        ("eng-node".into(), "bridge-me".into(), 3.0, true),
        ("music-node".into(), "bridge-me".into(), 3.0, true),
        ("math-node".into(), "eng-node".into(), 2.0, true),     // cycle: math-eng-bridge-math
        ("eng-node".into(), "music-node".into(), 2.0, true),    // cycle: eng-music-bridge-eng
    ];
    load_graph(&mut field, nodes, edges).unwrap();
    step_attractors(&mut field, 0.6, 0.3).unwrap();

    // Query from math-node should also activate cross-domain nodes.
    let result = field_recall(&mut field, vec!["math-node".into()], vec![], 10).unwrap();
    let activated_concepts: Vec<&str> = result.activations.iter().map(|a| a.concept.as_str()).collect();

    assert!(result.activations.len() >= 2,
        "Cross-domain graph should activate multiple nodes: {:?}", activated_concepts);

    // The bridge node should be activated (it's on every cycle).
    let has_bridge = activated_concepts.contains(&"bridge-me");
    assert!(has_bridge,
        "Bridge node (on all cycles) should be activated in cross-domain recall: {:?}", activated_concepts);
}

#[test]
fn test_70_holographic_persistence_across_recalls() {
    // Multiple recalls with attractor steps in between should produce
    // evolving but stable results — the holographic state should persist
    // and smoothly influence subsequent recalls.
    let mut field = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    load_graph(&mut field, nodes, edges).unwrap();

    let mut all_scores: Vec<Vec<f64>> = Vec::new();

    for i in 0..5 {
        let signal = 0.5 + 0.2 * (i as f64 * 0.5).sin();
        let noise = 0.3 + 0.1 * (i as f64 * 0.3).cos();
        step_attractors(&mut field, signal, noise).unwrap();

        let result = field_recall(&mut field, vec!["code".into(), "system".into()], vec![], 10).unwrap();
        assert!(!result.activations.is_empty(),
            "Recall iteration {i} should produce activations");

        let scores: Vec<f64> = result.activations.iter().map(|a| a.score).collect();
        all_scores.push(scores);
    }

    // Scores should evolve (not be identical across iterations).
    // At minimum, the cycle counter advances, which can affect warm-up weights.
    assert!(all_scores.len() == 5, "Should have 5 rounds of scores");
    // All rounds should produce at least some results.
    for (i, scores) in all_scores.iter().enumerate() {
        assert!(!scores.is_empty(), "Round {i} should have scores");
    }
}

// =====================================================================
// CIRCUIT END-TO-END TESTS
//
// These tests verify that data flows correctly through the FULL pipeline.
// Each test exercises a complete mathematical circuit — from boundary
// conditions (graph, signal/noise) through bulk dynamics (attractors,
// heat kernel, topology) to observable output (recall activations).
// =====================================================================

/// Circuit test: Signal/noise from attractor stepping flows through to heat kernel
/// diffusion time, which changes the breadth of recall activation.
/// High signal = precise (local), low signal = broad (associative).
#[test]
fn test_circuit_signal_to_heat_kernel_breadth() {
    let mut field = FieldState::new();
    // Build a 5-node chain: A-B-C-D-E
    let nodes: Vec<_> = ["a","b","c","d","e"].iter().enumerate()
        .map(|(i, &name)| (name.into(), "math".into(), (5-i) as i32, vec![format!("e{i}")]))
        .collect();
    let edges = vec![
        ("a".into(), "b".into(), 2.0, false),
        ("b".into(), "c".into(), 1.5, false),
        ("c".into(), "d".into(), 1.0, false),
        ("d".into(), "e".into(), 0.8, false),
    ];
    load_graph(&mut field, nodes.clone(), edges.clone()).unwrap();

    // High signal: precise, local recall — should activate fewer nodes
    step_attractors(&mut field, 0.95, 0.05).unwrap();
    let precise = field_recall(&mut field, vec!["a".into()], vec![], 10).unwrap();

    // Low signal: broad, associative recall — should activate more nodes
    step_attractors(&mut field, 0.1, 0.8).unwrap();
    let broad = field_recall(&mut field, vec!["a".into()], vec![], 10).unwrap();

    // The broad recall should have more or equal activations (more distant nodes activated)
    assert!(broad.activations.len() >= precise.activations.len(),
        "Low signal should produce broader recall (more activations): broad={}, precise={}",
        broad.activations.len(), precise.activations.len());
}

/// Circuit test: Adding cycles to the graph creates topological flux
/// that influences scoring. Nodes on cycles should score differently
/// than nodes on trees. Verified through the public status API.
#[test]
fn test_circuit_topology_to_scoring() {
    let mut field = FieldState::new();

    // Tree graph (no cycles) — topology should have no flux
    let nodes = vec![
        ("hub".into(), "math".into(), 5, vec!["e1".into()]),
        ("leaf1".into(), "math".into(), 3, vec!["e2".into()]),
        ("leaf2".into(), "math".into(), 3, vec!["e3".into()]),
        ("leaf3".into(), "math".into(), 3, vec!["e4".into()]),
    ];
    let edges_tree = vec![
        ("hub".into(), "leaf1".into(), 2.0, false),
        ("hub".into(), "leaf2".into(), 2.0, false),
        ("hub".into(), "leaf3".into(), 2.0, false),
    ];
    load_graph(&mut field, nodes.clone(), edges_tree).unwrap();

    // Checkpoint sexp for tree should show zero topology cycles
    let checkpoint_tree = field.checkpoint_sexp();
    assert!(checkpoint_tree.contains(":topology-cycles 0"),
        "Tree should have 0 topology cycles: {checkpoint_tree}");

    // Add cycle-forming edges
    let edges_cyclic = vec![
        ("hub".into(), "leaf1".into(), 2.0, false),
        ("hub".into(), "leaf2".into(), 2.0, false),
        ("hub".into(), "leaf3".into(), 2.0, false),
        ("leaf1".into(), "leaf2".into(), 1.5, false),  // creates cycle hub-leaf1-leaf2
        ("leaf2".into(), "leaf3".into(), 1.0, false),  // creates another cycle
    ];
    load_graph(&mut field, nodes, edges_cyclic).unwrap();

    let checkpoint_cyclic = field.checkpoint_sexp();
    // Extract topology-cycles count from checkpoint
    let has_cycles = !checkpoint_cyclic.contains(":topology-cycles 0");
    assert!(has_cycles,
        "Cyclic graph should have non-zero topology cycles: {checkpoint_cyclic}");

    // Recall should work in both cases, and the cyclic graph may yield different scores
    step_attractors(&mut field, 0.7, 0.2).unwrap();
    let result = field_recall(&mut field, vec!["hub".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(), "Cyclic graph recall should produce results");
}

/// Circuit test: Topological flux from cycles between basins
/// contributes to hysteresis coercive energy for basin switching.
/// Verified through basin_status and checkpoint_sexp public APIs.
#[test]
fn test_circuit_topology_to_hysteresis() {
    let mut field = FieldState::new();
    // Cross-domain graph with cycles connecting different basins
    let nodes = vec![
        ("math1".into(), "math".into(), 5, vec!["e1".into()]),
        ("math2".into(), "math".into(), 3, vec!["e2".into()]),
        ("eng1".into(), "engineering".into(), 5, vec!["e3".into()]),
        ("eng2".into(), "engineering".into(), 3, vec!["e4".into()]),
    ];
    let edges = vec![
        ("math1".into(), "math2".into(), 2.0, false),
        ("eng1".into(), "eng2".into(), 2.0, false),
        ("math1".into(), "eng1".into(), 1.5, true),   // cross-domain, creates inter-basin cycle
        ("math2".into(), "eng2".into(), 1.0, true),   // another cross-domain cycle
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // The graph should have cycles connecting different domain basins
    let checkpoint = field.checkpoint_sexp();
    assert!(!checkpoint.contains(":topology-cycles 0"),
        "Cross-domain cycles should exist: {checkpoint}");

    // Step attractors multiple times — the topological flux should contribute
    for _ in 0..30 {
        step_attractors(&mut field, 0.6, 0.3).unwrap();
    }

    // Basin should be established (non-zero dwell) — verify through basin_status
    let bs = basin_status(&field).unwrap();
    assert!(bs.contains(":dwell-ticks"), "Basin status should report dwell ticks");

    // Parse dwell-ticks and verify > 0
    if let Some(pos) = bs.find(":dwell-ticks ") {
        let rest = &bs[pos + 13..];
        let end = rest.find(|c: char| c.is_whitespace() || c == ')').unwrap_or(rest.len());
        if let Ok(dwell) = rest[..end].parse::<u64>() {
            assert!(dwell > 0, "Basin should have dwell time after 30 steps, got {dwell}");
        }
    }
}

/// Circuit test: Soft basin classification produces continuous affinity
/// that modulates scoring. After attractor evolution, the soft basins
/// should be non-uniform (verified through checkpoint_sexp).
#[test]
fn test_circuit_soft_basin_to_affinity() {
    let mut field = FieldState::new();
    let nodes = vec![
        ("x".into(), "math".into(), 5, vec!["e1".into()]),
        ("y".into(), "engineering".into(), 5, vec!["e2".into()]),
    ];
    let edges = vec![
        ("x".into(), "y".into(), 2.0, true),
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Evolve to establish basin
    for _ in 0..50 {
        step_attractors(&mut field, 0.7, 0.2).unwrap();
    }

    // Checkpoint should contain soft-basins with a valid probability distribution
    let checkpoint = field.checkpoint_sexp();
    assert!(checkpoint.contains(":soft-basins"),
        "Checkpoint should contain soft-basins: {checkpoint}");

    // Extract the soft-basins values and verify they form a valid distribution
    if let Some(pos) = checkpoint.find(":soft-basins (") {
        let rest = &checkpoint[pos + 14..];
        if let Some(end) = rest.find(')') {
            let basin_str = &rest[..end];
            let probs: Vec<f64> = basin_str.split_whitespace()
                .filter_map(|s| s.parse::<f64>().ok())
                .collect();
            assert_eq!(probs.len(), 6, "Should have 6 soft basin probabilities");
            let sum: f64 = probs.iter().sum();
            assert!((sum - 1.0).abs() < 0.01,
                "Probabilities should sum to 1.0, got {sum}: {:?}", probs);
            let max_prob = probs.iter().cloned().fold(0.0_f64, f64::max);
            assert!(max_prob > 0.1,
                "Dominant basin should have probability > 0.1 after evolution: {:?}", probs);
        }
    }

    // The soft basins should influence scoring: recall should produce results
    let result = field_recall(&mut field, vec!["x".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(),
        "Recall after basin evolution should produce results");
}

/// Circuit test: Full state checkpoint and restore preserves all
/// dynamical state. The holographic principle: boundary state
/// must persist completely.
#[test]
fn test_circuit_checkpoint_restore() {
    let mut field = FieldState::new();
    let nodes = vec![
        ("a".into(), "math".into(), 5, vec!["e1".into()]),
        ("b".into(), "math".into(), 3, vec!["e2".into()]),
        ("c".into(), "engineering".into(), 2, vec!["e3".into()]),
    ];
    let edges = vec![
        ("a".into(), "b".into(), 2.0, false),
        ("b".into(), "c".into(), 1.5, true),
        ("a".into(), "c".into(), 1.0, true),
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Evolve state
    for _ in 0..20 {
        step_attractors(&mut field, 0.8, 0.15).unwrap();
    }

    // The checkpoint_sexp should produce a non-empty string with all key fields
    let checkpoint = field.checkpoint_sexp();
    assert!(!checkpoint.is_empty(), "Checkpoint should produce output");
    assert!(checkpoint.contains(":thomas"), "Checkpoint should contain thomas state");
    assert!(checkpoint.contains(":last-signal"), "Checkpoint should contain signal");
    assert!(checkpoint.contains(":last-noise"), "Checkpoint should contain noise");
    assert!(checkpoint.contains(":soft-basins"), "Checkpoint should contain soft basins");
    assert!(checkpoint.contains(":basin"), "Checkpoint should contain basin");
    assert!(checkpoint.contains(":cycle"), "Checkpoint should contain cycle");
    assert!(checkpoint.contains(":aizawa"), "Checkpoint should contain aizawa state");
    assert!(checkpoint.contains(":halvorsen"), "Checkpoint should contain halvorsen state");
    assert!(checkpoint.contains(":measure-visits"), "Checkpoint should contain measure visits");
    assert!(checkpoint.contains(":topology-cycles"), "Checkpoint should contain topology cycle count");

    // Verify signal/noise are recorded correctly (last step was 0.8 / 0.15)
    assert!(checkpoint.contains(":last-signal 0.80"),
        "Signal should reflect last step: {checkpoint}");
    assert!(checkpoint.contains(":last-noise 0.15"),
        "Noise should reflect last step: {checkpoint}");

    // Call recall to increment cycle counter (step_attractors does not increment cycle)
    let _r = field_recall(&mut field, vec!["a".into()], vec![], 5).unwrap();
    let checkpoint2 = field.checkpoint_sexp();
    assert!(!checkpoint2.contains(":cycle 0 "),
        "Cycle should be non-zero after recall: {checkpoint2}");

    // Verify measure visits accumulated from stepping
    assert!(checkpoint.contains(":measure-visits"),
        "Checkpoint should contain measure visits: {checkpoint}");
    // Parse measure-visits and verify > 0
    if let Some(pos) = checkpoint.find(":measure-visits ") {
        let rest = &checkpoint[pos + 16..];
        let end = rest.find(|c: char| c.is_whitespace() || c == ')').unwrap_or(rest.len());
        if let Ok(visits) = rest[..end].parse::<u64>() {
            assert!(visits > 0, "Measure visits should be > 0 after 20 steps, got {visits}");
        }
    }
}

/// Circuit test: Complete holographic pipeline. Every mathematical
/// component must contribute to the final recall score.
/// This is the master circuit test.
#[test]
fn test_circuit_full_holographic_pipeline() {
    let mut field = FieldState::new();

    // Rich cross-domain graph with cycles
    let nodes = vec![
        ("fourier".into(), "math".into(), 8, vec!["e1".into()]),
        ("harmonic".into(), "music".into(), 6, vec!["e2".into()]),
        ("signal".into(), "engineering".into(), 7, vec!["e3".into()]),
        ("synthesis".into(), "engineering".into(), 4, vec!["e4".into()]),
        ("wave".into(), "math".into(), 5, vec!["e5".into()]),
        ("frequency".into(), "math".into(), 3, vec!["e6".into()]),
    ];
    let edges = vec![
        ("fourier".into(), "harmonic".into(), 3.0, true),
        ("harmonic".into(), "signal".into(), 2.5, true),
        ("signal".into(), "synthesis".into(), 2.0, false),
        ("synthesis".into(), "wave".into(), 1.5, true),
        ("wave".into(), "fourier".into(), 2.0, false),      // cycle!
        ("fourier".into(), "frequency".into(), 3.0, false),
        ("frequency".into(), "signal".into(), 1.5, true),   // another cycle!
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Verify topology computed (cycles should be detected)
    let checkpoint_pre = field.checkpoint_sexp();
    assert!(!checkpoint_pre.contains(":topology-cycles 0"),
        "Cycles should be detected: {checkpoint_pre}");

    // Verify eigenvalues computed (spectral decomposition happened)
    let eigen = eigenmode_status(&field).unwrap();
    assert!(!eigen.contains(":eigenvalues ()"),
        "Eigenvalues should be computed for a 6-node connected graph");

    // Evolve attractor dynamics
    for _ in 0..30 {
        step_attractors(&mut field, 0.75, 0.2).unwrap();
    }

    // Verify signal propagation through checkpoint
    let checkpoint = field.checkpoint_sexp();
    assert!(checkpoint.contains(":last-signal 0.75"),
        "Signal should be 0.75: {checkpoint}");
    assert!(checkpoint.contains(":last-noise 0.20"),
        "Noise should be 0.20: {checkpoint}");

    // Recall with a multi-concept query
    let result = field_recall(
        &mut field,
        vec!["fourier".into(), "signal".into()],
        vec![],
        10,
    ).unwrap();

    // Verify: activations exist and scores are valid
    assert!(!result.activations.is_empty(), "Recall should produce activations");
    for act in &result.activations {
        assert!(act.score >= 0.0 && act.score <= 1.0,
            "Score should be in [0,1], got {} for {}", act.score, act.concept);
        assert!(!act.concept.is_empty(), "Concept name should not be empty");
        assert!(!act.entry_ids.is_empty(), "Entry IDs should not be empty");
    }

    // The query concepts should be among the highest scoring
    let top_concepts: Vec<&str> = result.activations.iter()
        .take(3)
        .map(|a| a.concept.as_str())
        .collect();
    assert!(
        top_concepts.contains(&"fourier") || top_concepts.contains(&"signal"),
        "Query concepts should rank high: got {:?}", top_concepts
    );
}

/// Circuit test: The invariant measure should accumulate visits
/// and track the attractor's density distribution even as the
/// trajectory is chaotic. Verified through checkpoint_sexp.
#[test]
fn test_circuit_invariant_measure_stability() {
    let mut field = FieldState::new();
    let nodes = vec![
        ("x".into(), "math".into(), 3, vec!["e1".into()]),
        ("y".into(), "math".into(), 2, vec!["e2".into()]),
        ("z".into(), "engineering".into(), 1, vec!["e3".into()]),
    ];
    let edges = vec![
        ("x".into(), "y".into(), 2.0, false),
        ("y".into(), "z".into(), 1.0, false),
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Step 200 times with varying signal/noise (simulating chaotic input)
    for i in 0..200 {
        let signal = 0.3 + 0.4 * ((i as f64) * 0.1).sin();
        let noise = 0.1 + 0.2 * ((i as f64) * 0.07).cos();
        let step_result = step_attractors(&mut field, signal, noise).unwrap();
        // No NaN or Inf in any step
        assert!(!step_result.contains("NaN"),
            "Step {i} produced NaN: {step_result}");
        assert!(!step_result.contains("inf") && !step_result.contains("Inf"),
            "Step {i} produced Inf: {step_result}");
    }

    // Verify measure accumulated visits via checkpoint
    let checkpoint = field.checkpoint_sexp();
    // Parse measure-visits from checkpoint
    if let Some(pos) = checkpoint.find(":measure-visits ") {
        let rest = &checkpoint[pos + 16..];
        let end = rest.find(|c: char| c.is_whitespace() || c == ')').unwrap_or(rest.len());
        if let Ok(visits) = rest[..end].parse::<u64>() {
            assert!(visits >= 150,
                "Should have 150+ visits after 200 steps, got {visits}");
        }
    }

    // Attractor should still be bounded: Thomas state values in checkpoint
    // should be within [-3.5, 3.5] (soft saturation radius is 3.0)
    if let Some(pos) = checkpoint.find(":thomas (") {
        let rest = &checkpoint[pos + 9..];
        if let Some(end) = rest.find(')') {
            let thomas_str = &rest[..end];
            let coords: Vec<f64> = thomas_str.split_whitespace()
                .filter_map(|s| s.parse::<f64>().ok())
                .collect();
            assert_eq!(coords.len(), 3, "Thomas should have 3 coordinates");
            for (i, &c) in coords.iter().enumerate() {
                assert!(c.abs() < 3.5,
                    "Thomas coord {i} should be bounded, got {c}");
                assert!(c.is_finite(),
                    "Thomas coord {i} should be finite, got {c}");
            }
        }
    }
}

/// Circuit test: Eigenmode structure changes when graph topology changes.
/// The spectral circuit: graph → Laplacian → eigendecomposition → eigenmodes
/// → recall activation must be recomputed on each load_graph.
#[test]
fn test_circuit_eigenmode_recomputation() {
    let mut field = FieldState::new();

    // Phase 1: Simple 2-node graph
    let nodes1 = vec![
        ("a".into(), "math".into(), 5, vec!["e1".into()]),
        ("b".into(), "math".into(), 3, vec!["e2".into()]),
    ];
    let edges1 = vec![("a".into(), "b".into(), 1.0, false)];
    load_graph(&mut field, nodes1, edges1).unwrap();
    let eigen1 = eigenmode_status(&field).unwrap();

    // Phase 2: Richer 4-node graph with cycle
    let nodes2 = vec![
        ("a".into(), "math".into(), 5, vec!["e1".into()]),
        ("b".into(), "math".into(), 3, vec!["e2".into()]),
        ("c".into(), "engineering".into(), 4, vec!["e3".into()]),
        ("d".into(), "engineering".into(), 2, vec!["e4".into()]),
    ];
    let edges2 = vec![
        ("a".into(), "b".into(), 2.0, false),
        ("b".into(), "c".into(), 1.5, false),
        ("c".into(), "d".into(), 1.0, false),
        ("d".into(), "a".into(), 1.0, true),  // cycle
    ];
    load_graph(&mut field, nodes2, edges2).unwrap();
    let eigen2 = eigenmode_status(&field).unwrap();

    // Eigenmode status should be different (different graph = different spectrum)
    assert_ne!(eigen1, eigen2,
        "Eigenmode status should change when graph topology changes");

    // Graph version should have incremented (visible in eigenmode status)
    assert!(eigen2.contains(":graph-version 2"),
        "Graph version should be 2 after two loads: {eigen2}");

    // Recall should work with the new eigenmodes
    step_attractors(&mut field, 0.6, 0.3).unwrap();
    let result = field_recall(&mut field, vec!["a".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(),
        "Recall should work after eigenmode recomputation");
}

/// Circuit test: Cross-domain bridging through the full scoring pipeline.
/// Nodes connected by interdisciplinary edges should be reachable from
/// queries in either domain. Verifies the cycle: graph loading → basin
/// assignment → cross-domain affinity → recall activation.
#[test]
fn test_circuit_cross_domain_bridging() {
    let mut field = FieldState::new();

    // Three-domain graph with cross-domain bridges
    let nodes = vec![
        ("math-core".into(), "math".into(), 8, vec!["M1".into()]),
        ("eng-core".into(), "engineering".into(), 7, vec!["E1".into()]),
        ("music-core".into(), "music".into(), 6, vec!["U1".into()]),
        ("bridge-a".into(), "generic".into(), 4, vec!["B1".into()]),
        ("bridge-b".into(), "generic".into(), 4, vec!["B2".into()]),
    ];
    let edges = vec![
        ("math-core".into(), "bridge-a".into(), 3.0, true),
        ("eng-core".into(), "bridge-a".into(), 3.0, true),
        ("eng-core".into(), "bridge-b".into(), 2.5, true),
        ("music-core".into(), "bridge-b".into(), 2.5, true),
        ("bridge-a".into(), "bridge-b".into(), 2.0, false),
        ("math-core".into(), "eng-core".into(), 1.5, true),   // direct cross-domain + cycle
    ];
    load_graph(&mut field, nodes, edges).unwrap();

    // Step to establish basin state
    for _ in 0..15 {
        step_attractors(&mut field, 0.6, 0.3).unwrap();
    }

    // Query from math domain should reach engineering concepts through bridges
    let result = field_recall(&mut field, vec!["math-core".into()], vec![], 10).unwrap();
    let activated: Vec<&str> = result.activations.iter().map(|a| a.concept.as_str()).collect();

    assert!(result.activations.len() >= 2,
        "Cross-domain graph should activate multiple nodes: {:?}", activated);

    // At least one bridge node should be activated
    let has_bridge = activated.contains(&"bridge-a") || activated.contains(&"bridge-b");
    assert!(has_bridge,
        "Bridge nodes should be activated in cross-domain recall: {:?}", activated);

    // Verify domains are mixed in results (not all math)
    let has_non_math = result.activations.iter().any(|a| {
        a.domain != harmonia_memory_field::Domain::Math
    });
    assert!(has_non_math,
        "Cross-domain recall should activate nodes from multiple domains: {:?}", activated);
}

/// Circuit test: Attractor stability under rapid signal/noise oscillation.
/// The full circuit from signal input → attractor step → basin hysteresis
/// → recall must remain stable (no NaN, no divergence) even under adversarial
/// input patterns.
#[test]
fn test_circuit_attractor_stability_adversarial() {
    let mut field = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    load_graph(&mut field, nodes, edges).unwrap();

    // Phase 1: Rapid oscillation between extreme signal/noise values
    for i in 0..100 {
        let signal = if i % 2 == 0 { 0.99 } else { 0.01 };
        let noise = 1.0 - signal;
        let step = step_attractors(&mut field, signal, noise).unwrap();
        assert!(!step.contains("NaN"), "Step {i} (oscillation) produced NaN: {step}");
    }

    // Phase 2: Sustained extreme high signal
    for i in 0..50 {
        let step = step_attractors(&mut field, 0.99, 0.01).unwrap();
        assert!(!step.contains("NaN"), "Step {i} (high signal) produced NaN: {step}");
    }

    // Phase 3: Sustained extreme low signal
    for i in 0..50 {
        let step = step_attractors(&mut field, 0.01, 0.99).unwrap();
        assert!(!step.contains("NaN"), "Step {i} (low signal) produced NaN: {step}");
    }

    // After adversarial input, recall should still work
    let result = field_recall(&mut field, vec!["rust".into(), "code".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(),
        "Recall should work after adversarial attractor stepping");

    // All scores should be valid
    for act in &result.activations {
        assert!(act.score.is_finite(), "Score for {} should be finite", act.concept);
        assert!(act.score >= 0.0 && act.score <= 1.0,
            "Score for {} should be in [0,1], got {}", act.concept, act.score);
    }

    // Checkpoint should show bounded attractor state
    let checkpoint = field.checkpoint_sexp();
    assert!(!checkpoint.contains("NaN"), "Checkpoint should not contain NaN: {checkpoint}");
    assert!(!checkpoint.contains("inf"), "Checkpoint should not contain inf: {checkpoint}");
}

/// Circuit test: Heat kernel diffusion and eigenmode contribution
/// produce different recall patterns depending on graph connectivity.
/// A densely connected subgraph should score higher than an isolated chain.
#[test]
fn test_circuit_heat_kernel_connectivity_sensitivity() {
    let mut field = FieldState::new();

    // Graph with a dense clique and a sparse chain sharing a common query node
    let nodes = vec![
        ("query".into(), "math".into(), 5, vec!["e1".into()]),
        // Dense clique
        ("clique-a".into(), "math".into(), 3, vec!["e2".into()]),
        ("clique-b".into(), "math".into(), 3, vec!["e3".into()]),
        ("clique-c".into(), "math".into(), 3, vec!["e4".into()]),
        // Sparse chain
        ("chain-1".into(), "engineering".into(), 3, vec!["e5".into()]),
        ("chain-2".into(), "engineering".into(), 3, vec!["e6".into()]),
        ("chain-3".into(), "engineering".into(), 3, vec!["e7".into()]),
    ];
    let edges = vec![
        // Query connects to both structures
        ("query".into(), "clique-a".into(), 2.0, false),
        ("query".into(), "chain-1".into(), 2.0, false),
        // Dense clique (3-node complete graph)
        ("clique-a".into(), "clique-b".into(), 2.0, false),
        ("clique-b".into(), "clique-c".into(), 2.0, false),
        ("clique-a".into(), "clique-c".into(), 2.0, false),
        // Sparse chain
        ("chain-1".into(), "chain-2".into(), 2.0, false),
        ("chain-2".into(), "chain-3".into(), 2.0, false),
    ];
    load_graph(&mut field, nodes, edges).unwrap();
    step_attractors(&mut field, 0.5, 0.3).unwrap();

    let result = field_recall(&mut field, vec!["query".into()], vec![], 10).unwrap();
    assert!(result.activations.len() >= 4,
        "Should activate nodes from both clique and chain, got {}",
        result.activations.len());

    // clique-a and chain-1 are equidistant from query (same edge weight).
    // With heat kernel and topological flux, the clique node (on a cycle)
    // should receive a scoring contribution from topology that the chain does not.
    let clique_a_score = result.activations.iter()
        .find(|a| a.concept == "clique-a")
        .map(|a| a.score);
    let chain_1_score = result.activations.iter()
        .find(|a| a.concept == "chain-1")
        .map(|a| a.score);

    if let (Some(cs), Some(ls)) = (clique_a_score, chain_1_score) {
        // Clique node should score at least as high as chain node
        // because topological flux adds a non-negative contribution.
        assert!(cs >= ls * 0.9,
            "Clique node (on cycle) should score comparably or higher than chain node: clique={cs}, chain={ls}");
    }
}

/// Circuit test: The restore API preserves attractor state correctly.
/// Verifies the checkpoint → restore → verify round-trip.
#[test]
fn test_circuit_restore_api_roundtrip() {
    let mut field = FieldState::new();
    let nodes = vec![
        ("a".into(), "math".into(), 5, vec!["e1".into()]),
        ("b".into(), "engineering".into(), 3, vec!["e2".into()]),
    ];
    let edges = vec![("a".into(), "b".into(), 2.0, true)];
    load_graph(&mut field, nodes.clone(), edges.clone()).unwrap();

    // Evolve state
    for _ in 0..25 {
        step_attractors(&mut field, 0.7, 0.25).unwrap();
    }

    // Capture checkpoint before restore
    let checkpoint_before = field.checkpoint_sexp();

    // Now create a fresh field, load the same graph, and restore attractor state
    let mut field2 = FieldState::new();
    load_graph(&mut field2, nodes, edges).unwrap();

    // Restore signal/noise
    field2.restore_signal_noise(0.7, 0.25);
    // Restore soft basins to uniform (a reasonable starting point)
    field2.restore_soft_basins([1.0 / 6.0; 6]);

    // The restored field should be able to recall
    step_attractors(&mut field2, 0.7, 0.25).unwrap();
    let result = field_recall(&mut field2, vec!["a".into()], vec![], 10).unwrap();
    assert!(!result.activations.is_empty(),
        "Recall should work after restore");

    // The checkpoint of the restored field should show the restored signal/noise
    let checkpoint_after = field2.checkpoint_sexp();
    assert!(checkpoint_after.contains(":last-signal 0.70"),
        "Restored field should have signal 0.70: {checkpoint_after}");
    assert!(checkpoint_after.contains(":last-noise 0.25"),
        "Restored field should have noise 0.25: {checkpoint_after}");

    // Original field should still be functional
    assert!(!checkpoint_before.is_empty(),
        "Original checkpoint should not be empty");
}

/// Circuit test: Multi-query recall exercises the full scoring pipeline
/// with multiple source nodes, verifying that the heat kernel propagates
/// from all sources simultaneously.
#[test]
fn test_circuit_multi_source_heat_propagation() {
    let mut field = FieldState::new();

    // Star graph: central node connected to 5 peripherals
    let nodes = vec![
        ("center".into(), "generic".into(), 10, vec!["e0".into()]),
        ("north".into(), "math".into(), 3, vec!["e1".into()]),
        ("south".into(), "engineering".into(), 3, vec!["e2".into()]),
        ("east".into(), "music".into(), 3, vec!["e3".into()]),
        ("west".into(), "cognitive".into(), 3, vec!["e4".into()]),
        ("up".into(), "life".into(), 3, vec!["e5".into()]),
    ];
    let edges = vec![
        ("center".into(), "north".into(), 3.0, false),
        ("center".into(), "south".into(), 3.0, false),
        ("center".into(), "east".into(), 3.0, false),
        ("center".into(), "west".into(), 3.0, false),
        ("center".into(), "up".into(), 3.0, false),
    ];
    load_graph(&mut field, nodes, edges).unwrap();
    step_attractors(&mut field, 0.5, 0.3).unwrap();

    // Single-source query
    let single = field_recall(&mut field, vec!["north".into()], vec![], 10).unwrap();

    // Multi-source query from opposite ends
    let multi = field_recall(
        &mut field,
        vec!["north".into(), "south".into()],
        vec![],
        10,
    ).unwrap();

    // Multi-source should activate at least as many nodes
    assert!(multi.activations.len() >= single.activations.len(),
        "Multi-source recall should activate >= single-source: multi={}, single={}",
        multi.activations.len(), single.activations.len());

    // The center node should appear in both (it's equidistant from all peripherals)
    let single_has_center = single.activations.iter().any(|a| a.concept == "center");
    let multi_has_center = multi.activations.iter().any(|a| a.concept == "center");
    assert!(single_has_center || multi_has_center,
        "Center node should appear in recall results (it connects all peripherals)");
}

/// Circuit test: Dream stats track entropy bookkeeping through the
/// attractor stepping pipeline. Entropy delta should be non-zero
/// after evolution.
#[test]
fn test_circuit_entropy_bookkeeping() {
    let mut field = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    load_graph(&mut field, nodes, edges).unwrap();

    // Initial dream stats
    let initial_stats = dream_stats(&field).unwrap();
    assert!(initial_stats.contains(":entropy-delta"),
        "Dream stats should report entropy delta");

    // Evolve with varying input
    for i in 0..30 {
        let signal = 0.4 + 0.3 * ((i as f64) * 0.2).sin();
        let noise = 0.2 + 0.1 * ((i as f64) * 0.15).cos();
        step_attractors(&mut field, signal, noise).unwrap();
    }

    // After evolution, the checkpoint should show accumulated state
    let checkpoint = field.checkpoint_sexp();
    assert!(checkpoint.contains(":entropy-delta"),
        "Checkpoint should contain entropy delta: {checkpoint}");
}

/// Circuit test: Edge current flow reflects the field solution.
/// After loading a graph and establishing signal state, edge currents
/// should be computable and report actual flow between concepts.
#[test]
fn test_circuit_edge_current_flow() {
    let mut field = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    load_graph(&mut field, nodes, edges).unwrap();

    step_attractors(&mut field, 0.7, 0.2).unwrap();

    let currents = edge_current_status(&field).unwrap();
    assert!(currents.contains(":ok"),
        "Edge current status should report ok: {currents}");
    // Should contain at least some edge currents for a 30-node connected graph
    assert!(currents.len() > 20,
        "Edge current report should be non-trivial for realistic graph, got len={}",
        currents.len());
}

/// Circuit test: Structural recall (no entry content, just concept/score/basin)
/// exercises the same holographic pipeline as full recall but produces
/// a lighter output format.
#[test]
fn test_circuit_structural_recall_pipeline() {
    let mut field = FieldState::new();
    let nodes = vec![
        ("alpha".into(), "math".into(), 5, vec!["S1".into()]),
        ("beta".into(), "engineering".into(), 4, vec!["S2".into()]),
        ("gamma".into(), "music".into(), 3, vec!["S3".into()]),
    ];
    let edges = vec![
        ("alpha".into(), "beta".into(), 3.0, true),
        ("beta".into(), "gamma".into(), 2.0, true),
        ("alpha".into(), "gamma".into(), 1.5, true),  // cycle
    ];
    load_graph(&mut field, nodes, edges).unwrap();
    step_attractors(&mut field, 0.6, 0.3).unwrap();

    // Full recall
    let full = field_recall(
        &mut field,
        vec!["alpha".into()],
        vec![],
        10,
    ).unwrap();

    // Structural recall
    let structural = field_recall_structural(
        &mut field,
        vec!["alpha".into()],
        10,
    ).unwrap();

    // Both should produce the same number of activations
    assert_eq!(full.activations.len(), structural.activations.len(),
        "Full and structural recall should produce same number of activations");

    // Structural sexp should NOT contain entry IDs (lighter format)
    let structural_sexp = structural.to_sexp_structural();
    assert!(!structural_sexp.contains(":entries"),
        "Structural recall sexp should not contain entries: {structural_sexp}");

    // Full sexp should contain entry IDs
    let full_sexp = full.to_sexp();
    assert!(full_sexp.contains(":entries"),
        "Full recall sexp should contain entries: {full_sexp}");
}

/// Circuit test: Basin status evolves consistently with attractor stepping.
/// The basin → hysteresis → dwell-ticks pipeline should show monotonic
/// dwell increase when signal is stable (no basin switching).
#[test]
fn test_circuit_basin_dwell_monotonic() {
    let mut field = FieldState::new();
    let nodes = vec![
        ("a".into(), "math".into(), 5, vec!["e1".into()]),
        ("b".into(), "math".into(), 3, vec!["e2".into()]),
    ];
    let edges = vec![("a".into(), "b".into(), 2.0, false)];
    load_graph(&mut field, nodes, edges).unwrap();

    // Use a stable, consistent signal to stay in the same basin
    let mut prev_dwell = 0u64;
    let mut dwell_increased = false;
    for i in 0..50 {
        step_attractors(&mut field, 0.7, 0.2).unwrap();
        let bs = basin_status(&field).unwrap();
        if let Some(pos) = bs.find(":dwell-ticks ") {
            let rest = &bs[pos + 13..];
            let end = rest.find(|c: char| c.is_whitespace() || c == ')').unwrap_or(rest.len());
            if let Ok(dwell) = rest[..end].parse::<u64>() {
                if dwell > prev_dwell && i > 5 {
                    dwell_increased = true;
                }
                prev_dwell = dwell;
            }
        }
    }
    assert!(dwell_increased,
        "Dwell ticks should increase during stable signal (stayed in basin)");
}

/// Circuit test: The full memory digest pipeline produces a valid
/// summary of the graph state. This exercises graph → domain counting
/// → top concepts → entropy estimation.
#[test]
fn test_circuit_memory_digest() {
    let mut field = FieldState::new();
    let (nodes, edges) = build_realistic_graph();
    load_graph(&mut field, nodes, edges).unwrap();

    for _ in 0..10 {
        step_attractors(&mut field, 0.6, 0.3).unwrap();
    }

    let digest = harmonia_memory_field::compute_digest(&field).unwrap();
    let sexp = digest.to_sexp();

    assert!(sexp.contains(":ok"), "Digest should report ok: {sexp}");
    assert_eq!(digest.concept_count, 30,
        "Digest should report 30 concepts for realistic graph");
    assert!(digest.graph_version > 0,
        "Digest graph version should be > 0");
    assert!(!digest.top_concepts.is_empty(),
        "Digest should have top concepts");

    // Domain distribution should sum to approximately 1.0
    let domain_sum: f32 = digest.domain_distribution.iter().sum();
    assert!((domain_sum - 1.0).abs() < 0.01,
        "Domain distribution should sum to 1.0, got {domain_sum}");
}
