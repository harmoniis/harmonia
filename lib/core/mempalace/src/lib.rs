macro_rules! define_sexp_enum {
    ($name:ident, $default:ident { $($variant:ident => $kw:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $($variant),* }
        impl $name {
            pub fn to_sexp(&self) -> &'static str {
                match self { $(Self::$variant => concat!(":", $kw)),* }
            }
            /// Parse from sexp keyword. Returns None for unrecognized input.
            pub fn try_from_sexp(s: &str) -> Option<Self> {
                let s = s.strip_prefix(':').unwrap_or(s);
                match s { $($kw => Some(Self::$variant),)* _ => None, }
            }
            /// Parse from sexp keyword with default fallback.
            /// Use try_from_sexp() at trust boundaries.
            pub fn from_str(s: &str) -> Self {
                Self::try_from_sexp(s).unwrap_or(Self::$default)
            }
        }
    };
}
pub(crate) use define_sexp_enum;

mod aaak;
pub mod codebook;
pub mod compress;
pub mod drawer;
pub mod graph;
mod layers;
mod query;
mod sexp;

pub use graph::{Domain, EdgeKind, GraphEdge, GraphNode, NodeKind};
pub use aaak::{codebook_lookup, compress_aaak};
pub use drawer::{file_drawer, get_drawer, search_drawers};
pub use graph::{add_edge, add_node, find_tunnels, graph_stats};
pub use layers::{context_l0, context_l1, context_l2, context_l3};
pub use query::{query_graph, Traversal};

pub(crate) fn sexp_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn truncate_safe(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes { return s; }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

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

pub fn init(s: &mut PalaceState) -> Result<String, String> {
    if let Ok(Some(cb_json)) = harmonia_config_store::get_own("mempalace", "codebook") {
        s.codebook = codebook::AaakCodebook::from_json(&cb_json);
    }
    Ok(format!(
        "(:ok :nodes {} :drawers {} :codebook-entries {})",
        s.graph.nodes.len(), s.drawers.len(), s.codebook.len(),
    ))
}

pub fn persist(s: &PalaceState) -> Result<String, String> {
    let cb_json = s.codebook.to_json();
    harmonia_config_store::set_config("mempalace", "mempalace", "codebook", &cb_json)
        .map_err(|e| format!("(:error \"persist failed: {}\")", sexp_escape(&e)))?;
    Ok("(:ok :persisted t)".into())
}

pub fn health_check(s: &PalaceState) -> Result<String, String> {
    Ok(format!(
        "(:ok :healthy t :nodes {} :edges {} :drawers {} :codebook {})",
        s.graph.nodes.len(), s.graph.edges.len(), s.drawers.len(), s.codebook.len(),
    ))
}
