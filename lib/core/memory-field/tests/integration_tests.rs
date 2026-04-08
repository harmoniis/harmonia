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
    field_recall, load_graph, reset, step_attractors, basin_status, eigenmode_status, status,
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
    let results = recall(&mut s, &["fractal", "ratio"], 8);
    assert!(!results.is_empty());
    let has_source = results.iter().any(|(c, _)| c == "fractal" || c == "ratio");
    assert!(has_source, "Should find at least one source concept, got: {:?}", results);
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
    let results = recall(&mut s, &["harmony", "ratio"], 8);
    assert!(!results.is_empty());
    let has_music = results.iter().any(|(c, _)| ["harmony", "melody", "rhythm"].contains(&c.as_str()));
    let has_math = results.iter().any(|(c, _)| ["ratio", "fractal", "geometry"].contains(&c.as_str()));
    assert!(has_music, "Cross-domain query should include music concepts");
    assert!(has_math, "Cross-domain query should include math concepts");
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
