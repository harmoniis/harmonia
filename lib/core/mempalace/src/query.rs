use harmonia_actor_protocol::MemoryError;

use crate::sexp_escape;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub enum Traversal {
    Bfs,
    Outgoing,
    Incoming,
}

impl Traversal {
    pub fn from_str(s: &str) -> Self {
        match s {
            "outgoing" | ":outgoing" => Self::Outgoing,
            "incoming" | ":incoming" => Self::Incoming,
            _ => Self::Bfs,
        }
    }
    pub fn to_sexp(&self) -> &'static str {
        match self { Self::Bfs => ":bfs", Self::Outgoing => ":outgoing", Self::Incoming => ":incoming" }
    }
}

fn collect_neighbors(graph: &crate::graph::KnowledgeGraph, node: usize, now: u64, traversal: Traversal) -> Vec<usize> {
    match traversal {
        Traversal::Bfs => graph.neighbors(node).to_vec(),
        Traversal::Outgoing => graph.edges.iter()
            .filter(|e| e.source as usize == node && e.is_valid_at(now) && (e.target as usize) < graph.nodes.len())
            .map(|e| e.target as usize).collect(),
        Traversal::Incoming => graph.edges.iter()
            .filter(|e| e.target as usize == node && e.is_valid_at(now) && (e.source as usize) < graph.nodes.len())
            .map(|e| e.source as usize).collect(),
    }
}

fn traverse(
    graph: &crate::graph::KnowledgeGraph,
    start: usize,
    max_depth: u32,
    traversal: Traversal,
) -> Vec<(usize, u32)> {
    let n = graph.nodes.len();
    let now = crate::current_epoch_ms();
    let mut visited = vec![false; n];
    let mut results = Vec::new();
    let mut queue = VecDeque::new();
    visited[start] = true;
    queue.push_back((start, 0u32));
    while let Some((node_idx, depth)) = queue.pop_front() {
        if depth > max_depth { continue; }
        results.push((node_idx, depth));
        for neighbor in collect_neighbors(graph, node_idx, now, traversal) {
            if neighbor < n && !visited[neighbor] {
                visited[neighbor] = true;
                queue.push_back((neighbor, depth + 1));
            }
        }
    }
    results
}

pub fn query_graph(
    s: &mut crate::PalaceState,
    from: u32,
    traversal: Traversal,
    max_depth: u32,
) -> Result<String, MemoryError> {
    let start_idx = s.graph.find_node_by_id(from)
        .ok_or_else(|| MemoryError::NodeNotFound(format!("id {from}")))?;
    let results = traverse(&s.graph, start_idx, max_depth, traversal);
    let items: Vec<String> = results.iter()
        .map(|&(idx, depth)| {
            let node = &s.graph.nodes[idx];
            format!("(:id {} :label \"{}\" :kind {} :domain {} :depth {})", node.id, sexp_escape(&node.label), node.kind.to_sexp(), node.domain.to_sexp(), depth)
        })
        .collect();
    Ok(format!("(:ok :from {} :traversal {} :depth {} :nodes ({}))", from, traversal.to_sexp(), max_depth, items.join(" ")))
}
