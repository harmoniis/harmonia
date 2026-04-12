/// Topological invariants for the memory field — the Aharonov-Bohm layer.
///
/// The concept graph has cycles (loops). Each cycle can carry "enclosed flux" —
/// information visible only when traversing the complete loop, invisible to
/// local field measurements. This is the discrete analog of the electromagnetic
/// vector potential's line integral around a solenoid.
///
/// Key quantities:
/// - Cycle basis: the independent loops (first Betti number = |E| - |V| + 1)
/// - Circulation: sum of directed weights around each cycle
/// - Node flux: total absolute circulation through cycles passing through each node

use crate::graph::SparseGraph;

/// A cycle is a list of node indices forming a closed loop.
#[derive(Clone, Debug)]
pub(crate) struct Cycle {
    pub(crate) nodes: Vec<usize>,
}

/// Topological state computed from the graph.
#[derive(Clone, Debug)]
pub(crate) struct TopologyState {
    pub(crate) cycles: Vec<Cycle>,
    pub(crate) circulations: Vec<f64>,
    /// Per-node total absolute flux from all cycles passing through the node.
    pub(crate) node_flux: Vec<f64>,
}

impl Default for TopologyState {
    fn default() -> Self {
        Self {
            cycles: Vec::new(),
            circulations: Vec::new(),
            node_flux: Vec::new(),
        }
    }
}

/// Compute the fundamental cycle basis of the graph via spanning tree complement.
///
/// For a connected graph with V nodes and E edges, there are E - V + 1
/// independent cycles. Each non-tree edge creates exactly one fundamental cycle:
/// the path in the tree between the edge's endpoints, plus the edge itself.
pub(crate) fn compute_cycle_basis(graph: &SparseGraph) -> Vec<Cycle> {
    let n = graph.n;
    if n < 2 {
        return Vec::new();
    }

    // BFS spanning tree
    let mut parent = vec![usize::MAX; n];
    let mut visited = vec![false; n];
    let mut tree_edges = vec![false; graph.values.len()]; // which edges are in the tree
    let mut queue = std::collections::VecDeque::new();

    // Start BFS from node 0
    visited[0] = true;
    parent[0] = 0; // root points to self
    queue.push_back(0);

    while let Some(u) = queue.pop_front() {
        let start = graph.row_ptr[u];
        let end = graph.row_ptr[u + 1];
        for idx in start..end {
            let v = graph.col_idx[idx];
            if !visited[v] {
                visited[v] = true;
                parent[v] = u;
                tree_edges[idx] = true;
                // Mark the reverse edge as tree edge too
                let v_start = graph.row_ptr[v];
                let v_end = graph.row_ptr[v + 1];
                for v_idx in v_start..v_end {
                    if graph.col_idx[v_idx] == u {
                        tree_edges[v_idx] = true;
                        break;
                    }
                }
                queue.push_back(v);
            }
        }
    }

    // For each non-tree edge (u, v) with u < v, find the cycle
    let mut cycles = Vec::new();
    let mut seen_pairs = std::collections::HashSet::new();

    for u in 0..n {
        if !visited[u] { continue; } // Skip disconnected nodes
        let start = graph.row_ptr[u];
        let end = graph.row_ptr[u + 1];
        for idx in start..end {
            let v = graph.col_idx[idx];
            if !visited[v] || tree_edges[idx] || u >= v {
                continue; // Skip disconnected, tree edges, and reverse edges
            }

            let pair = (u.min(v), u.max(v));
            if !seen_pairs.insert(pair) {
                continue; // Already processed
            }

            // Find path from u to v in the tree
            if let Some(path) = tree_path(u, v, &parent) {
                cycles.push(Cycle { nodes: path });
            }
        }
    }

    cycles
}

/// Find path between two nodes in the BFS tree.
fn tree_path(u: usize, v: usize, parent: &[usize]) -> Option<Vec<usize>> {
    // Trace both paths to root, find common ancestor
    let path_u = path_to_root(u, parent);
    let path_v = path_to_root(v, parent);

    // Find lowest common ancestor
    let set_u: std::collections::HashSet<usize> = path_u.iter().copied().collect();
    let mut lca = v;
    for &node in &path_v {
        if set_u.contains(&node) {
            lca = node;
            break;
        }
    }

    // Build cycle: u -> ... -> lca -> ... -> v -> u
    let mut cycle = Vec::new();

    // Path from u to lca
    let mut cur = u;
    while cur != lca {
        cycle.push(cur);
        if parent[cur] == cur { break; } // root
        cur = parent[cur];
    }
    cycle.push(lca);

    // Path from lca to v (reversed)
    let mut v_to_lca = Vec::new();
    cur = v;
    while cur != lca {
        v_to_lca.push(cur);
        if parent[cur] == cur { break; }
        cur = parent[cur];
    }
    v_to_lca.reverse();
    cycle.extend(v_to_lca);

    if cycle.len() >= 3 {
        Some(cycle)
    } else {
        None // degenerate
    }
}

fn path_to_root(mut node: usize, parent: &[usize]) -> Vec<usize> {
    let mut path = Vec::new();
    let max_depth = parent.len(); // prevent infinite loops
    for _ in 0..max_depth {
        if node >= parent.len() { break; } // guard disconnected nodes
        path.push(node);
        if parent[node] == node { break; }
        node = parent[node];
    }
    path
}

/// Compute circulation for each cycle using directed edge weights.
///
/// For symmetric graphs (w_forward == w_reverse), all circulations are zero.
/// When directed weights differ, the circulation measures the directional
/// bias of information flow around the loop — the "enclosed conceptual flux."
///
/// Phi_c = sum_{(i,j) in cycle} log(w_forward(i,j) / w_reverse(j,i))
///
/// If no directed weights are available, computes a structural circulation
/// based on edge weight asymmetry in the graph (heavier edges bias the flow).
pub(crate) fn compute_circulations(
    cycles: &[Cycle],
    graph: &SparseGraph,
    directed_weights: Option<&std::collections::HashMap<(usize, usize), f64>>,
) -> Vec<f64> {
    cycles.iter().map(|cycle| {
        let nodes = &cycle.nodes;
        let mut circulation = 0.0;
        for k in 0..nodes.len() {
            let i = nodes[k];
            let j = nodes[(k + 1) % nodes.len()];

            if let Some(dw) = directed_weights {
                let w_forward = dw.get(&(i, j)).copied().unwrap_or(1.0);
                let w_reverse = dw.get(&(j, i)).copied().unwrap_or(1.0);
                if w_forward > 0.0 && w_reverse > 0.0 {
                    circulation += (w_forward / w_reverse).ln();
                }
            } else {
                // Structural proxy: use edge weight gradient as proxy for direction
                let w_ij = edge_weight(graph, i, j);
                let w_ji = edge_weight(graph, j, i);
                // For undirected graphs w_ij == w_ji, so this gives ~0
                // but small numerical differences still carry information
                if w_ij > 0.0 && w_ji > 0.0 {
                    circulation += (w_ij / w_ji).ln();
                }
            }
        }
        circulation
    }).collect()
}

/// Get edge weight from sparse graph.
fn edge_weight(graph: &SparseGraph, from: usize, to: usize) -> f64 {
    if from >= graph.n { return 0.0; }
    let start = graph.row_ptr[from];
    let end = graph.row_ptr[from + 1];
    for idx in start..end {
        if graph.col_idx[idx] == to {
            return graph.values[idx];
        }
    }
    0.0
}

/// Compute per-node topological flux: sum of |Phi_c| for all cycles through node.
///
/// High flux nodes sit on cycles with strong directional bias — they are
/// topological bridges where the A-B effect is strongest.
pub(crate) fn compute_node_flux(
    cycles: &[Cycle],
    circulations: &[f64],
    n: usize,
) -> Vec<f64> {
    let mut flux = vec![0.0; n];
    for (cycle, &phi) in cycles.iter().zip(circulations.iter()) {
        let abs_phi = phi.abs();
        for &node in &cycle.nodes {
            if node < n {
                flux[node] += abs_phi;
            }
        }
    }
    flux
}

/// Compute topological flux between two basins: sum of |Phi_c| for cycles
/// that touch both the current and proposed basin.
///
/// This is the non-decaying A-B invariant that contributes to hysteresis
/// without exponential decay — structural connectivity persists.
pub(crate) fn flux_between_basins(
    cycles: &[Cycle],
    circulations: &[f64],
    node_basins: &[crate::basin::Basin],
    current: crate::basin::Basin,
    proposed: crate::basin::Basin,
) -> f64 {
    if current == proposed {
        return 0.0;
    }
    let mut flux = 0.0;
    for (cycle, &phi) in cycles.iter().zip(circulations.iter()) {
        let touches_current = cycle.nodes.iter().any(|&i| {
            i < node_basins.len() && node_basins[i] == current
        });
        let touches_proposed = cycle.nodes.iter().any(|&i| {
            i < node_basins.len() && node_basins[i] == proposed
        });
        if touches_current && touches_proposed {
            flux += phi.abs();
        }
    }
    flux
}

/// Full topology computation: cycle basis + circulations + node flux.
pub(crate) fn compute_topology(
    graph: &SparseGraph,
    directed_weights: Option<&std::collections::HashMap<(usize, usize), f64>>,
) -> TopologyState {
    let cycles = compute_cycle_basis(graph);
    let circulations = compute_circulations(&cycles, graph, directed_weights);
    let node_flux = compute_node_flux(&cycles, &circulations, graph.n);
    TopologyState {
        cycles,
        circulations,
        node_flux,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph;

    #[test]
    fn test_tree_has_no_cycles() {
        // Path graph: A -- B -- C (tree, no cycles)
        let nodes = vec![
            ("a".into(), "generic".into(), 1, vec![]),
            ("b".into(), "generic".into(), 1, vec![]),
            ("c".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            ("a".into(), "b".into(), 1.0, false),
            ("b".into(), "c".into(), 1.0, false),
        ];
        let g = build_graph(&nodes, &edges);
        let cycles = compute_cycle_basis(&g);
        assert!(cycles.is_empty(), "A tree should have no cycles");
    }

    #[test]
    fn test_triangle_has_one_cycle() {
        // Triangle: A -- B -- C -- A (one cycle)
        let nodes = vec![
            ("a".into(), "generic".into(), 1, vec![]),
            ("b".into(), "generic".into(), 1, vec![]),
            ("c".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            ("a".into(), "b".into(), 1.0, false),
            ("b".into(), "c".into(), 1.0, false),
            ("a".into(), "c".into(), 1.0, false),
        ];
        let g = build_graph(&nodes, &edges);
        let cycles = compute_cycle_basis(&g);
        assert_eq!(cycles.len(), 1, "Triangle has exactly 1 independent cycle");
        assert!(cycles[0].nodes.len() >= 3, "Cycle should have at least 3 nodes");
    }

    #[test]
    fn test_betti_number() {
        // Two triangles sharing an edge: 4 nodes, 5 edges -> beta_1 = 5 - 4 + 1 = 2
        let nodes = vec![
            ("a".into(), "generic".into(), 1, vec![]),
            ("b".into(), "generic".into(), 1, vec![]),
            ("c".into(), "generic".into(), 1, vec![]),
            ("d".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            ("a".into(), "b".into(), 1.0, false),
            ("b".into(), "c".into(), 1.0, false),
            ("a".into(), "c".into(), 1.0, false),
            ("b".into(), "d".into(), 1.0, false),
            ("c".into(), "d".into(), 1.0, false),
        ];
        let g = build_graph(&nodes, &edges);
        let cycles = compute_cycle_basis(&g);
        assert_eq!(cycles.len(), 2, "Two triangles sharing edge: beta_1 = 2");
    }

    #[test]
    fn test_symmetric_graph_zero_circulation() {
        // Symmetric graph: all circulations should be zero (no directional bias)
        let nodes = vec![
            ("a".into(), "generic".into(), 1, vec![]),
            ("b".into(), "generic".into(), 1, vec![]),
            ("c".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            ("a".into(), "b".into(), 1.0, false),
            ("b".into(), "c".into(), 1.0, false),
            ("a".into(), "c".into(), 1.0, false),
        ];
        let g = build_graph(&nodes, &edges);
        let topo = compute_topology(&g, None);
        for &c in &topo.circulations {
            assert!(c.abs() < 1e-10, "Symmetric graph should have zero circulation");
        }
    }

    #[test]
    fn test_directed_weights_nonzero_circulation() {
        // With directed weights, circulation should be nonzero
        let nodes = vec![
            ("a".into(), "generic".into(), 1, vec![]),
            ("b".into(), "generic".into(), 1, vec![]),
            ("c".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            ("a".into(), "b".into(), 1.0, false),
            ("b".into(), "c".into(), 1.0, false),
            ("a".into(), "c".into(), 1.0, false),
        ];
        let g = build_graph(&nodes, &edges);

        let mut directed = std::collections::HashMap::new();
        // A->B is strong, B->A is weak (asymmetric flow)
        directed.insert((0, 1), 3.0);
        directed.insert((1, 0), 1.0);
        directed.insert((1, 2), 2.0);
        directed.insert((2, 1), 1.0);
        directed.insert((0, 2), 1.0);
        directed.insert((2, 0), 2.0);

        let topo = compute_topology(&g, Some(&directed));
        assert!(!topo.circulations.is_empty());
        // At least one circulation should be nonzero
        assert!(
            topo.circulations.iter().any(|c| c.abs() > 0.01),
            "Directed weights should produce nonzero circulation"
        );
    }

    #[test]
    fn test_node_flux_computed() {
        let nodes = vec![
            ("a".into(), "generic".into(), 1, vec![]),
            ("b".into(), "generic".into(), 1, vec![]),
            ("c".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            ("a".into(), "b".into(), 1.0, false),
            ("b".into(), "c".into(), 1.0, false),
            ("a".into(), "c".into(), 1.0, false),
        ];
        let g = build_graph(&nodes, &edges);

        let mut directed = std::collections::HashMap::new();
        directed.insert((0, 1), 3.0);
        directed.insert((1, 0), 1.0);
        directed.insert((1, 2), 2.0);
        directed.insert((2, 1), 1.0);
        directed.insert((0, 2), 1.0);
        directed.insert((2, 0), 2.0);

        let topo = compute_topology(&g, Some(&directed));
        assert_eq!(topo.node_flux.len(), 3);
        // All nodes are in the single cycle, so all should have the same flux
        assert!(topo.node_flux.iter().all(|f| *f > 0.0), "All cycle nodes should have positive flux");
    }
}
