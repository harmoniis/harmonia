use crate::{sexp_escape, current_epoch_ms};

#[derive(Clone, Debug)]
pub enum DrawerSource {
    Conversation { session_id: String },
    File { path: String, hash: u64 },
    Datamining { lode_id: String, node_label: String },
    Manual,
}

impl DrawerSource {
    pub fn to_sexp(&self) -> String {
        match self {
            Self::Conversation { session_id } => format!("(:conversation :session \"{}\")", sexp_escape(session_id)),
            Self::File { path, hash } => format!("(:file :path \"{}\" :hash {})", sexp_escape(path), hash),
            Self::Datamining { lode_id, node_label } => format!("(:datamining :lode \"{}\" :node \"{}\")", sexp_escape(lode_id), sexp_escape(node_label)),
            Self::Manual => "(:manual)".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Drawer {
    pub id: u64,
    pub content: String,
    pub source: DrawerSource,
    pub room_id: u32,
    pub chunk_index: u16,
    pub created_at: u64,
    pub tags: Vec<String>,
}

pub struct DrawerStore {
    drawers: Vec<Drawer>,
}

impl DrawerStore {
    pub fn new() -> Self { Self { drawers: Vec::new() } }
    pub fn len(&self) -> usize { self.drawers.len() }
    pub fn push(&mut self, drawer: Drawer) { self.drawers.push(drawer); }

    pub fn get(&self, id: u64) -> Option<&Drawer> {
        self.drawers.iter().find(|d| d.id == id)
    }

    pub fn get_by_ids(&self, ids: &[u64]) -> Vec<&Drawer> {
        self.drawers.iter().filter(|d| ids.contains(&d.id)).collect()
    }

    pub fn search(&self, query: &str, room_filter: Option<u32>, limit: usize) -> Vec<&Drawer> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<&Drawer> = self.drawers.iter()
            .filter(|d| {
                let room_ok = room_filter.map_or(true, |r| d.room_id == r);
                let content_ok = query_lower.is_empty()
                    || d.content.to_lowercase().contains(&query_lower)
                    || d.tags.iter().any(|t| t.to_lowercase().contains(&query_lower));
                room_ok && content_ok
            })
            .collect();
        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        results.truncate(limit);
        results
    }

    pub fn by_room(&self, room_id: u32) -> Vec<&Drawer> {
        self.drawers.iter().filter(|d| d.room_id == room_id).collect()
    }
}

// ── Public API ──

pub fn file_drawer(
    s: &mut crate::PalaceState,
    content: &str,
    room_id: u32,
    source: DrawerSource,
    tags: &[&str],
) -> Result<String, String> {
    if content.len() < 10 {
        return Err("(:error \"content too short: min 10 chars\")".into());
    }
    if s.graph.find_node_by_id(room_id).is_none() {
        return Err(format!("(:error \"room {} not found\")", room_id));
    }
    let id = s.next_drawer_id;
    s.next_drawer_id += 1;
    let drawer = Drawer {
        id, content: content.to_string(), source, room_id, chunk_index: 0,
        created_at: current_epoch_ms(), tags: tags.iter().map(|t| t.to_string()).collect(),
    };
    s.drawers.push(drawer);
    Ok(format!("(:ok :id {} :room {} :size {})", id, room_id, content.len()))
}

pub fn search_drawers(
    s: &mut crate::PalaceState,
    query: &str,
    room_filter: Option<u32>,
    limit: usize,
) -> Result<String, String> {
    let limit = limit.min(50).max(1);
    let results = s.drawers.search(query, room_filter, limit);
    let items: Vec<String> = results.iter()
        .map(|d| {
            let preview = if d.content.len() > 120 {
                format!("{}...", crate::truncate_safe(&d.content, 120))
            } else {
                d.content.clone()
            };
            format!(
                "(:id {} :room {} :preview \"{}\" :tags ({}))",
                d.id, d.room_id, sexp_escape(&preview),
                d.tags.iter().map(|t| format!("\"{}\"", sexp_escape(t))).collect::<Vec<_>>().join(" "),
            )
        })
        .collect();
    Ok(format!("(:ok :count {} :results ({}))", results.len(), items.join(" ")))
}

pub fn get_drawer(s: &crate::PalaceState, id: u64) -> Result<String, String> {
    match s.drawers.get(id) {
        Some(d) => Ok(format!(
            "(:ok :id {} :room {} :content \"{}\" :source {} :tags ({}) :created {})",
            d.id, d.room_id, sexp_escape(&d.content), d.source.to_sexp(),
            d.tags.iter().map(|t| format!("\"{}\"", sexp_escape(t))).collect::<Vec<_>>().join(" "),
            d.created_at,
        )),
        None => Err(format!("(:error \"drawer {} not found\")", id)),
    }
}
