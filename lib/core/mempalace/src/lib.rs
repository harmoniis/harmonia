pub(crate) use harmonia_actor_protocol::define_sexp_enum;

mod aaak;
pub mod codebook;
pub mod compress;
pub mod drawer;
pub mod graph;
mod layers;
mod query;
mod sexp;

pub use harmonia_actor_protocol::MemoryError;
pub use graph::{Domain, EdgeKind, GraphEdge, GraphNode, NodeKind};
pub use aaak::{codebook_lookup, compress_aaak};
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
}

pub fn init(s: &mut PalaceState) -> Result<String, MemoryError> {
    if let Ok(Some(cb_json)) = harmonia_config_store::get_own("mempalace", "codebook") {
        s.codebook = codebook::AaakCodebook::from_json(&cb_json);
    }
    Ok(format!(
        "(:ok :nodes {} :drawers {} :codebook-entries {})",
        s.graph.nodes.len(), s.drawers.len(), s.codebook.len(),
    ))
}

pub fn persist(s: &PalaceState) -> Result<String, MemoryError> {
    let cb_json = s.codebook.to_json();
    harmonia_config_store::set_config("mempalace", "mempalace", "codebook", &cb_json)
        .map_err(|e| MemoryError::PersistenceFailed(e))?;
    Ok("(:ok :persisted t)".into())
}

pub fn health_check(s: &PalaceState) -> Result<String, MemoryError> {
    Ok(format!(
        "(:ok :healthy t :nodes {} :edges {} :drawers {} :codebook {})",
        s.graph.nodes.len(), s.graph.edges.len(), s.drawers.len(), s.codebook.len(),
    ))
}
