//! Disk persistence for palace verbatim data.
//!
//! .md files with YAML frontmatter for drawers.
//! .sexp files for graph structure.

use std::fs;
use std::path::{Path, PathBuf};

use crate::drawer::{Drawer, DrawerSource};
use crate::graph::KnowledgeGraph;

/// Resolve the memory root for the current node.
/// Reads `memory-root` from config-store's `node` scope.
pub(crate) fn memory_root() -> Option<PathBuf> {
    harmonia_config_store::get_config("mempalace", "node", "memory-root")
        .ok()
        .flatten()
        .map(PathBuf::from)
}

/// Build the path for a drawer .md file.
///
/// Layout: `<memory_root>/palace/<wing>/<room>/<DDDDDD>.md`
pub(crate) fn drawer_md_path(
    memory_root: &Path,
    wing_label: &str,
    room_label: &str,
    drawer_id: u64,
) -> PathBuf {
    memory_root
        .join("palace")
        .join(sanitize_label(wing_label))
        .join(sanitize_label(room_label))
        .join(format!("{:06}.md", drawer_id))
}

/// Serialize a drawer to .md format with YAML frontmatter.
///
/// The format is a lossless round-trip: write then read returns identical data.
pub(crate) fn drawer_to_md(drawer: &Drawer) -> String {
    let tags_str = drawer
        .tags
        .iter()
        .map(|t| format!("\"{}\"", t))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "---\nid: {}\nsource: {}\nroom: {}\nchunk: {}\ntags: [{}]\ncreated: {}\n---\n\n{}",
        drawer.id,
        drawer.source.to_persist_str(),
        drawer.room_id,
        drawer.chunk_index,
        tags_str,
        drawer.created_at,
        drawer.content,
    )
}

/// Parse a .md file back into drawer fields.
///
/// Returns: `(id, content, source, room_id, chunk_index, created_at, tags)`
pub(crate) fn md_to_drawer(
    content: &str,
) -> Option<(u64, String, DrawerSource, u32, u16, u64, Vec<String>)> {
    // Split on "---" frontmatter delimiters
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return None;
    }
    let frontmatter = parts[1].trim();
    let body = parts[2].trim().to_string();

    let mut id = 0u64;
    let mut source_str = String::new();
    let mut room_id = 0u32;
    let mut chunk_index = 0u16;
    let mut created_at = 0u64;
    let mut tags = Vec::new();

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("id: ") {
            id = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("source: ") {
            source_str = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("room: ") {
            room_id = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("chunk: ") {
            chunk_index = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("created: ") {
            created_at = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("tags: ") {
            // Parse [tag1, tag2] format
            let inner = v.trim().trim_start_matches('[').trim_end_matches(']');
            tags = inner
                .split(',')
                .map(|t| t.trim().trim_matches('"').to_string())
                .filter(|t| !t.is_empty())
                .collect();
        }
    }

    let source = DrawerSource::from_persist_str(&source_str);
    Some((id, body, source, room_id, chunk_index, created_at, tags))
}

/// Write a drawer .md file atomically (temp + rename).
pub(crate) fn write_drawer_md(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load all drawers from palace directory tree.
///
/// Walks: `<memory_root>/palace/<wing>/<room>/*.md`
pub(crate) fn load_all_drawers(
    memory_root: &Path,
) -> Vec<(u64, String, DrawerSource, u32, u16, u64, Vec<String>)> {
    let palace_dir = memory_root.join("palace");
    if !palace_dir.exists() {
        return Vec::new();
    }

    let mut drawers = Vec::new();
    // Walk wing dirs -> room dirs -> .md files
    if let Ok(wings) = fs::read_dir(&palace_dir) {
        for wing_entry in wings.flatten() {
            if !wing_entry.path().is_dir() {
                continue;
            }
            if let Ok(rooms) = fs::read_dir(wing_entry.path()) {
                for room_entry in rooms.flatten() {
                    if !room_entry.path().is_dir() {
                        continue;
                    }
                    if let Ok(files) = fs::read_dir(room_entry.path()) {
                        for file_entry in files.flatten() {
                            let path = file_entry.path();
                            if path.extension().map_or(true, |e| e != "md") {
                                continue;
                            }
                            if let Ok(raw) = fs::read_to_string(&path) {
                                if let Some(drawer) = md_to_drawer(&raw) {
                                    drawers.push(drawer);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    drawers.sort_by_key(|d| d.0); // Sort by id
    drawers
}

/// Build graph index as sexp string (pure -- no I/O).
pub(crate) fn graph_to_sexp(graph: &KnowledgeGraph) -> String {
    let mut sexp = String::from("(:palace-graph\n :nodes (\n");
    for node in &graph.nodes {
        sexp.push_str(&format!(
            "  (:id {} :kind {} :label \"{}\" :domain {} :created {})\n",
            node.id,
            node.kind.to_sexp(),
            crate::sexp_escape(&node.label),
            node.domain.to_sexp(),
            node.created_at,
        ));
    }
    sexp.push_str(" )\n :edges (\n");
    for edge in &graph.edges {
        sexp.push_str(&format!(
            "  (:source {} :target {} :kind {} :weight {:.3} :confidence {:.3})\n",
            edge.source,
            edge.target,
            edge.kind.to_sexp(),
            edge.weight,
            edge.confidence,
        ));
    }
    sexp.push_str(" ))\n");
    sexp
}

/// Write graph index as sexp.
pub(crate) fn write_graph_index(path: &Path, graph: &KnowledgeGraph) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let sexp = graph_to_sexp(graph);
    let tmp = path.with_extension("sexp.tmp");
    fs::write(&tmp, &sexp).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load graph index from sexp file.
///
/// Returns `(nodes, edges)` where:
/// - nodes: `(id, kind_str, label, domain_str, created_at)`
/// - edges: `(source, target, kind_str, weight, confidence)`
pub(crate) fn load_graph_index(
    path: &Path,
) -> Option<(
    Vec<(u32, String, String, String, u64)>,
    Vec<(u32, u32, String, f64, f64)>,
)> {
    let content = fs::read_to_string(path).ok()?;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // We parse line by line. Each node line looks like:
    //   (:id N :kind :K :label "L" :domain :D :created C)
    // Each edge line looks like:
    //   (:source S :target T :kind :K :weight W :confidence C)
    let mut in_nodes = false;
    let mut in_edges = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(":nodes") {
            in_nodes = true;
            in_edges = false;
            continue;
        }
        if trimmed.starts_with(":edges") {
            in_nodes = false;
            in_edges = true;
            continue;
        }
        // End of section
        if trimmed == ")" || trimmed == "))" {
            if in_edges {
                in_edges = false;
            } else if in_nodes {
                in_nodes = false;
            }
            continue;
        }

        if in_nodes && trimmed.starts_with("(:id") {
            if let Some(node) = parse_node_sexp(trimmed) {
                nodes.push(node);
            }
        } else if in_edges && trimmed.starts_with("(:source") {
            if let Some(edge) = parse_edge_sexp(trimmed) {
                edges.push(edge);
            }
        }
    }

    Some((nodes, edges))
}

/// Parse a single node sexp line.
fn parse_node_sexp(line: &str) -> Option<(u32, String, String, String, u64)> {
    use harmonia_actor_protocol::{extract_sexp_string, extract_sexp_u64};
    let id = extract_sexp_u64(line, ":id")? as u32;
    let kind = extract_sexp_string(line, ":kind").unwrap_or_else(|| "concept".into());
    let label = extract_sexp_string(line, ":label").unwrap_or_default();
    let domain = extract_sexp_string(line, ":domain").unwrap_or_else(|| "generic".into());
    let created = extract_sexp_u64(line, ":created").unwrap_or(0);
    // Strip leading colon from kind/domain if present (to_sexp returns ":kind")
    let kind = kind.strip_prefix(':').unwrap_or(&kind).to_string();
    let domain = domain.strip_prefix(':').unwrap_or(&domain).to_string();
    Some((id, kind, label, domain, created))
}

/// Parse a single edge sexp line.
fn parse_edge_sexp(line: &str) -> Option<(u32, u32, String, f64, f64)> {
    use harmonia_actor_protocol::{extract_sexp_f64, extract_sexp_string, extract_sexp_u64};
    let source = extract_sexp_u64(line, ":source")? as u32;
    let target = extract_sexp_u64(line, ":target")? as u32;
    let kind = extract_sexp_string(line, ":kind").unwrap_or_else(|| "relates-to".into());
    let weight = extract_sexp_f64(line, ":weight").unwrap_or(1.0);
    let confidence = extract_sexp_f64(line, ":confidence").unwrap_or(1.0);
    let kind = kind.strip_prefix(':').unwrap_or(&kind).to_string();
    Some((source, target, kind, weight, confidence))
}

fn sanitize_label(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawer::DrawerSource;

    fn make_drawer(id: u64, content: &str, room_id: u32) -> Drawer {
        Drawer {
            id,
            content: content.to_string(),
            source: DrawerSource::Manual,
            room_id,
            chunk_index: 0,
            created_at: 1700000000000,
            tags: vec!["test".to_string(), "memory".to_string()],
        }
    }

    #[test]
    fn drawer_md_roundtrip() {
        let drawer = make_drawer(42, "This is verbatim test content for round-trip.", 3);
        let md = drawer_to_md(&drawer);
        let parsed = md_to_drawer(&md).expect("should parse");
        assert_eq!(parsed.0, 42); // id
        assert_eq!(parsed.1, "This is verbatim test content for round-trip."); // content
        assert_eq!(parsed.3, 3); // room_id
        assert_eq!(parsed.4, 0); // chunk_index
        assert_eq!(parsed.5, 1700000000000); // created_at
        assert_eq!(parsed.6, vec!["test", "memory"]); // tags
        // Source round-trip
        assert_eq!(parsed.2.to_persist_str(), "manual");
    }

    #[test]
    fn drawer_md_roundtrip_conversation_source() {
        let drawer = Drawer {
            id: 7,
            content: "Conversation content with special chars: \"quotes\" and \\backslash".to_string(),
            source: DrawerSource::Conversation {
                session_id: "sess-abc-123".to_string(),
            },
            room_id: 1,
            chunk_index: 2,
            created_at: 1700000001000,
            tags: vec!["chat".to_string()],
        };
        let md = drawer_to_md(&drawer);
        let parsed = md_to_drawer(&md).expect("should parse");
        assert_eq!(parsed.0, 7);
        assert_eq!(
            parsed.1,
            "Conversation content with special chars: \"quotes\" and \\backslash"
        );
        assert_eq!(parsed.2.to_persist_str(), "conversation:sess-abc-123");
        assert_eq!(parsed.3, 1);
        assert_eq!(parsed.4, 2);
        assert_eq!(parsed.5, 1700000001000);
        assert_eq!(parsed.6, vec!["chat"]);
    }

    #[test]
    fn drawer_md_roundtrip_file_source() {
        let drawer = Drawer {
            id: 99,
            content: "File content from disk".to_string(),
            source: DrawerSource::File {
                path: "/home/user/doc.txt".to_string(),
                hash: 12345,
            },
            room_id: 5,
            chunk_index: 0,
            created_at: 1700000002000,
            tags: vec!["file".to_string(), "import".to_string()],
        };
        let md = drawer_to_md(&drawer);
        let parsed = md_to_drawer(&md).expect("should parse");
        assert_eq!(parsed.0, 99);
        assert_eq!(parsed.1, "File content from disk");
        assert_eq!(
            parsed.2.to_persist_str(),
            "file:/home/user/doc.txt:12345"
        );
        assert_eq!(parsed.6, vec!["file", "import"]);
    }

    #[test]
    fn drawer_md_roundtrip_empty_tags() {
        let drawer = Drawer {
            id: 1,
            content: "Minimal content for drawer".to_string(),
            source: DrawerSource::Manual,
            room_id: 0,
            chunk_index: 0,
            created_at: 1700000003000,
            tags: vec![],
        };
        let md = drawer_to_md(&drawer);
        let parsed = md_to_drawer(&md).expect("should parse");
        assert_eq!(parsed.0, 1);
        assert!(parsed.6.is_empty());
    }

    #[test]
    fn drawer_md_roundtrip_multiline_content() {
        let content = "Line one of the content.\n\nParagraph two.\n\nParagraph three with more text.";
        let drawer = make_drawer(50, content, 2);
        let md = drawer_to_md(&drawer);
        let parsed = md_to_drawer(&md).expect("should parse");
        assert_eq!(parsed.1, content);
    }

    #[test]
    fn sanitize_label_works() {
        assert_eq!(sanitize_label("Project X!"), "project-x-");
        assert_eq!(sanitize_label("auth-module"), "auth-module");
        assert_eq!(sanitize_label("My Wing"), "my-wing");
    }

    #[test]
    fn drawer_md_path_construction() {
        let root = PathBuf::from("/memory");
        let path = drawer_md_path(&root, "Project-X", "Auth Module", 42);
        assert_eq!(
            path,
            PathBuf::from("/memory/palace/project-x/auth-module/000042.md")
        );
    }

    #[test]
    fn write_and_load_drawers() {
        let tmp = std::env::temp_dir().join("harmonia-test-palace-drawers");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // Write two drawers
        let d1 = make_drawer(1, "First drawer with enough content for testing.", 0);
        let d2 = make_drawer(2, "Second drawer also has enough content for testing.", 0);

        let p1 = drawer_md_path(&tmp, "wing-a", "room-1", 1);
        let p2 = drawer_md_path(&tmp, "wing-a", "room-1", 2);
        write_drawer_md(&p1, &drawer_to_md(&d1)).unwrap();
        write_drawer_md(&p2, &drawer_to_md(&d2)).unwrap();

        // Load them back
        let loaded = load_all_drawers(&tmp);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].0, 1); // sorted by id
        assert_eq!(loaded[1].0, 2);
        assert_eq!(loaded[0].1, "First drawer with enough content for testing.");
        assert_eq!(loaded[1].1, "Second drawer also has enough content for testing.");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn graph_sexp_roundtrip() {
        use crate::graph::{Domain, EdgeKind, GraphEdge, GraphNode, KnowledgeGraph, NodeKind};

        let mut graph = KnowledgeGraph::new();
        graph.nodes.push(GraphNode {
            id: 0,
            kind: NodeKind::Wing,
            label: "project-x".to_string(),
            domain: Domain::Engineering,
            created_at: 1700000000000,
            properties: vec![],
        });
        graph.offsets.push(0);
        graph.nodes.push(GraphNode {
            id: 1,
            kind: NodeKind::Room,
            label: "auth".to_string(),
            domain: Domain::Engineering,
            created_at: 1700000001000,
            properties: vec![],
        });
        graph.offsets.push(0);
        graph.edges.push(GraphEdge {
            source: 0,
            target: 1,
            kind: EdgeKind::Contains,
            weight: 1.0,
            valid_from: 0,
            valid_to: None,
            confidence: 0.95,
        });

        // Write and read back
        let tmp = std::env::temp_dir().join("harmonia-test-palace-graph");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let graph_path = tmp.join("palace").join("index.sexp");
        write_graph_index(&graph_path, &graph).unwrap();

        let (nodes, edges) = load_graph_index(&graph_path).expect("should parse");
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].0, 0); // id
        assert_eq!(nodes[0].1, "wing"); // kind
        assert_eq!(nodes[0].2, "project-x"); // label
        assert_eq!(nodes[0].3, "engineering"); // domain
        assert_eq!(nodes[0].4, 1700000000000); // created

        assert_eq!(nodes[1].0, 1);
        assert_eq!(nodes[1].1, "room");
        assert_eq!(nodes[1].2, "auth");

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0, 0); // source
        assert_eq!(edges[0].1, 1); // target
        assert_eq!(edges[0].2, "contains"); // kind
        assert!((edges[0].3 - 1.0).abs() < 0.01); // weight
        assert!((edges[0].4 - 0.95).abs() < 0.01); // confidence

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn graph_sexp_with_special_chars_in_label() {
        use crate::graph::{Domain, GraphNode, KnowledgeGraph, NodeKind};

        let mut graph = KnowledgeGraph::new();
        graph.nodes.push(GraphNode {
            id: 0,
            kind: NodeKind::Concept,
            label: "quotes \"and\" backslash \\".to_string(),
            domain: Domain::Generic,
            created_at: 0,
            properties: vec![],
        });
        graph.offsets.push(0);

        let sexp = graph_to_sexp(&graph);
        // Verify it contains escaped quotes
        assert!(sexp.contains(r#"quotes \"and\" backslash \\"#));

        // Write and read back
        let tmp = std::env::temp_dir().join("harmonia-test-palace-graph-esc");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let graph_path = tmp.join("index.sexp");
        write_graph_index(&graph_path, &graph).unwrap();
        let (nodes, _) = load_graph_index(&graph_path).expect("should parse");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].2, "quotes \"and\" backslash \\");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn md_to_drawer_rejects_malformed() {
        assert!(md_to_drawer("no frontmatter here").is_none());
        assert!(md_to_drawer("---only one delimiter").is_none());
    }
}
