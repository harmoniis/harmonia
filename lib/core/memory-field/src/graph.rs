/// Sparse graph representation and Laplacian operations for the concept graph.
///
/// The concept graph is represented in Compressed Sparse Row (CSR) format
/// for cache-friendly traversal. The graph Laplacian L = D - A is the discrete
/// wave equation on this lattice — its eigenmodes are the Chladni patterns
/// that define frequency-selective recall.

/// Maximum supported graph size.
pub(crate) const MAX_NODES: usize = 256;

/// Domain classification for concept nodes, matching Lisp concept-map.lisp.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Domain {
    Music,
    Math,
    Engineering,
    Cognitive,
    Life,
    Generic,
}

impl Domain {
    pub(crate) fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "music" => Domain::Music,
            "math" => Domain::Math,
            "engineering" => Domain::Engineering,
            "cognitive" => Domain::Cognitive,
            "life" => Domain::Life,
            _ => Domain::Generic,
        }
    }

    pub(crate) fn index(self) -> u8 {
        match self {
            Domain::Music => 0,
            Domain::Math => 1,
            Domain::Engineering => 2,
            Domain::Cognitive => 3,
            Domain::Life => 4,
            Domain::Generic => 5,
        }
    }
}

/// A concept node in the field graph.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct FieldNode {
    pub(crate) index: usize,
    pub(crate) concept: String,
    pub(crate) domain: Domain,
    pub(crate) count: i32,
    pub(crate) entry_ids: Vec<String>,
}

/// Compressed Sparse Row representation of the weighted undirected concept graph.
#[derive(Clone, Debug)]
pub(crate) struct SparseGraph {
    pub(crate) n: usize,
    pub(crate) nodes: Vec<FieldNode>,
    pub(crate) row_ptr: Vec<usize>,
    pub(crate) col_idx: Vec<usize>,
    pub(crate) values: Vec<f64>,
    pub(crate) degree: Vec<f64>,
    pub(crate) concept_to_index: Vec<(String, usize)>,
}

impl SparseGraph {
    pub(crate) fn empty() -> Self {
        Self {
            n: 0,
            nodes: Vec::new(),
            row_ptr: vec![0],
            col_idx: Vec::new(),
            values: Vec::new(),
            degree: Vec::new(),
            concept_to_index: Vec::new(),
        }
    }
}

/// Build a SparseGraph from node and edge lists received from Lisp via IPC.
///
/// Nodes: (concept, domain_str, count, entry_ids)
/// Edges: (concept_a, concept_b, weight, interdisciplinary)
pub(crate) fn build_graph(
    nodes: &[(String, String, i32, Vec<String>)],
    edges: &[(String, String, f64, bool)],
) -> SparseGraph {
    let n = nodes.len().min(MAX_NODES);
    if n == 0 {
        return SparseGraph::empty();
    }

    // Build node list and concept-to-index mapping.
    let mut field_nodes = Vec::with_capacity(n);
    let mut concept_to_index: Vec<(String, usize)> = Vec::with_capacity(n);

    for (i, (concept, domain_str, count, entry_ids)) in nodes.iter().enumerate().take(n) {
        field_nodes.push(FieldNode {
            index: i,
            concept: concept.clone(),
            domain: Domain::from_str(domain_str),
            count: *count,
            entry_ids: entry_ids.clone(),
        });
        concept_to_index.push((concept.clone(), i));
    }
    concept_to_index.sort_by(|a, b| a.0.cmp(&b.0));

    // Build adjacency lists (symmetric: add both directions).
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for (a_concept, b_concept, weight, _interdisciplinary) in edges {
        let a_idx = concept_index_in(&concept_to_index, a_concept);
        let b_idx = concept_index_in(&concept_to_index, b_concept);
        if let (Some(a), Some(b)) = (a_idx, b_idx) {
            if a != b {
                adj[a].push((b, *weight));
                adj[b].push((a, *weight));
            }
        }
    }

    // Sort adjacency lists by column index for CSR and deduplicate.
    for row in &mut adj {
        row.sort_by_key(|(col, _)| *col);
        row.dedup_by_key(|(col, _)| *col);
    }

    // Build CSR arrays.
    let mut row_ptr = Vec::with_capacity(n + 1);
    let mut col_idx = Vec::new();
    let mut values = Vec::new();
    let mut degree = vec![0.0_f64; n];

    row_ptr.push(0);
    for (i, neighbors) in adj.iter().enumerate() {
        for &(j, w) in neighbors {
            col_idx.push(j);
            values.push(w);
            degree[i] += w;
        }
        row_ptr.push(col_idx.len());
    }

    SparseGraph {
        n,
        nodes: field_nodes,
        row_ptr,
        col_idx,
        values,
        degree,
        concept_to_index,
    }
}

/// Look up a concept's node index via binary search.
pub(crate) fn concept_index(graph: &SparseGraph, concept: &str) -> Option<usize> {
    concept_index_in(&graph.concept_to_index, concept)
}

fn concept_index_in(sorted: &[(String, usize)], concept: &str) -> Option<usize> {
    sorted
        .binary_search_by(|(c, _)| c.as_str().cmp(concept))
        .ok()
        .map(|pos| sorted[pos].1)
}

/// Compute L*x where L = D - A (graph Laplacian times vector).
///
/// (Lx)_i = degree[i]*x[i] - Σ_{j ∈ N(i)} w_ij * x[j]
///
/// This is O(|E|) per call — the core operation for conjugate gradient.
pub(crate) fn laplacian_mul(graph: &SparseGraph, x: &[f64], out: &mut [f64]) {
    for i in 0..graph.n {
        let mut sum = graph.degree[i] * x[i];
        let start = graph.row_ptr[i];
        let end = graph.row_ptr[i + 1];
        for idx in start..end {
            sum -= graph.values[idx] * x[graph.col_idx[idx]];
        }
        out[i] = sum;
    }
}

/// Compute (L + εI)*x — regularized Laplacian for CG solver.
pub(crate) fn regularized_laplacian_mul(
    graph: &SparseGraph,
    x: &[f64],
    out: &mut [f64],
    epsilon: f64,
) {
    laplacian_mul(graph, x, out);
    for i in 0..graph.n {
        out[i] += epsilon * x[i];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle_graph() -> SparseGraph {
        let nodes = vec![
            ("a".into(), "generic".into(), 3, vec![]),
            ("b".into(), "generic".into(), 2, vec![]),
            ("c".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            ("a".into(), "b".into(), 1.0, false),
            ("b".into(), "c".into(), 1.0, false),
            ("a".into(), "c".into(), 1.0, false),
        ];
        build_graph(&nodes, &edges)
    }

    #[test]
    fn test_graph_construction() {
        let g = triangle_graph();
        assert_eq!(g.n, 3);
        assert_eq!(g.degree, vec![2.0, 2.0, 2.0]);
    }

    #[test]
    fn test_laplacian_constant_nullspace() {
        // L * 1 = 0 for any graph Laplacian.
        let g = triangle_graph();
        let ones = vec![1.0; g.n];
        let mut out = vec![0.0; g.n];
        laplacian_mul(&g, &ones, &mut out);
        for val in &out {
            assert!(val.abs() < 1e-12, "L*1 should be zero, got {val}");
        }
    }

    #[test]
    fn test_concept_lookup() {
        let g = triangle_graph();
        assert_eq!(concept_index(&g, "a"), Some(0));
        assert_eq!(concept_index(&g, "b"), Some(1));
        assert_eq!(concept_index(&g, "c"), Some(2));
        assert_eq!(concept_index(&g, "d"), None);
    }
}
