use harmonia_actor_protocol::MemoryError;

use crate::{sexp_escape, current_epoch_ms, cfg_usize};

// ── Types ──

crate::define_sexp_enum!(NodeKind, Concept {
    Wing => "wing",
    Room => "room",
    Entity => "entity",
    Concept => "concept",
    Tunnel => "tunnel",
});

crate::define_sexp_enum!(Domain, Generic {
    Music => "music",
    Math => "math",
    Engineering => "engineering",
    Cognitive => "cognitive",
    Life => "life",
    Generic => "generic",
    System => "system",
});

crate::define_sexp_enum!(EdgeKind, RelatesTo {
    Contains => "contains",
    RelatesTo => "relates-to",
    Bridges => "bridges",
    Temporal => "temporal",
    Causal => "causal",
});

#[derive(Clone, Debug)]
pub struct GraphNode {
    pub id: u32,
    pub kind: NodeKind,
    pub label: String,
    pub domain: Domain,
    pub created_at: u64,
    pub properties: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct GraphEdge {
    pub source: u32,
    pub target: u32,
    pub kind: EdgeKind,
    pub weight: f64,
    pub valid_from: u64,
    pub valid_to: Option<u64>,
    pub confidence: f64,
}

impl GraphEdge {
    pub fn is_valid_at(&self, timestamp: u64) -> bool {
        timestamp >= self.valid_from && self.valid_to.map_or(true, |to| timestamp <= to)
    }
}

pub struct KnowledgeGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub offsets: Vec<usize>,
    pub targets: Vec<usize>,
    pub weights: Vec<f64>,
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self { nodes: Vec::new(), edges: Vec::new(), offsets: vec![0], targets: Vec::new(), weights: Vec::new() }
    }

    pub fn rebuild_csr(&mut self) {
        let n = self.nodes.len();
        let now = current_epoch_ms();
        let valid_edges: Vec<(usize, usize, f64)> = self.edges.iter()
            .filter(|e| e.is_valid_at(now))
            .filter(|e| (e.source as usize) < n && (e.target as usize) < n)
            .flat_map(|e| {
                let (s, t) = (e.source as usize, e.target as usize);
                vec![(s, t, e.weight), (t, s, e.weight)].into_iter()
            })
            .collect();
        let degree: Vec<usize> = valid_edges.iter()
            .fold(vec![0usize; n], |mut deg, &(src, _, _)| { deg[src] += 1; deg });
        self.offsets = std::iter::once(0)
            .chain(degree.iter().scan(0usize, |acc, &d| { *acc += d; Some(*acc) }))
            .collect();
        let total = *self.offsets.last().unwrap_or(&0);
        self.targets = vec![0; total];
        self.weights = vec![0.0; total];
        let mut cursor = vec![0usize; n];
        for &(src, tgt, weight) in &valid_edges {
            let idx = self.offsets[src] + cursor[src];
            self.targets[idx] = tgt;
            self.weights[idx] = weight;
            cursor[src] += 1;
        }
    }

    pub fn neighbors(&self, node: usize) -> &[usize] {
        if node + 1 < self.offsets.len() {
            &self.targets[self.offsets[node]..self.offsets[node + 1]]
        } else {
            &[]
        }
    }

    pub fn find_node(&self, label: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.label == label)
    }

    pub fn find_node_by_id(&self, id: u32) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Restore a node from persisted Chronicle data (warm-start).
    pub fn restore_node(&mut self, id: u32, kind: &str, label: &str, domain: &str, created_at: u64) {
        let node = GraphNode {
            id,
            kind: NodeKind::from_str(kind),
            label: label.to_string(),
            domain: Domain::from_str(domain),
            created_at,
            properties: Vec::new(),
        };
        self.nodes.push(node);
        self.offsets.push(*self.offsets.last().unwrap_or(&0));
    }

    /// Restore an edge from persisted Chronicle data (warm-start).
    pub fn restore_edge(&mut self, source: u32, target: u32, kind: &str, weight: f64, confidence: f64) {
        let edge = GraphEdge {
            source,
            target,
            kind: EdgeKind::from_str(kind),
            weight,
            valid_from: 0,
            valid_to: None,
            confidence,
        };
        self.edges.push(edge);
    }
}

// ── ConceptGraph trait implementation ──

impl harmonia_actor_protocol::ConceptGraph for KnowledgeGraph {
    fn node_count(&self) -> usize { self.nodes.len() }

    fn concept_index(&self, concept: &str) -> Option<usize> {
        self.find_node(concept)
    }

    fn neighbor_indices(&self, node: usize) -> &[usize] {
        if node + 1 < self.offsets.len() {
            &self.targets[self.offsets[node]..self.offsets[node + 1]]
        } else {
            &[]
        }
    }

    fn degree(&self, node: usize) -> f64 {
        if node + 1 < self.offsets.len() {
            self.weights[self.offsets[node]..self.offsets[node + 1]].iter().sum()
        } else {
            0.0
        }
    }

    fn edge_weight(&self, from: usize, to: usize) -> f64 {
        if from + 1 >= self.offsets.len() { return 0.0; }
        let start = self.offsets[from];
        let end = self.offsets[from + 1];
        for idx in start..end {
            if self.targets[idx] == to {
                return self.weights[idx];
            }
        }
        0.0
    }
}

// ── Public API ──

pub fn add_node(
    s: &mut crate::PalaceState,
    kind: NodeKind,
    label: &str,
    domain: Domain,
) -> Result<String, MemoryError> {
    let max_nodes = cfg_usize("max-nodes", 1024);
    if s.graph.nodes.len() >= max_nodes {
        return Err(MemoryError::CapacityExceeded { kind: "nodes", limit: max_nodes });
    }
    if s.graph.find_node(label).is_some() {
        return Err(MemoryError::DuplicateNode(label.into()));
    }
    let id = s.graph.nodes.len() as u32;
    let node = GraphNode {
        id, kind, label: label.to_string(), domain, created_at: current_epoch_ms(), properties: Vec::new(),
    };
    s.graph.nodes.push(node);
    s.graph.offsets.push(*s.graph.offsets.last().unwrap_or(&0));
    Ok(format!("(:ok :id {} :kind {} :label \"{}\" :domain {})", id, kind.to_sexp(), sexp_escape(label), domain.to_sexp()))
}

pub fn add_edge(
    s: &mut crate::PalaceState,
    source: u32,
    target: u32,
    kind: EdgeKind,
    weight: f64,
) -> Result<String, MemoryError> {
    let n = s.graph.nodes.len();
    if source as usize >= n || target as usize >= n {
        return Err(MemoryError::NodeNotFound(format!("id {source} or {target}")));
    }
    let edge = GraphEdge {
        source, target, kind, weight: weight.clamp(0.0, 1.0),
        valid_from: current_epoch_ms(), valid_to: None, confidence: 1.0,
    };
    s.graph.edges.push(edge);
    s.graph.rebuild_csr();
    Ok(format!("(:ok :source {} :target {} :kind {} :weight {:.3})", source, target, kind.to_sexp(), weight))
}

pub fn find_tunnels(s: &mut crate::PalaceState) -> Result<String, MemoryError> {
    // Tunnel: a Room with Contains edges from 2+ different Wings
    let mut room_wings: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for e in &s.graph.edges {
        if e.kind == EdgeKind::Contains && e.is_valid_at(current_epoch_ms()) {
            let (si, ti) = (e.source as usize, e.target as usize);
            if si < s.graph.nodes.len() && ti < s.graph.nodes.len()
                && s.graph.nodes[si].kind == NodeKind::Wing
                && s.graph.nodes[ti].kind == NodeKind::Room
            {
                room_wings.entry(e.target).or_default().push(e.source);
            }
        }
    }
    let tunnels: Vec<String> = room_wings.iter()
        .filter(|(_, wings)| wings.len() >= 2)
        .filter_map(|(room_id, wings)| {
            let idx = *room_id as usize;
            if idx >= s.graph.nodes.len() { return None; }
            let label = &s.graph.nodes[idx].label;
            let wing_labels: Vec<String> = wings.iter()
                .filter_map(|w| {
                    let i = *w as usize;
                    (i < s.graph.nodes.len()).then(|| format!("\"{}\"", sexp_escape(&s.graph.nodes[i].label)))
                })
                .collect();
            Some(format!("(:room \"{}\" :wings ({}))", sexp_escape(label), wing_labels.join(" ")))
        })
        .collect();
    Ok(format!("(:ok :tunnels ({}))", tunnels.join(" ")))
}

pub fn graph_stats(s: &crate::PalaceState) -> Result<String, MemoryError> {
    let now = current_epoch_ms();
    let valid_edges = s.graph.edges.iter().filter(|e| e.is_valid_at(now)).count();
    let counts = s.graph.nodes.iter().fold([0usize; 5], |mut c, n| {
        match n.kind {
            NodeKind::Wing => c[0] += 1, NodeKind::Room => c[1] += 1,
            NodeKind::Entity => c[2] += 1, NodeKind::Concept => c[3] += 1,
            NodeKind::Tunnel => c[4] += 1,
        }
        c
    });
    Ok(format!("(:ok :nodes {} :edges {} :wings {} :rooms {} :entities {} :concepts {} :tunnels {})", s.graph.nodes.len(), valid_edges, counts[0], counts[1], counts[2], counts[3], counts[4]))
}
