pub(crate) use harmonia_actor_protocol::define_sexp_enum;

mod aaak;
pub mod codebook;
pub mod command;
pub mod compress;
pub(crate) mod disk;
pub mod drawer;
pub mod graph;
mod interpret;
mod layers;
mod query;
mod sexp;

pub use harmonia_actor_protocol::MemoryError;
pub use graph::{Domain, EdgeKind, GraphEdge, GraphNode, NodeKind};
pub use aaak::{codebook_lookup, codebook_register, compress_aaak};
pub use drawer::{file_drawer, get_drawer, search_drawers};
pub use graph::{add_edge, add_node, find_tunnels, graph_stats};
pub use layers::{context_l0, context_l1, context_l2, context_l3};
pub use query::{query_graph, Traversal};

pub(crate) use harmonia_actor_protocol::sexp_escape;
pub(crate) use harmonia_actor_protocol::truncate_safe;

pub(crate) fn current_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn cfg_usize(key: &str, default: usize) -> usize {
    harmonia_config_store::get_own("mempalace", key)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(default)
}

pub struct PalaceState {
    pub(crate) graph: graph::KnowledgeGraph,
    pub(crate) drawers: drawer::DrawerStore,
    pub codebook: codebook::AaakCodebook,
    pub(crate) next_drawer_id: u64,
}

impl PalaceState {
    pub fn new() -> Self {
        Self {
            graph: graph::KnowledgeGraph::new(),
            drawers: drawer::DrawerStore::new(),
            codebook: codebook::AaakCodebook::new(),
            next_drawer_id: 1,
        }
    }

    /// Resolve wing and room labels for a given room_id.
    /// Walks the graph: room -> Contains edge -> wing.
    pub(crate) fn resolve_wing_room(&self, room_id: u32) -> (String, String) {
        let room_label = self
            .graph
            .nodes
            .iter()
            .find(|n| n.id == room_id)
            .map(|n| n.label.clone())
            .unwrap_or_else(|| format!("room-{}", room_id));

        // Find wing that contains this room via a Contains edge
        let wing_label = self
            .graph
            .edges
            .iter()
            .find(|e| e.target == room_id && e.kind == graph::EdgeKind::Contains)
            .and_then(|e| self.graph.nodes.iter().find(|n| n.id == e.source))
            .filter(|n| n.kind == graph::NodeKind::Wing)
            .map(|n| n.label.clone())
            .unwrap_or_else(|| "default".to_string());

        (wing_label, room_label)
    }
}

pub fn init(s: &mut PalaceState) -> Result<String, MemoryError> {
    // Try disk first (primary storage)
    if let Some(root) = disk::memory_root() {
        // Load drawers from .md files
        let disk_drawers = disk::load_all_drawers(&root);
        for (id, content, source, room_id, chunk_index, created_at, tags) in disk_drawers {
            s.drawers.restore(id, content, source, room_id, chunk_index, created_at, tags);
            s.next_drawer_id = s.next_drawer_id.max(id + 1);
        }

        // Load graph from index.sexp
        let graph_path = root.join("palace").join("index.sexp");
        if let Some((nodes, edges)) = disk::load_graph_index(&graph_path) {
            for (id, kind, label, domain, created_at) in nodes {
                s.graph.restore_node(id, &kind, &label, &domain, created_at);
            }
            for (source, target, kind, weight, confidence) in edges {
                s.graph.restore_edge(source, target, &kind, weight, confidence);
            }
            if !s.graph.edges.is_empty() {
                s.graph.rebuild_csr();
            }
        }

        // Codebook: disk first, config-store fallback
        let cb_path = root.join("codebook.json");
        if let Ok(json) = std::fs::read_to_string(&cb_path) {
            s.codebook = codebook::AaakCodebook::from_json(&json);
        } else if let Ok(Some(json)) = harmonia_config_store::get_own("mempalace", "codebook") {
            s.codebook = codebook::AaakCodebook::from_json(&json);
        }
    } else {
        // No memory root -- restore codebook from config-store only
        if let Ok(Some(cb_json)) = harmonia_config_store::get_own("mempalace", "codebook") {
            s.codebook = codebook::AaakCodebook::from_json(&cb_json);
        }
    }

    Ok(format!(
        "(:ok :nodes {} :drawers {} :codebook-entries {})",
        s.graph.nodes.len(), s.drawers.len(), s.codebook.len(),
    ))
}

pub fn persist(s: &PalaceState) -> Result<String, MemoryError> {
    // Write to disk if memory root is configured
    if let Some(root) = disk::memory_root() {
        // Write all drawer .md files
        for d in s.drawers.all() {
            let (wing, room) = s.resolve_wing_room(d.room_id);
            let path = disk::drawer_md_path(&root, &wing, &room, d.id);
            let md = disk::drawer_to_md(d);
            let _ = disk::write_drawer_md(&path, &md);
        }
        // Write graph index
        let graph_path = root.join("palace").join("index.sexp");
        let _ = disk::write_graph_index(&graph_path, &s.graph);
        // Write codebook to disk
        let cb_path = root.join("codebook.json");
        let tmp = cb_path.with_extension("json.tmp");
        let _ = std::fs::write(&tmp, s.codebook.to_json());
        let _ = std::fs::rename(&tmp, &cb_path);
    }

    // Keep config-store codebook as backup
    let cb_json = s.codebook.to_json();
    harmonia_config_store::set_config("mempalace", "mempalace", "codebook", &cb_json)
        .map_err(|e| MemoryError::PersistenceFailed(e))?;

    Ok(format!(
        "(:ok :persisted t :drawers {} :nodes {} :edges {})",
        s.drawers.len(), s.graph.nodes.len(), s.graph.edges.len(),
    ))
}

pub fn health_check(s: &PalaceState) -> Result<String, MemoryError> {
    Ok(format!(
        "(:ok :healthy t :nodes {} :edges {} :drawers {} :codebook {})",
        s.graph.nodes.len(), s.graph.edges.len(), s.drawers.len(), s.codebook.len(),
    ))
}
