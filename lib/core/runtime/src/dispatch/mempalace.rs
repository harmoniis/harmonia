//! MemPalace component dispatch — requires actor-owned PalaceState.

use harmonia_actor_protocol::{extract_sexp_string, extract_sexp_u64_or, extract_sexp_f64, sexp_escape};

use super::dispatch_op;

pub(crate) fn dispatch(
    sexp: &str,
    palace: &mut harmonia_mempalace::PalaceState,
) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => dispatch_op!("init", harmonia_mempalace::init(palace)),
        "health" => dispatch_op!("health", harmonia_mempalace::health_check(palace)),
        "add-node" => {
            let kind_str = extract_sexp_string(sexp, ":kind").unwrap_or_else(|| "concept".into());
            let label = extract_sexp_string(sexp, ":label").unwrap_or_default();
            let domain_str = extract_sexp_string(sexp, ":domain").unwrap_or_else(|| "generic".into());
            let kind = harmonia_mempalace::NodeKind::from_str(&kind_str);
            let domain = harmonia_mempalace::Domain::from_str(&domain_str);
            dispatch_op!("add-node", harmonia_mempalace::add_node(palace, kind, &label, domain))
        }
        "add-edge" => {
            let source = extract_sexp_u64_or(sexp, ":source", 0) as u32;
            let target = extract_sexp_u64_or(sexp, ":target", 0) as u32;
            let kind_str = extract_sexp_string(sexp, ":kind").unwrap_or_else(|| "relates-to".into());
            let weight = extract_sexp_f64(sexp, ":weight").unwrap_or(1.0);
            let kind = harmonia_mempalace::EdgeKind::from_str(&kind_str);
            dispatch_op!("add-edge", harmonia_mempalace::add_edge(palace, source, target, kind, weight))
        }
        "query-graph" => {
            let from = extract_sexp_u64_or(sexp, ":from", 0) as u32;
            let traversal_str = extract_sexp_string(sexp, ":traversal").unwrap_or_else(|| "bfs".into());
            let depth = extract_sexp_u64_or(sexp, ":depth", 3) as u32;
            let traversal = harmonia_mempalace::Traversal::from_str(&traversal_str);
            dispatch_op!("query-graph", harmonia_mempalace::query_graph(palace, from, traversal, depth))
        }
        "find-tunnels" => dispatch_op!("find-tunnels", harmonia_mempalace::find_tunnels(palace)),
        "graph-stats" => dispatch_op!("graph-stats", harmonia_mempalace::graph_stats(palace)),
        "file-drawer" => {
            let content = extract_sexp_string(sexp, ":content").unwrap_or_default();
            let room_id = extract_sexp_u64_or(sexp, ":room", 0) as u32;
            let tags_str = extract_sexp_string(sexp, ":tags").unwrap_or_default();
            let tags: Vec<&str> = tags_str.split_whitespace().collect();
            dispatch_op!("file-drawer", harmonia_mempalace::file_drawer(palace, &content, room_id, harmonia_mempalace::drawer::DrawerSource::Manual, &tags))
        }
        "search" => {
            let query = extract_sexp_string(sexp, ":query").unwrap_or_default();
            let room = extract_sexp_u64_or(sexp, ":room", u64::MAX);
            let room_filter = if room == u64::MAX { None } else { Some(room as u32) };
            let limit = extract_sexp_u64_or(sexp, ":limit", 10) as usize;
            dispatch_op!("search", harmonia_mempalace::search_drawers(palace, &query, room_filter, limit))
        }
        "get-drawer" => {
            let id = extract_sexp_u64_or(sexp, ":id", 0);
            dispatch_op!("get-drawer", harmonia_mempalace::get_drawer(palace, id))
        }
        "compress" => {
            let ids_str = extract_sexp_string(sexp, ":ids").unwrap_or_default();
            let ids: Vec<u64> = ids_str.split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            dispatch_op!("compress", harmonia_mempalace::compress_aaak(palace, &ids))
        }
        "codebook" => {
            let query = extract_sexp_string(sexp, ":query").unwrap_or_default();
            dispatch_op!("codebook", harmonia_mempalace::codebook_lookup(palace, &query))
        }
        "codebook-register" => {
            let concepts_str = extract_sexp_string(sexp, ":concepts").unwrap_or_default();
            let concepts: Vec<String> = concepts_str.split_whitespace().map(|s| s.to_string()).collect();
            dispatch_op!("codebook-register", harmonia_mempalace::codebook_register(palace, &concepts))
        }
        "context-l0" => dispatch_op!("context-l0", harmonia_mempalace::context_l0(palace)),
        "context-l1" => dispatch_op!("context-l1", harmonia_mempalace::context_l1(palace)),
        "context-l2" => {
            let domain = extract_sexp_string(sexp, ":domain").unwrap_or_else(|| "generic".into());
            dispatch_op!("context-l2", harmonia_mempalace::context_l2(palace, &domain))
        }
        "context-l3" => {
            let query = extract_sexp_string(sexp, ":query").unwrap_or_default();
            dispatch_op!("context-l3", harmonia_mempalace::context_l3(palace, &query))
        }
        _ => format!("(:error \"unknown mempalace op: {}\")", sexp_escape(&op)),
    }
}
