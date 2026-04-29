//! MemPalace component dispatch — requires actor-owned PalaceState.

use harmonia_actor_protocol::extract_sexp_string;

use super::{dispatch_op, esc, param, param_u64, param_f64};

pub(crate) fn dispatch(
    sexp: &str,
    palace: &mut harmonia_mempalace::PalaceState,
) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => dispatch_op!("init", harmonia_mempalace::init(palace)),
        "health" => dispatch_op!("health", harmonia_mempalace::health_check(palace)),
        "add-node" => {
            let kind_str = param!(sexp, ":kind", "concept");
            let label = param!(sexp, ":label");
            let domain_str = param!(sexp, ":domain", "generic");
            let kind = harmonia_mempalace::NodeKind::from_str(&kind_str);
            let domain = harmonia_mempalace::Domain::from_str(&domain_str);
            dispatch_op!("add-node", harmonia_mempalace::add_node(palace, kind, &label, domain))
        }
        "add-edge" => {
            let source = param_u64!(sexp,":source", 0) as u32;
            let target = param_u64!(sexp,":target", 0) as u32;
            let kind_str = param!(sexp, ":kind", "relates-to");
            let weight = param_f64!(sexp, ":weight", 1.0);
            let kind = harmonia_mempalace::EdgeKind::from_str(&kind_str);
            dispatch_op!("add-edge", harmonia_mempalace::add_edge(palace, source, target, kind, weight))
        }
        "query-graph" => {
            let from = param_u64!(sexp,":from", 0) as u32;
            let traversal_str = param!(sexp, ":traversal", "bfs");
            let depth = param_u64!(sexp,":depth", 3) as u32;
            let traversal = harmonia_mempalace::Traversal::from_str(&traversal_str);
            dispatch_op!("query-graph", harmonia_mempalace::query_graph(palace, from, traversal, depth))
        }
        "find-tunnels" => dispatch_op!("find-tunnels", harmonia_mempalace::find_tunnels(palace)),
        "graph-stats" => dispatch_op!("graph-stats", harmonia_mempalace::graph_stats(palace)),
        "file-drawer" => {
            let content = param!(sexp, ":content");
            let room_id = param_u64!(sexp,":room", 0) as u32;
            let tags_str = param!(sexp, ":tags");
            let tags: Vec<&str> = tags_str.split_whitespace().collect();
            dispatch_op!("file-drawer", harmonia_mempalace::file_drawer(palace, &content, room_id, harmonia_mempalace::drawer::DrawerSource::Manual, &tags))
        }
        "search" => {
            let query = param!(sexp, ":query");
            let room = param_u64!(sexp,":room", u64::MAX);
            let room_filter = if room == u64::MAX { None } else { Some(room as u32) };
            let limit = param_u64!(sexp,":limit", 10) as usize;
            dispatch_op!("search", harmonia_mempalace::search_drawers(palace, &query, room_filter, limit))
        }
        "get-drawer" => {
            let id = param_u64!(sexp,":id", 0);
            dispatch_op!("get-drawer", harmonia_mempalace::get_drawer(palace, id))
        }
        "compress" => {
            let ids_str = param!(sexp, ":ids");
            let ids: Vec<u64> = ids_str.split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            dispatch_op!("compress", harmonia_mempalace::compress_aaak(palace, &ids))
        }
        "codebook" => {
            let query = param!(sexp, ":query");
            dispatch_op!("codebook", harmonia_mempalace::codebook_lookup(palace, &query))
        }
        "codebook-register" => {
            let concepts_str = param!(sexp, ":concepts");
            let concepts: Vec<String> = concepts_str.split_whitespace().map(|s| s.to_string()).collect();
            dispatch_op!("codebook-register", harmonia_mempalace::codebook_register(palace, &concepts))
        }
        "context-l0" => dispatch_op!("context-l0", harmonia_mempalace::context_l0(palace)),
        "context-l1" => dispatch_op!("context-l1", harmonia_mempalace::context_l1(palace)),
        "context-l2" => {
            let domain = param!(sexp, ":domain", "generic");
            dispatch_op!("context-l2", harmonia_mempalace::context_l2(palace, &domain))
        }
        "context-l3" => {
            let query = param!(sexp, ":query");
            dispatch_op!("context-l3", harmonia_mempalace::context_l3(palace, &query))
        }
        _ => format!("(:error \"unknown mempalace op: {}\")", esc(&op)),
    }
}
