use std::collections::HashMap;
use std::env;

#[derive(Clone, Debug)]
pub(crate) struct Edge {
    pub(crate) weight: f64,
    pub(crate) min_harmony: f64,
    pub(crate) uses: u64,
    pub(crate) successes: u64,
    pub(crate) total_latency_ms: u64,
    pub(crate) total_cost_usd: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct RouteSample {
    pub(crate) ts: u64,
    pub(crate) success: bool,
    pub(crate) latency_ms: u64,
    pub(crate) cost_usd: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct MatrixEvent {
    pub(crate) ts: u64,
    pub(crate) component: String,
    pub(crate) direction: String,
    pub(crate) channel: String,
    pub(crate) payload: String,
    pub(crate) success: bool,
    pub(crate) error: String,
}

#[derive(Default, Clone)]
pub(crate) struct State {
    pub(crate) nodes: HashMap<String, String>,
    pub(crate) edges: HashMap<(String, String), Edge>,
    pub(crate) plugged: HashMap<String, bool>,
    pub(crate) route_history: HashMap<(String, String), Vec<RouteSample>>,
    pub(crate) events: Vec<MatrixEvent>,
    pub(crate) epoch: u64,
    pub(crate) revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StoreKind {
    Memory,
    Sqlite,
    Graph,
}

#[derive(Clone, Debug)]
pub(crate) struct StoreConfig {
    pub(crate) kind: StoreKind,
    pub(crate) path: String,
}

impl Default for StoreConfig {
    fn default() -> Self {
        let kind = match env::var("HARMONIA_MATRIX_STORE_KIND") {
            Ok(v) if v.eq_ignore_ascii_case("sqlite") || v.eq_ignore_ascii_case("sql") => {
                StoreKind::Sqlite
            }
            Ok(v) if v.eq_ignore_ascii_case("graph") => StoreKind::Graph,
            _ => StoreKind::Memory,
        };
        let path = match kind {
            StoreKind::Memory => env::var("HARMONIA_MATRIX_DB")
                .unwrap_or_else(|_| "/tmp/harmonia/harmonic-matrix.db".to_string()),
            StoreKind::Sqlite => env::var("HARMONIA_MATRIX_DB")
                .unwrap_or_else(|_| "/tmp/harmonia/harmonic-matrix.db".to_string()),
            StoreKind::Graph => env::var("HARMONIA_MATRIX_GRAPH_URI")
                .unwrap_or_else(|_| "bolt://127.0.0.1:7687".to_string()),
        };
        Self { kind, path }
    }
}

impl StoreConfig {
    pub(crate) fn kind_name(&self) -> &'static str {
        match self.kind {
            StoreKind::Memory => "memory",
            StoreKind::Sqlite => "sqlite",
            StoreKind::Graph => "graph",
        }
    }
}
