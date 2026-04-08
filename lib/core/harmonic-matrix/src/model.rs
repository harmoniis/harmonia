use std::collections::HashMap;

const COMPONENT: &str = "harmonic-matrix";

fn state_root() -> String {
    let default = std::env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", &default)
        .unwrap_or_else(|_| default)
}

fn default_matrix_db() -> String {
    format!("{}/harmonic-matrix.db", state_root())
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub weight: f64,
    pub min_harmony: f64,
    pub uses: u64,
    pub successes: u64,
    pub total_latency_ms: u64,
    pub total_cost_usd: f64,
}

#[derive(Clone, Debug)]
pub struct RouteSample {
    pub ts: u64,
    pub success: bool,
    pub latency_ms: u64,
    pub cost_usd: f64,
}

#[derive(Clone, Debug)]
pub struct MatrixEvent {
    pub ts: u64,
    pub component: String,
    pub direction: String,
    pub channel: String,
    pub payload: String,
    pub success: bool,
    pub error: String,
}

#[derive(Default, Clone)]
pub struct State {
    pub nodes: HashMap<String, String>,
    pub edges: HashMap<(String, String), Edge>,
    pub plugged: HashMap<String, bool>,
    pub route_history: HashMap<(String, String), Vec<RouteSample>>,
    pub events: Vec<MatrixEvent>,
    pub epoch: u64,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreKind {
    Memory,
    Sqlite,
    Graph,
}

#[derive(Clone, Debug)]
pub struct StoreConfig {
    pub kind: StoreKind,
    pub path: String,
}

impl Default for StoreConfig {
    fn default() -> Self {
        let kind = match harmonia_config_store::get_own(COMPONENT, "store-kind") {
            Ok(Some(v)) if v.eq_ignore_ascii_case("sqlite") || v.eq_ignore_ascii_case("sql") => {
                StoreKind::Sqlite
            }
            Ok(Some(v)) if v.eq_ignore_ascii_case("graph") => StoreKind::Graph,
            _ => StoreKind::Memory,
        };
        let path = match kind {
            StoreKind::Memory => harmonia_config_store::get_own(COMPONENT, "db")
                .ok()
                .flatten()
                .unwrap_or_else(default_matrix_db),
            StoreKind::Sqlite => harmonia_config_store::get_own(COMPONENT, "db")
                .ok()
                .flatten()
                .unwrap_or_else(default_matrix_db),
            StoreKind::Graph => harmonia_config_store::get_own(COMPONENT, "graph-uri")
                .ok()
                .flatten()
                .unwrap_or_default(),
        };
        Self { kind, path }
    }
}

impl StoreConfig {
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            StoreKind::Memory => "memory",
            StoreKind::Sqlite => "sqlite",
            StoreKind::Graph => "graph",
        }
    }
}
