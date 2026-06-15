//! harmonia-finder — the default local retrieval substrate.
//!
//! A long-lived actor wrapping the `fff-search` engine: persistent `FilePicker`
//! indexes (background scan + filesystem watcher + mmap content cache + LMDB
//! frecency) so repeated fuzzy file-path and content searches are fast and
//! frecency-ranked. TWO indexes, one substrate:
//!   - `proj` over the workspace root (project / source files)
//!   - `mem`  over <state-root>/mempalace (the on-disk MEMORY files: drawers + graph)
//! so a single `find`/`grep` mechanism covers project files AND memory files —
//! the user's "default mechanism to search through memory files or project files".
//!
//! NO FFI: pure Rust crate dependency, reached from Lisp only through the s-expr
//! IPC actor boundary, exactly like every other component.
//!
//! Results are always a BOUNDED top-N set — never an unbounded list — the
//! retrieval half of "always choose from a finite matrix of possibilities".
//! Harmonic ranking of that bounded set is layered on the Lisp side.

use std::path::PathBuf;
use std::time::Duration;

use fff_search::file_picker::FilePicker;
use fff_search::frecency::FrecencyTracker;
use fff_search::{
    parse_grep_query, FFFMode, FilePickerOptions, FuzzySearchOptions, GrepSearchOptions,
    PaginationArgs, QueryParser, SharedFilePicker, SharedFrecency,
};

use harmonia_actor_protocol::extract_sexp_string;

/// A fuzzy file-path hit (path relative to the index base).
pub struct FileHit {
    pub path: String,
}

/// A content-search hit.
pub struct GrepHit {
    pub path: String,
    pub line: u64,
    pub text: String,
    pub is_def: bool,
}

/// One fff-search index over a single base directory.
struct Index {
    picker: SharedFilePicker,
    #[allow(dead_code)]
    frecency: SharedFrecency,
    base: PathBuf,
    ready: bool,
}

impl Index {
    fn open(label: &str, base: PathBuf, db_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&db_dir);
        // Ensure the base exists so the index + watcher start even before the first
        // file lands (e.g. the memory dir on a fresh node) — the watcher then picks
        // up drawers as the palace writes them.
        let _ = std::fs::create_dir_all(&base);
        let picker = SharedFilePicker::default();
        let frecency = SharedFrecency::default();
        if let Ok(f) = FrecencyTracker::open(db_dir.join("frecency")) {
            let _ = frecency.init(f);
        }
        let exists = base.exists();
        let ready = exists
            && FilePicker::new_with_shared_state(
                picker.clone(),
                frecency.clone(),
                FilePickerOptions {
                    base_path: base.to_string_lossy().into_owned(),
                    mode: FFFMode::Ai,
                    enable_content_indexing: true,
                    enable_mmap_cache: true,
                    watch: true,
                    ..Default::default()
                },
            )
            .is_ok();
        eprintln!(
            "[INFO] [finder] {} index {} over {}",
            label,
            if ready { "started" } else { "skipped" },
            base.display()
        );
        Index {
            picker,
            frecency,
            base,
            ready,
        }
    }

    fn find(&self, query: &str, limit: usize) -> Vec<FileHit> {
        if !self.ready || query.is_empty() {
            return Vec::new();
        }
        self.picker.wait_for_scan(Duration::from_secs(6));
        let guard = match self.picker.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let picker = match guard.as_ref() {
            Some(p) => p,
            None => return Vec::new(),
        };
        let parser = QueryParser::default();
        let q = parser.parse(query);
        let results = picker.fuzzy_search(
            &q,
            None,
            FuzzySearchOptions {
                pagination: PaginationArgs { offset: 0, limit },
                ..Default::default()
            },
        );
        results
            .items
            .iter()
            .map(|it| FileHit {
                path: self
                    .base
                    .join(it.relative_path(picker))
                    .to_string_lossy()
                    .into_owned(),
            })
            .collect()
    }

    fn grep(&self, query: &str, limit: usize) -> Vec<GrepHit> {
        if !self.ready || query.is_empty() {
            return Vec::new();
        }
        self.picker.wait_for_scan(Duration::from_secs(6));
        let guard = match self.picker.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let picker = match guard.as_ref() {
            Some(p) => p,
            None => return Vec::new(),
        };
        let q = parse_grep_query(query);
        let result = picker.grep(
            &q,
            &GrepSearchOptions {
                page_limit: limit,
                max_matches_per_file: 8,
                ..Default::default()
            },
        );
        result
            .matches
            .iter()
            .take(limit)
            .map(|m| GrepHit {
                path: result
                    .files
                    .get(m.file_index)
                    .map(|f| {
                        self.base
                            .join(f.relative_path(picker))
                            .to_string_lossy()
                            .into_owned()
                    })
                    .unwrap_or_default(),
                line: m.line_number,
                text: m.line_content.clone(),
                is_def: m.is_definition,
            })
            .collect()
    }
}

/// Actor-owned finder: a project index + a memory index, one search substrate.
pub struct FinderState {
    proj: Index,
    mem: Index,
}

impl Default for FinderState {
    fn default() -> Self {
        Self::init()
    }
}

impl FinderState {
    pub fn init() -> Self {
        let proj_base = harmonia_config_store::get_own("workspace", "root")
            .ok()
            .flatten()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let mem_base = harmonia_config_store::paths::state_root().join("mempalace");
        let db = harmonia_config_store::paths::state_root().join("finder");
        FinderState {
            proj: Index::open("project", proj_base, db.join("proj")),
            mem: Index::open("memory", mem_base, db.join("mem")),
        }
    }

    /// Which indexes a scope covers. "all" merges project + memory.
    fn indexes(&self, scope: &str) -> Vec<&Index> {
        match scope {
            "memory" | "mem" => vec![&self.mem],
            "project" | "proj" => vec![&self.proj],
            _ => vec![&self.proj, &self.mem],
        }
    }

    pub fn find_files(&self, query: &str, limit: usize, scope: &str) -> Vec<FileHit> {
        let mut out = Vec::new();
        for idx in self.indexes(scope) {
            out.extend(idx.find(query, limit));
            if out.len() >= limit {
                break;
            }
        }
        out.truncate(limit);
        out
    }

    pub fn grep(&self, query: &str, limit: usize, scope: &str) -> Vec<GrepHit> {
        let mut out = Vec::new();
        for idx in self.indexes(scope) {
            out.extend(idx.grep(query, limit));
            if out.len() >= limit {
                break;
            }
        }
        out.truncate(limit);
        out
    }

    pub fn bases(&self) -> (String, String) {
        (
            self.proj.base.to_string_lossy().into_owned(),
            self.mem.base.to_string_lossy().into_owned(),
        )
    }
}

/// Escape a string for safe inclusion inside an s-expr "..." literal.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

fn limit_of(sexp: &str, default: usize, cap: usize) -> usize {
    harmonia_actor_protocol::sexp::extract_u64_or(sexp, ":limit", default as u64).min(cap as u64)
        as usize
}

/// IPC dispatch — parses op + optional :scope and returns an s-expr reply.
pub fn dispatch(state: &mut FinderState, sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    let scope = extract_sexp_string(sexp, ":scope").unwrap_or_else(|| "all".into());
    match op.as_str() {
        "find-files" | "find" => {
            let query = extract_sexp_string(sexp, ":query").unwrap_or_default();
            let limit = limit_of(sexp, 12, 50);
            let hits = state.find_files(&query, limit, &scope);
            let body: String = hits
                .iter()
                .map(|h| format!("(:path \"{}\")", esc(&h.path)))
                .collect::<Vec<_>>()
                .join(" ");
            format!("(:ok :count {} :results ({}))", hits.len(), body)
        }
        "grep" => {
            let query = extract_sexp_string(sexp, ":query").unwrap_or_default();
            let limit = limit_of(sexp, 20, 100);
            let hits = state.grep(&query, limit, &scope);
            let body: String = hits
                .iter()
                .map(|h| {
                    format!(
                        "(:path \"{}\" :line {} :def {} :text \"{}\")",
                        esc(&h.path),
                        h.line,
                        if h.is_def { "t" } else { "nil" },
                        esc(&h.text)
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("(:ok :count {} :results ({}))", hits.len(), body)
        }
        "base" => {
            let (p, m) = state.bases();
            format!("(:ok :project \"{}\" :memory \"{}\")", esc(&p), esc(&m))
        }
        "ready" => "(:ok :ready t)".to_string(),
        other => format!("(:error \"finder: unknown op '{}'\")", esc(other)),
    }
}
