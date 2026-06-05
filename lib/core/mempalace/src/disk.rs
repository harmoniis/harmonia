//! Canonical MemPalace persistence.
//!
//! Actor state is durable only under `<HARMONIA_STATE_ROOT>/mempalace`.
//! All persisted data is versioned s-expression data; no Chronicle mirror,
//! config fallback, or shutdown-only path is authoritative.

use std::fs;
use std::path::{Path, PathBuf};

use harmonia_actor_protocol::{
    extract_sexp_f64, extract_sexp_string, extract_sexp_u64, sexp_escape, MemoryError,
};

use crate::codebook::AaakCodebook;
use crate::drawer::{Drawer, DrawerSource, DrawerStore};
use crate::graph::{GraphEdge, GraphNode, KnowledgeGraph};

pub(crate) fn state_dir() -> PathBuf {
    harmonia_config_store::paths::state_root().join("mempalace")
}

pub(crate) fn graph_path(dir: &Path) -> PathBuf {
    dir.join("graph.sexp")
}

pub(crate) fn drawers_dir(dir: &Path) -> PathBuf {
    dir.join("drawers")
}

pub(crate) fn drawer_path(dir: &Path, drawer_id: u64) -> PathBuf {
    drawers_dir(dir).join(format!("{drawer_id:016}.sexp"))
}

pub(crate) fn codebook_path(dir: &Path) -> PathBuf {
    dir.join("codebook.sexp")
}

pub(crate) fn write_graph(dir: &Path, graph: &KnowledgeGraph) -> Result<(), MemoryError> {
    write_atomic(&graph_path(dir), &graph_to_sexp(graph))
}

pub(crate) fn write_drawer(dir: &Path, drawer: &Drawer) -> Result<(), MemoryError> {
    write_atomic(&drawer_path(dir, drawer.id), &drawer_to_sexp(drawer))
}

pub(crate) fn write_codebook(dir: &Path, codebook: &AaakCodebook) -> Result<(), MemoryError> {
    write_atomic(&codebook_path(dir), &codebook.to_sexp())
}

pub(crate) fn load_state(
    dir: &Path,
) -> Result<(KnowledgeGraph, DrawerStore, AaakCodebook, u64), MemoryError> {
    let graph = load_graph(dir)?;
    let (drawers, next_id) = load_drawers(dir)?;
    let codebook = load_codebook(dir)?;
    Ok((graph, drawers, codebook, next_id))
}

fn write_atomic(path: &Path, content: &str) -> Result<(), MemoryError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| MemoryError::PersistenceFailed(e.to_string()))?;
    }
    let tmp = path.with_extension("sexp.tmp");
    fs::write(&tmp, content).map_err(|e| MemoryError::PersistenceFailed(e.to_string()))?;
    fs::rename(&tmp, path).map_err(|e| MemoryError::PersistenceFailed(e.to_string()))?;
    Ok(())
}

fn load_graph(dir: &Path) -> Result<KnowledgeGraph, MemoryError> {
    let path = graph_path(dir);
    if !path.exists() {
        return Ok(KnowledgeGraph::new());
    }
    let content =
        fs::read_to_string(&path).map_err(|e| MemoryError::PersistenceFailed(e.to_string()))?;
    let mut graph = KnowledgeGraph::new();
    for chunk in content.split("(:id ").skip(1) {
        let line = format!("(:id {chunk}");
        let Some(id) = extract_sexp_u64(&line, ":id") else { continue };
        let kind = extract_sexp_string(&line, ":kind").unwrap_or_else(|| "concept".into());
        let label = extract_sexp_string(&line, ":label").unwrap_or_default();
        let domain = extract_sexp_string(&line, ":domain").unwrap_or_else(|| "generic".into());
        let created = extract_sexp_u64(&line, ":created").unwrap_or(0);
        graph.restore_node(
            id as u32,
            kind.strip_prefix(':').unwrap_or(&kind),
            &label,
            domain.strip_prefix(':').unwrap_or(&domain),
            created,
        );
    }
    for chunk in content.split("(:source ").skip(1) {
        let line = format!("(:source {chunk}");
        let Some(source) = extract_sexp_u64(&line, ":source") else { continue };
        let Some(target) = extract_sexp_u64(&line, ":target") else { continue };
        let kind = extract_sexp_string(&line, ":kind").unwrap_or_else(|| "relates-to".into());
        let weight = extract_sexp_f64(&line, ":weight").unwrap_or(1.0);
        let confidence = extract_sexp_f64(&line, ":confidence").unwrap_or(1.0);
        let valid_from = extract_sexp_u64(&line, ":valid-from").unwrap_or(0);
        let valid_to = extract_sexp_u64(&line, ":valid-to");
        graph.restore_edge(
            source as u32,
            target as u32,
            kind.strip_prefix(':').unwrap_or(&kind),
            weight,
            confidence,
            valid_from,
            valid_to,
        );
    }
    graph.rebuild_csr();
    Ok(graph)
}

fn load_drawers(dir: &Path) -> Result<(DrawerStore, u64), MemoryError> {
    let path = drawers_dir(dir);
    let mut store = DrawerStore::new();
    let mut next_id = 1;
    if !path.exists() {
        return Ok((store, next_id));
    }
    let mut files = fs::read_dir(&path)
        .map_err(|e| MemoryError::PersistenceFailed(e.to_string()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "sexp"))
        .collect::<Vec<_>>();
    files.sort();
    for file in files {
        let raw = fs::read_to_string(&file)
            .map_err(|e| MemoryError::PersistenceFailed(e.to_string()))?;
        if let Some(drawer) = drawer_from_sexp(&raw) {
            next_id = next_id.max(drawer.id + 1);
            store.push(drawer);
        }
    }
    Ok((store, next_id))
}

fn load_codebook(dir: &Path) -> Result<AaakCodebook, MemoryError> {
    let path = codebook_path(dir);
    if !path.exists() {
        return Ok(AaakCodebook::new());
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| MemoryError::PersistenceFailed(e.to_string()))?;
    Ok(AaakCodebook::from_sexp(&raw))
}

pub(crate) fn graph_to_sexp(graph: &KnowledgeGraph) -> String {
    let nodes = graph
        .nodes
        .iter()
        .map(node_to_sexp)
        .collect::<Vec<_>>()
        .join("\n");
    let edges = graph
        .edges
        .iter()
        .map(edge_to_sexp)
        .collect::<Vec<_>>()
        .join("\n");
    format!("(:mempalace-graph :version 1\n :nodes (\n{nodes}\n )\n :edges (\n{edges}\n ))\n")
}

fn node_to_sexp(node: &GraphNode) -> String {
    format!(
        "  (:id {} :kind {} :label \"{}\" :domain {} :created {} :properties ())",
        node.id,
        node.kind.to_sexp(),
        sexp_escape(&node.label),
        node.domain.to_sexp(),
        node.created_at,
    )
}

fn edge_to_sexp(edge: &GraphEdge) -> String {
    let valid_to = edge
        .valid_to
        .map(|v| v.to_string())
        .unwrap_or_else(|| "nil".to_string());
    format!(
        "  (:source {} :target {} :kind {} :weight {:.6} :valid-from {} :valid-to {} :confidence {:.6})",
        edge.source,
        edge.target,
        edge.kind.to_sexp(),
        edge.weight,
        edge.valid_from,
        valid_to,
        edge.confidence,
    )
}

fn drawer_to_sexp(drawer: &Drawer) -> String {
    let tags = drawer
        .tags
        .iter()
        .map(|tag| format!("\"{}\"", sexp_escape(tag)))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "(:drawer :version 1 :id {} :room {} :chunk {} :created {} :source \"{}\" :tags ({}) :content \"{}\")\n",
        drawer.id,
        drawer.room_id,
        drawer.chunk_index,
        drawer.created_at,
        sexp_escape(&drawer.source.to_persist_str()),
        tags,
        sexp_escape(&drawer.content),
    )
}

fn drawer_from_sexp(sexp: &str) -> Option<Drawer> {
    let id = extract_sexp_u64(sexp, ":id")?;
    let room_id = extract_sexp_u64(sexp, ":room").unwrap_or(0) as u32;
    let chunk_index = extract_sexp_u64(sexp, ":chunk").unwrap_or(0) as u16;
    let created_at = extract_sexp_u64(sexp, ":created").unwrap_or(0);
    let source = DrawerSource::from_persist_str(
        &extract_sexp_string(sexp, ":source").unwrap_or_else(|| "manual".into()),
    );
    let tags = harmonia_actor_protocol::extract_sexp_string_list(sexp, ":tags");
    let content = extract_sexp_string(sexp, ":content").unwrap_or_default();
    Some(Drawer {
        id,
        content,
        source,
        room_id,
        chunk_index,
        created_at,
        tags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Domain, EdgeKind, NodeKind};

    fn temp_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("harmonia-mempalace-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn graph_round_trip_preserves_temporal_edge_fields() {
        let dir = temp_dir("graph");
        let mut graph = KnowledgeGraph::new();
        graph.restore_node(0, "wing", "life", "life", 17);
        graph.restore_node(1, "room", "daily", "life", 18);
        graph.restore_edge(0, 1, "contains", 0.75, 0.8, 19, Some(20));
        graph.rebuild_csr();

        write_graph(&dir, &graph).unwrap();
        let restored = load_graph(&dir).unwrap();

        assert_eq!(restored.nodes.len(), 2);
        assert_eq!(restored.nodes[0].kind, NodeKind::Wing);
        assert_eq!(restored.nodes[1].domain, Domain::Life);
        assert_eq!(restored.edges.len(), 1);
        assert_eq!(restored.edges[0].kind, EdgeKind::Contains);
        assert_eq!(restored.edges[0].valid_from, 19);
        assert_eq!(restored.edges[0].valid_to, Some(20));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn drawer_round_trip_preserves_content_bytes() {
        let dir = temp_dir("drawer");
        let content = " leading\nLine \"two\" with \\\\ slash\ntrailing ";
        let drawer = Drawer {
            id: 7,
            content: content.to_string(),
            source: DrawerSource::Manual,
            room_id: 3,
            chunk_index: 0,
            created_at: 21,
            tags: vec!["alpha".into(), "beta".into()],
        };

        write_drawer(&dir, &drawer).unwrap();
        let (store, next_id) = load_drawers(&dir).unwrap();

        assert_eq!(next_id, 8);
        assert_eq!(store.len(), 1);
        assert_eq!(store.get(7).unwrap().content, content);
        assert_eq!(store.get(7).unwrap().tags, vec!["alpha", "beta"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_state_reads_canonical_files() {
        let dir = temp_dir("state");
        let mut graph = KnowledgeGraph::new();
        graph.restore_node(0, "room", "notes", "generic", 1);
        graph.rebuild_csr();
        write_graph(&dir, &graph).unwrap();
        write_drawer(
            &dir,
            &Drawer {
                id: 1,
                content: "Durable palace drawer content.".into(),
                source: DrawerSource::Memory { entry_id: "daily-1-1".into() },
                room_id: 0,
                chunk_index: 0,
                created_at: 2,
                tags: vec!["durable".into()],
            },
        )
        .unwrap();
        let mut codebook = AaakCodebook::new();
        codebook.code_for("durable palace");
        write_codebook(&dir, &codebook).unwrap();

        let (g2, d2, c2, next_id) = load_state(&dir).unwrap();

        assert!(graph_path(&dir).exists());
        assert!(drawer_path(&dir, 1).exists());
        assert!(codebook_path(&dir).exists());
        assert_eq!(g2.nodes.len(), 1);
        assert_eq!(d2.get(1).unwrap().content, "Durable palace drawer content.");
        // The chronicle entry-id round-trips through the drawer source.
        match &d2.get(1).unwrap().source {
            DrawerSource::Memory { entry_id } => assert_eq!(entry_id, "daily-1-1"),
            other => panic!("expected Memory source, got {other:?}"),
        }
        assert_eq!(c2.lookup("durable-palace"), Some("A".into()));
        assert_eq!(next_id, 2);
        let _ = fs::remove_dir_all(dir);
    }
}
