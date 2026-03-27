/// Spectral decomposition of the graph Laplacian — the Chladni modes.
///
/// Eigenvectors of the graph Laplacian are standing-wave patterns on the
/// concept graph. Different signal frequencies excite different eigenmodes,
/// giving frequency-selective recall.
///
/// The Fiedler vector (eigenvector for λ₁) gives the optimal graph bisection.
/// Higher eigenvectors provide finer clustering.

use crate::error::clamp;
use crate::graph::{regularized_laplacian_mul, SparseGraph};

/// Compute the first k non-trivial eigenvectors of the graph Laplacian.
///
/// Uses inverse power iteration: find the smallest eigenvalues of L by
/// finding the largest eigenvalues of (σI - L) where σ is an upper bound.
/// Then deflate to get subsequent eigenvectors.
///
/// Returns (eigenvalues[k], eigenvectors[k][n]).
pub(crate) fn spectral_decompose(
    graph: &SparseGraph,
    k: usize,
    max_iter: usize,
    tol: f64,
) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = graph.n;
    if n < 2 {
        return (Vec::new(), Vec::new());
    }

    let k = k.min(n - 1); // Can't have more eigenvectors than n-1 non-trivial ones.
    let mut eigenvalues = Vec::with_capacity(k);
    let mut eigenvectors: Vec<Vec<f64>> = Vec::with_capacity(k);

    // Upper bound on largest eigenvalue: max degree * 2 (Gershgorin bound).
    let sigma = graph.degree.iter().cloned().fold(0.0_f64, f64::max) * 2.0 + 1.0;

    for ki in 0..k {
        // Power iteration on (σI - L) to find eigenvector of L for smallest eigenvalue.
        let mut v = initial_vector(n, ki);

        // Orthogonalize against constant vector (trivial eigenvector).
        remove_constant_component(&mut v, n);
        // Orthogonalize against previously found eigenvectors.
        for prev in &eigenvectors {
            orthogonalize(&mut v, prev);
        }
        normalize_vec(&mut v);

        let mut eigenvalue = 0.0;
        let mut tmp = vec![0.0; n];
        let mut lv = vec![0.0; n]; // Reused across iterations (was per-iteration alloc)

        for _iter in 0..max_iter {
            // w = (σI - L) · v
            regularized_laplacian_mul(graph, &v, &mut tmp, 0.0);
            for i in 0..n {
                tmp[i] = sigma * v[i] - tmp[i]; // (σI - L)·v
            }

            // Orthogonalize against constant vector and previous eigenvectors.
            remove_constant_component(&mut tmp, n);
            for prev in &eigenvectors {
                orthogonalize(&mut tmp, prev);
            }

            let norm = vec_norm(&tmp);
            if norm < 1e-30 {
                break;
            }
            for i in 0..n {
                tmp[i] /= norm;
            }

            // Rayleigh quotient: eigenvalue of L = σ - (σ eigenvalue of (σI-L))
            regularized_laplacian_mul(graph, &tmp, &mut lv, 0.0);
            eigenvalue = dot(&tmp, &lv);

            // Check convergence: ||Lv - λv|| < tol
            let mut residual = 0.0;
            for i in 0..n {
                let diff = lv[i] - eigenvalue * tmp[i];
                residual += diff * diff;
            }
            // Swap instead of clone — zero allocation
            std::mem::swap(&mut v, &mut tmp);

            if residual.sqrt() < tol {
                break;
            }
        }

        eigenvalues.push(clamp(eigenvalue, 0.0, sigma));
        eigenvectors.push(v);
    }

    (eigenvalues, eigenvectors)
}

/// Project a query activation vector onto the eigenmode basis.
/// s_k = ⟨signal, v_k⟩ for each eigenmode k.
pub(crate) fn eigenmode_project(query_activation: &[f64], eigenvectors: &[Vec<f64>]) -> Vec<f64> {
    eigenvectors
        .iter()
        .map(|v| dot(query_activation, v))
        .collect()
}

/// Reconstruct activation pattern from eigenmode projections.
/// a(i) = Σ_k s_k · v_k(i) — the Chladni pattern.
pub(crate) fn eigenmode_activate(
    projections: &[f64],
    eigenvectors: &[Vec<f64>],
    n: usize,
) -> Vec<f64> {
    let mut activation = vec![0.0; n];
    for (s, v) in projections.iter().zip(eigenvectors.iter()) {
        for i in 0..n.min(v.len()) {
            activation[i] += s * v[i];
        }
    }
    activation
}

// ─── Internal helpers ───────────────────────────────────────────────────────

fn initial_vector(n: usize, seed: usize) -> Vec<f64> {
    // Deterministic pseudorandom initialization using golden ratio.
    let phi = 1.618_033_988_749_895_f64;
    (0..n)
        .map(|i| ((i + seed + 1) as f64 * phi).sin())
        .collect()
}

fn remove_constant_component(v: &mut [f64], n: usize) {
    let mean = v.iter().sum::<f64>() / n as f64;
    for x in v.iter_mut() {
        *x -= mean;
    }
}

fn orthogonalize(v: &mut [f64], against: &[f64]) {
    let proj = dot(v, against);
    for (x, a) in v.iter_mut().zip(against.iter()) {
        *x -= proj * a;
    }
}

fn normalize_vec(v: &mut [f64]) {
    let norm = vec_norm(v);
    if norm > 1e-30 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn vec_norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph;

    #[test]
    fn test_spectral_barbell() {
        // Barbell graph: two triangles connected by one edge.
        // Fiedler vector should separate the two triangles.
        let nodes = vec![
            ("a1".into(), "generic".into(), 1, vec![]),
            ("a2".into(), "generic".into(), 1, vec![]),
            ("a3".into(), "generic".into(), 1, vec![]),
            ("b1".into(), "generic".into(), 1, vec![]),
            ("b2".into(), "generic".into(), 1, vec![]),
            ("b3".into(), "generic".into(), 1, vec![]),
        ];
        let edges = vec![
            // Triangle A
            ("a1".into(), "a2".into(), 1.0, false),
            ("a2".into(), "a3".into(), 1.0, false),
            ("a1".into(), "a3".into(), 1.0, false),
            // Triangle B
            ("b1".into(), "b2".into(), 1.0, false),
            ("b2".into(), "b3".into(), 1.0, false),
            ("b1".into(), "b3".into(), 1.0, false),
            // Bridge
            ("a3".into(), "b1".into(), 0.5, true),
        ];
        let g = build_graph(&nodes, &edges);

        let (eigenvalues, eigenvectors) = spectral_decompose(&g, 2, 200, 1e-6);
        assert!(!eigenvalues.is_empty(), "Should have at least one eigenvalue");

        // Fiedler vector (first non-trivial eigenvector) should separate
        // the two clusters: signs should differ between A-nodes and B-nodes.
        let fiedler = &eigenvectors[0];
        let a_sign = fiedler[0].signum();
        let b_sign = fiedler[3].signum();
        assert!(
            a_sign * b_sign < 0.0,
            "Fiedler vector should separate the two triangles"
        );
    }

    #[test]
    fn test_eigenmode_roundtrip() {
        let v1 = vec![1.0, 0.0, -1.0];
        let v2 = vec![0.5, -1.0, 0.5];
        let eigenvectors = vec![v1, v2];

        let signal = vec![1.0, 0.5, 0.0];
        let projections = eigenmode_project(&signal, &eigenvectors);
        let activation = eigenmode_activate(&projections, &eigenvectors, 3);

        // Activation should be non-trivial.
        assert!(activation.iter().any(|a| a.abs() > 0.01));
    }
}
