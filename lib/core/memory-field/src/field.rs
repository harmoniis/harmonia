/// Conjugate gradient solver for the graph Laplacian field equation.
///
/// Solves (L + εI) · φ = b where L is the graph Laplacian, ε is a small
/// regularization term (the Laplacian is singular), and b is the source
/// potential vector set by query concepts.
///
/// This is the lightning pathfinding principle: source concepts are electrodes,
/// the field concentrates current along optimal paths to memory nodes.

use crate::error::clamp;
use crate::graph::{concept_index, regularized_laplacian_mul, SparseGraph};

/// Solve the regularized Laplacian system (L + εI) · φ = b via conjugate gradient.
///
/// For a ~120 node sparse graph, this converges in 10-30 iterations.
/// Total cost per solve: O(max_iter × |E|).
pub(crate) fn solve_field(
    graph: &SparseGraph,
    sources: &[f64],
    max_iter: usize,
    tol: f64,
    epsilon: f64,
) -> Vec<f64> {
    let n = graph.n;
    if n == 0 {
        return Vec::new();
    }

    // x_0 = 0
    let mut x = vec![0.0; n];
    // r = b - A*x_0 = b (since x_0 = 0)
    let mut r = sources.to_vec();
    let mut p = r.clone();
    let mut rs_old = dot(&r, &r);

    if rs_old.sqrt() < tol {
        return x;
    }

    let mut ap = vec![0.0; n];

    for _iter in 0..max_iter {
        // Ap = (L + εI) * p
        regularized_laplacian_mul(graph, &p, &mut ap, epsilon);

        let p_ap = dot(&p, &ap);
        if p_ap.abs() < 1e-30 {
            break;
        }

        let alpha = rs_old / p_ap;

        // x += α·p, r -= α·Ap
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }

        let rs_new = dot(&r, &r);
        if rs_new.sqrt() < tol {
            break;
        }

        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }

    x
}

/// Build source potential vector from query concepts.
///
/// Maps query words to graph node indices. Source potential is proportional
/// to the node's reference count (how central it is in the graph).
pub(crate) fn build_source_vector(graph: &SparseGraph, query_concepts: &[String]) -> Vec<f64> {
    let mut b = vec![0.0; graph.n];
    for concept in query_concepts {
        if let Some(idx) = concept_index(graph, concept) {
            let count = graph.nodes[idx].count as f64;
            b[idx] = clamp(count, 1.0, 100.0);
        }
    }
    b
}

/// Compute edge current magnitudes from solved field potentials.
///
/// Current I_ij = w_ij · |φ_i - φ_j| through each edge.
/// Returns (node_a, node_b, current_magnitude) for all edges.
#[allow(dead_code)]
pub(crate) fn edge_currents(graph: &SparseGraph, phi: &[f64]) -> Vec<(usize, usize, f64)> {
    let mut currents = Vec::new();
    for i in 0..graph.n {
        let start = graph.row_ptr[i];
        let end = graph.row_ptr[i + 1];
        for idx in start..end {
            let j = graph.col_idx[idx];
            if i < j {
                // Only emit each edge once (undirected).
                let current = graph.values[idx] * (phi[i] - phi[j]).abs();
                currents.push((i, j, current));
            }
        }
    }
    currents
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph;

    #[test]
    fn test_solve_field_simple() {
        // Path graph: A -- B -- C
        // Source at A, expect potential decreasing A > B > C.
        let nodes = vec![
            ("a".into(), "generic".into(), 5, vec![]),
            ("b".into(), "generic".into(), 3, vec![]),
            ("c".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            ("a".into(), "b".into(), 1.0, false),
            ("b".into(), "c".into(), 1.0, false),
        ];
        let g = build_graph(&nodes, &edges);

        let sources = build_source_vector(&g, &["a".into()]);
        let phi = solve_field(&g, &sources, 100, 1e-8, 0.01);

        assert!(phi[0] > phi[1], "A should have higher potential than B");
        assert!(phi[1] > phi[2], "B should have higher potential than C");
    }

    #[test]
    fn test_empty_graph() {
        let g = build_graph(&[], &[]);
        let phi = solve_field(&g, &[], 50, 1e-8, 0.01);
        assert!(phi.is_empty());
    }
}
