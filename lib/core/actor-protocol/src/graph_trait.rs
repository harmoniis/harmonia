//! Trait abstraction for concept graphs.
//!
//! Both SparseGraph (memory-field) and KnowledgeGraph (mempalace) implement
//! this trait, enabling shared algorithms (traversal, centrality, spectral).

/// Common interface for concept graphs in CSR format.
pub trait ConceptGraph {
    /// Number of nodes in the graph.
    fn node_count(&self) -> usize;

    /// Look up a node index by concept label. Binary search or linear scan.
    fn concept_index(&self, concept: &str) -> Option<usize>;

    /// Neighbor indices for a given node (from CSR structure).
    fn neighbor_indices(&self, node: usize) -> &[usize];

    /// Weighted degree of a node.
    fn degree(&self, node: usize) -> f64;

    /// Edge weight between two adjacent nodes. Returns 0.0 if not connected.
    fn edge_weight(&self, from: usize, to: usize) -> f64;

    /// Perform Laplacian matrix-vector multiplication: out = L * x
    /// where L = D - A (degree matrix minus adjacency matrix).
    fn laplacian_mul(&self, x: &[f64], out: &mut [f64]) {
        let n = self.node_count();
        for i in 0..n {
            let d = self.degree(i);
            out[i] = d * x[i];
            for &j in self.neighbor_indices(i) {
                let w = self.edge_weight(i, j);
                out[i] -= w * x[j];
            }
        }
    }
}
