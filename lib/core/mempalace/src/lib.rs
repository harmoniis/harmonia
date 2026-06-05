pub(crate) use harmonia_actor_protocol::define_sexp_enum;

mod aaak;
pub mod codebook;
pub mod compress;
pub(crate) mod disk;
pub mod drawer;
pub mod graph;
mod layers;
mod query;
mod sexp;

pub use harmonia_actor_protocol::MemoryError;
pub use graph::{Domain, EdgeKind, GraphEdge, GraphNode, NodeKind};
pub use aaak::{codebook_lookup, codebook_register, compress_aaak};
pub use drawer::{entry_ids, file_drawer, get_drawer, search_drawers};
pub use graph::{add_edge, add_node, find_tunnels, graph_stats};
pub use layers::{context_l0, context_l1, context_l2, context_l3};
pub use query::{query_graph, Traversal};

pub(crate) use harmonia_actor_protocol::sexp_escape;
pub(crate) use harmonia_actor_protocol::truncate_safe;
use std::path::{Path, PathBuf};

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
    pub(crate) storage_dir: Option<PathBuf>,
}

impl PalaceState {
    pub fn new() -> Self {
        Self {
            graph: graph::KnowledgeGraph::new(),
            drawers: drawer::DrawerStore::new(),
            codebook: codebook::AaakCodebook::new(),
            next_drawer_id: 1,
            storage_dir: None,
        }
    }

    pub fn load_or_empty() -> Result<Self, MemoryError> {
        Self::load_from_dir(disk::state_dir())
    }

    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, MemoryError> {
        let dir = dir.as_ref().to_path_buf();
        let (graph, drawers, codebook, next_drawer_id) = disk::load_state(&dir)?;
        Ok(Self { graph, drawers, codebook, next_drawer_id, storage_dir: Some(dir) })
    }

    pub(crate) fn persist_graph(&self) -> Result<(), MemoryError> {
        if let Some(dir) = &self.storage_dir {
            disk::write_graph(dir, &self.graph)?;
        }
        Ok(())
    }

    pub(crate) fn persist_drawer(&self, drawer: &drawer::Drawer) -> Result<(), MemoryError> {
        if let Some(dir) = &self.storage_dir {
            disk::write_drawer(dir, drawer)?;
        }
        Ok(())
    }

    pub(crate) fn persist_codebook(&self) -> Result<(), MemoryError> {
        if let Some(dir) = &self.storage_dir {
            disk::write_codebook(dir, &self.codebook)?;
        }
        Ok(())
    }
}

pub fn init(s: &mut PalaceState) -> Result<String, MemoryError> {
    Ok(format!(
        "(:ok :nodes {} :drawers {} :codebook-entries {})",
        s.graph.nodes.len(), s.drawers.len(), s.codebook.len(),
    ))
}

pub fn health_check(s: &PalaceState) -> Result<String, MemoryError> {
    Ok(format!(
        "(:ok :healthy t :nodes {} :edges {} :drawers {} :codebook {})",
        s.graph.nodes.len(), s.graph.edges.len(), s.drawers.len(), s.codebook.len(),
    ))
}
