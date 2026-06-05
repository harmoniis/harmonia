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
fn test_duplicate_node_is_idempotent() {
    let mut s = PalaceState::new();
    let first = add_node(&mut s, NodeKind::Wing, "test", Domain::Generic).unwrap();
    let second = add_node(&mut s, NodeKind::Wing, "test", Domain::Generic).unwrap();
    assert!(first.contains(":id 0"));
    assert!(second.contains(":id 0"));
    assert!(graph_stats(&s).unwrap().contains(":nodes 1"));
}

#[test]
fn test_duplicate_edge_is_idempotent() {
    let mut s = PalaceState::new();
    add_node(&mut s, NodeKind::Wing, "project", Domain::Engineering).unwrap();
    add_node(&mut s, NodeKind::Room, "notes", Domain::Engineering).unwrap();
    add_edge(&mut s, 0, 1, EdgeKind::Contains, 1.0).unwrap();
    add_edge(&mut s, 0, 1, EdgeKind::Contains, 1.0).unwrap();
    assert!(graph_stats(&s).unwrap().contains(":edges 1"));
}

#[test]
fn test_file_and_search_drawer() {
    let mut s = PalaceState::new();
    add_node(&mut s, NodeKind::Room, "notes", Domain::Generic).unwrap();
    file_drawer(
        &mut s,
        "This is a test note about memory fields and attractors",
        0,
        drawer::DrawerSource::Manual,
        &["test", "memory"],
    )
    .unwrap();
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
    file_drawer(
        &mut s,
        "Backend service handles authentication and session management",
        1,
        drawer::DrawerSource::Manual,
        &["backend"],
    )
    .unwrap();
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
    let sexp = s.codebook.to_sexp();
    let restored = codebook::AaakCodebook::from_sexp(&sexp);
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

/// P3 guard: persist-before-acknowledge + warm-start from the on-disk journal.
/// Mutations are durable the instant the op returns; a fresh actor restores them
/// from disk with IDENTICAL counts (no rebuild, no duplication), and the chronicle
/// entry-id survives so boot reconciliation can diff against it.
#[test]
fn test_persistence_survives_reload_without_duplication() {
    let dir = std::env::temp_dir()
        .join(format!("harmonia-mempalace-survive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // First actor lifetime: warm-start empty, then mutate. Each op persists to the
    // disk journal before returning Ok — nothing is flushed on drop.
    {
        let mut s = PalaceState::load_from_dir(&dir).unwrap();
        add_node(&mut s, NodeKind::Room, "notes", Domain::Generic).unwrap();
        file_drawer(
            &mut s,
            "Durable fact: the probe code is ZX42.",
            0,
            drawer::DrawerSource::Memory { entry_id: "daily-100-1".into() },
            &["probe"],
        )
        .unwrap();
    }

    assert!(dir.join("graph.sexp").exists());
    assert!(dir.join("drawers").join(format!("{:016}.sexp", 1)).exists());

    // Reload from disk only, twice — counts must be IDENTICAL each time (no rebuild,
    // no doubling), and the entry-id must survive exactly once.
    for _ in 0..2 {
        let mut s = PalaceState::load_from_dir(&dir).unwrap();
        assert!(graph_stats(&s).unwrap().contains(":nodes 1"));
        assert!(search_drawers(&mut s, "ZX42", None, 10).unwrap().contains(":count 1"));
        let ids = entry_ids(&s).unwrap();
        assert!(ids.contains(":count 1"));
        assert!(ids.contains("daily-100-1"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
