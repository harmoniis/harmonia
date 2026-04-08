//! Typed error hierarchy for the memory subsystem.
//!
//! Replaces `Result<String, String>` across memory-field, mempalace, and chronicle.
//! Display produces plain text (not sexp) — the dispatch_op! macro wraps it.

use std::fmt;

/// Structured error for all memory operations.
#[derive(Debug, Clone)]
pub enum MemoryError {
    /// Operation on an empty graph (no nodes loaded).
    GraphEmpty,
    /// Referenced node does not exist.
    NodeNotFound(String),
    /// Referenced edge does not exist.
    EdgeNotFound { source: String, target: String },
    /// Capacity limit exceeded (max-graph-nodes, max-drawers, etc.).
    CapacityExceeded { kind: &'static str, limit: usize },
    /// Content validation failed (too short, invalid format).
    InvalidContent(String),
    /// Conjugate gradient solver failed to converge.
    SolverDiverged,
    /// Required configuration key missing or unparseable.
    ConfigMissing(String),
    /// Persistence operation failed (disk, chronicle, config-store).
    PersistenceFailed(String),
    /// Compression or AAAK encoding failed.
    CompressionFailed(String),
    /// Graph traversal hit an invalid state.
    TraversalFailed(String),
    /// Duplicate node label.
    DuplicateNode(String),
    /// Room not found (mempalace drawer operations).
    RoomNotFound(u32),
    /// Drawer not found.
    DrawerNotFound(u64),
    /// Generic operation error with context.
    OperationFailed(String),
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GraphEmpty => write!(f, "graph is empty"),
            Self::NodeNotFound(label) => write!(f, "node not found: {label}"),
            Self::EdgeNotFound { source, target } => {
                write!(f, "edge not found: {source} -> {target}")
            }
            Self::CapacityExceeded { kind, limit } => {
                write!(f, "{kind} capacity exceeded: max {limit}")
            }
            Self::InvalidContent(detail) => write!(f, "invalid content: {detail}"),
            Self::SolverDiverged => write!(f, "solver did not converge"),
            Self::ConfigMissing(key) => write!(f, "config missing: {key}"),
            Self::PersistenceFailed(detail) => write!(f, "persistence failed: {detail}"),
            Self::CompressionFailed(detail) => write!(f, "compression failed: {detail}"),
            Self::TraversalFailed(detail) => write!(f, "traversal failed: {detail}"),
            Self::DuplicateNode(label) => write!(f, "node already exists: {label}"),
            Self::RoomNotFound(id) => write!(f, "room {id} not found"),
            Self::DrawerNotFound(id) => write!(f, "drawer {id} not found"),
            Self::OperationFailed(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for MemoryError {}

/// Conversion from String for easy migration from `Result<T, String>`.
impl From<String> for MemoryError {
    fn from(s: String) -> Self {
        Self::OperationFailed(s)
    }
}

impl From<&str> for MemoryError {
    fn from(s: &str) -> Self {
        Self::OperationFailed(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_produces_plain_text() {
        let e = MemoryError::GraphEmpty;
        assert_eq!(e.to_string(), "graph is empty");
    }

    #[test]
    fn display_node_not_found() {
        let e = MemoryError::NodeNotFound("vitruvian".into());
        assert_eq!(e.to_string(), "node not found: vitruvian");
    }

    #[test]
    fn display_capacity_exceeded() {
        let e = MemoryError::CapacityExceeded { kind: "nodes", limit: 256 };
        assert_eq!(e.to_string(), "nodes capacity exceeded: max 256");
    }

    #[test]
    fn from_string_conversion() {
        let e: MemoryError = "something broke".into();
        assert_eq!(e.to_string(), "something broke");
    }

    #[test]
    fn display_room_not_found() {
        let e = MemoryError::RoomNotFound(42);
        assert_eq!(e.to_string(), "room 42 not found");
    }
}
