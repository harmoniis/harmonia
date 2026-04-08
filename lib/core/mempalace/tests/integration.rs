use harmonia_mempalace::*;

fn setup_palace() -> PalaceState {
    let mut s = PalaceState::new();
    add_node(&mut s, NodeKind::Wing, "project-x", Domain::Engineering).unwrap();
    add_node(&mut s, NodeKind::Room, "auth", Domain::Engineering).unwrap();
    add_edge(&mut s, 0, 1, EdgeKind::Contains, 1.0).unwrap();
    s
}

#[test]
fn test_add_node_and_edge() {
    let s = setup_palace();
    let stats = graph_stats(&s).unwrap();
    assert!(stats.contains(":nodes 2"));
    assert!(stats.contains(":edges 1"));
}

#[test]
fn test_duplicate_node_rejected() {
    let mut s = PalaceState::new();
    add_node(&mut s, NodeKind::Wing, "test", Domain::Generic).unwrap();
    assert!(add_node(&mut s, NodeKind::Wing, "test", Domain::Generic).is_err());
}

#[test]
fn test_file_and_search_drawer() {
    let mut s = PalaceState::new();
    add_node(&mut s, NodeKind::Room, "notes", Domain::Generic).unwrap();
    file_drawer(&mut s, "This is a test note about memory fields and attractors", 0, drawer::DrawerSource::Manual, &["test", "memory"]).unwrap();
    let results = search_drawers(&mut s, "memory", None, 10);
    assert!(results.is_ok());
    assert!(results.unwrap().contains(":count 1"));
}

#[test]
fn test_aaak_compress() {
    let mut s = PalaceState::new();
    add_node(&mut s, NodeKind::Room, "code", Domain::Engineering).unwrap();
    file_drawer(
        &mut s,
        "The memory field uses graph Laplacian for field propagation. The memory field is based on spectral decomposition. Memory field recall uses attractor basins.",
        0, drawer::DrawerSource::Manual, &["memory"],
    ).unwrap();
    let result = compress_aaak(&mut s, &[1]);
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.contains(":aaak"));
    assert!(r.contains("memory"));
}

#[test]
fn test_graph_stats() {
    let s = setup_palace();
    let stats = graph_stats(&s);
    assert!(stats.is_ok());
    let r = stats.unwrap();
    assert!(r.contains(":wings 1"));
    assert!(r.contains(":rooms 1"));
}

#[test]
fn test_context_tiers() {
    let mut s = setup_palace();
    file_drawer(&mut s, "Backend service handles authentication and session management", 1, drawer::DrawerSource::Manual, &["backend"]).unwrap();
    let l0 = context_l0(&s);
    assert!(l0.is_ok());
    assert!(l0.unwrap().contains("project-x"));
    let l1 = context_l1(&s);
    assert!(l1.is_ok());
    let l2 = context_l2(&mut s, "engineering");
    assert!(l2.is_ok());
}

#[test]
fn test_find_tunnels() {
    let mut s = PalaceState::new();
    add_node(&mut s, NodeKind::Wing, "project-a", Domain::Engineering).unwrap();
    add_node(&mut s, NodeKind::Wing, "project-b", Domain::Engineering).unwrap();
    add_node(&mut s, NodeKind::Room, "auth", Domain::Engineering).unwrap();
    add_edge(&mut s, 0, 2, EdgeKind::Contains, 1.0).unwrap();
    add_edge(&mut s, 1, 2, EdgeKind::Contains, 1.0).unwrap();
    let tunnels = find_tunnels(&mut s);
    assert!(tunnels.is_ok());
    assert!(tunnels.unwrap().contains("auth"));
}

#[test]
fn test_codebook_persistence() {
    let mut s = PalaceState::new();
    s.codebook.code_for("memory-field");
    s.codebook.code_for("spectral");
    let json = s.codebook.to_json();
    let restored = codebook::AaakCodebook::from_json(&json);
    assert_eq!(restored.len(), 2);
    assert_eq!(restored.lookup("memory-field"), Some("A".into()));
}

#[test]
fn test_health_check() {
    let s = PalaceState::new();
    let h = health_check(&s);
    assert!(h.is_ok());
    assert!(h.unwrap().contains(":healthy t"));
}
