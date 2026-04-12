use harmonia_actor_protocol::MemoryError;

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

    /// Serialize to a compact string for Chronicle persistence.
    /// Format: "conversation:<session_id>", "file:<path>:<hash>",
    ///         "datamining:<lode_id>:<node_label>", "manual"
    pub fn to_persist_str(&self) -> String {
        match self {
            Self::Conversation { session_id } => format!("conversation:{}", session_id),
            Self::File { path, hash } => format!("file:{}:{}", path, hash),
            Self::Datamining { lode_id, node_label } => format!("datamining:{}:{}", lode_id, node_label),
            Self::Manual => "manual".into(),
        }
    }

    /// Deserialize from the compact persist string.
    pub fn from_persist_str(s: &str) -> Self {
        if let Some(rest) = s.strip_prefix("conversation:") {
            Self::Conversation { session_id: rest.to_string() }
        } else if let Some(rest) = s.strip_prefix("file:") {
            // Split on last ':' to separate path from hash
            if let Some(colon_pos) = rest.rfind(':') {
                let path = &rest[..colon_pos];
                let hash = rest[colon_pos + 1..].parse::<u64>().unwrap_or(0);
                Self::File { path: path.to_string(), hash }
            } else {
                Self::File { path: rest.to_string(), hash: 0 }
            }
        } else if let Some(rest) = s.strip_prefix("datamining:") {
            if let Some(colon_pos) = rest.find(':') {
                let lode_id = &rest[..colon_pos];
                let node_label = &rest[colon_pos + 1..];
                Self::Datamining { lode_id: lode_id.to_string(), node_label: node_label.to_string() }
            } else {
                Self::Datamining { lode_id: rest.to_string(), node_label: String::new() }
            }
        } else {
            Self::Manual
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

    /// Restore a drawer from persisted Chronicle data (warm-start).
    pub fn restore(
        &mut self,
        id: u64,
        content: String,
        source: DrawerSource,
        room_id: u32,
        chunk_index: u16,
        created_at: u64,
        tags: Vec<String>,
    ) {
        self.drawers.push(Drawer { id, content, source, room_id, chunk_index, created_at, tags });
    }

    /// Get all drawers (for persistence).
    pub fn all(&self) -> &[Drawer] {
        &self.drawers
    }

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
) -> Result<String, MemoryError> {
    if content.len() < 10 {
        return Err(MemoryError::InvalidContent("min 10 chars".into()));
    }
    if s.graph.find_node_by_id(room_id).is_none() {
        return Err(MemoryError::RoomNotFound(room_id));
    }
    let id = s.next_drawer_id;
    s.next_drawer_id += 1;
    let drawer = Drawer {
        id, content: content.to_string(), source, room_id, chunk_index: 0,
        created_at: current_epoch_ms(), tags: tags.iter().map(|t| t.to_string()).collect(),
    };
    s.drawers.push(drawer.clone());

    // Write to disk IMMEDIATELY -- verbatim must never be lost
    if let Some(root) = crate::disk::memory_root() {
        let (wing, room) = s.resolve_wing_room(room_id);
        let path = crate::disk::drawer_md_path(&root, &wing, &room, id);
        let md = crate::disk::drawer_to_md(&drawer);
        let _ = crate::disk::write_drawer_md(&path, &md);
    }

    Ok(format!("(:ok :id {} :room {} :size {})", id, room_id, content.len()))
}

pub fn search_drawers(
    s: &mut crate::PalaceState,
    query: &str,
    room_filter: Option<u32>,
    limit: usize,
) -> Result<String, MemoryError> {
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

pub fn get_drawer(s: &crate::PalaceState, id: u64) -> Result<String, MemoryError> {
    match s.drawers.get(id) {
        Some(d) => Ok(format!(
            "(:ok :id {} :room {} :content \"{}\" :source {} :tags ({}) :created {})",
            d.id, d.room_id, sexp_escape(&d.content), d.source.to_sexp(),
            d.tags.iter().map(|t| format!("\"{}\"", sexp_escape(t))).collect::<Vec<_>>().join(" "),
            d.created_at,
        )),
        None => Err(MemoryError::DrawerNotFound(id)),
    }
}
