use harmonia_actor_protocol::MemoryError;

use crate::{sexp_escape, cfg_usize};

fn preview(content: &str, max_len: usize) -> String {
    if content.len() > max_len {
        format!("{}...", crate::truncate_safe(content, max_len))
    } else {
        content.to_string()
    }
}

pub fn context_l0(s: &crate::PalaceState) -> Result<String, MemoryError> {
    let wings: Vec<String> = s.graph.nodes.iter()
        .filter(|n| n.kind == crate::graph::NodeKind::Wing)
        .map(|n| format!("\"{}\"", sexp_escape(&n.label)))
        .collect();
    Ok(format!("(:ok :tier :l0 :wings ({}))", wings.join(" ")))
}

pub fn context_l1(s: &crate::PalaceState) -> Result<String, MemoryError> {
    let max_entries = cfg_usize("l1-max-entries", 15);
    let mut all_drawers: Vec<&crate::drawer::Drawer> = s.drawers.search("", None, max_entries * 3);
    all_drawers.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    all_drawers.truncate(max_entries);
    let items: Vec<String> = all_drawers.iter()
        .map(|d| format!("(:room {} :preview \"{}\")", d.room_id, sexp_escape(&preview(&d.content, 200))))
        .collect();
    Ok(format!("(:ok :tier :l1 :count {} :entries ({}))", items.len(), items.join(" ")))
}

pub fn context_l2(s: &mut crate::PalaceState, domain_filter: &str) -> Result<String, MemoryError> {
    let domain = crate::graph::Domain::from_str(domain_filter);
    let max_entries = cfg_usize("l2-max-entries", 20);
    let room_ids: Vec<u32> = s.graph.nodes.iter()
        .filter(|n| n.domain == domain && n.kind == crate::graph::NodeKind::Room)
        .map(|n| n.id)
        .collect();
    let results: Vec<String> = room_ids.iter()
        .flat_map(|room_id| {
            s.drawers.by_room(*room_id).into_iter().map(|d| {
                format!("(:room {} :id {} :preview \"{}\")", d.room_id, d.id, sexp_escape(&preview(&d.content, 150)))
            })
        })
        .take(max_entries)
        .collect();
    Ok(format!("(:ok :tier :l2 :domain {} :count {} :entries ({}))", domain.to_sexp(), results.len(), results.join(" ")))
}

pub fn context_l3(s: &mut crate::PalaceState, query: &str) -> Result<String, MemoryError> {
    let max_entries = cfg_usize("l3-max-entries", 30);
    let results = s.drawers.search(query, None, max_entries);
    let items: Vec<String> = results.iter().map(|d| {
        format!(
            "(:id {} :room {} :preview \"{}\" :tags ({}))",
            d.id, d.room_id, sexp_escape(&preview(&d.content, 200)),
            d.tags.iter().map(|t| format!("\"{}\"", sexp_escape(t))).collect::<Vec<_>>().join(" "),
        )
    }).collect();
    Ok(format!("(:ok :tier :l3 :count {} :results ({}))", items.len(), items.join(" ")))
}
