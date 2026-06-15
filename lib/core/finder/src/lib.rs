//! harmonia-finder — the default local retrieval substrate.
//!
//! A long-lived actor wrapping the `fff-search` engine: it holds a persistent
//! `FilePicker` (background scan + filesystem watcher + mmap content cache +
//! LMDB frecency) so repeated fuzzy file-path and content searches over the
//! project tree and the on-disk memory files are fast and frecency-ranked.
//!
//! NO FFI: this is a pure Rust crate dependency. The Lisp side reaches it only
//! through the s-expr IPC actor boundary, exactly like every other component.
//!
//! Two ops:
//!   (:component "finder" :op "find-files" :query "..." :limit N)  → fuzzy path search
//!   (:component "finder" :op "grep"       :query "..." :limit N)  → content search
//!
//! Both return a BOUNDED top-N set (never an unbounded list) — the retrieval
//! half of "always choose from a finite matrix of possibilities". Harmonic
//! ranking of that bounded set is layered on the Lisp side.

use std::path::PathBuf;
use std::time::Duration;

use fff_search::file_picker::FilePicker;
use fff_search::frecency::FrecencyTracker;
use fff_search::{
    parse_grep_query, FFFMode, FilePickerOptions, FuzzySearchOptions, GrepSearchOptions,
    PaginationArgs, QueryParser, SharedFilePicker, SharedFrecency,
};

use harmonia_actor_protocol::extract_sexp_string;

/// A fuzzy file-path hit (relative to the index base).
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

/// Actor-owned finder state: the persistent fff-search index.
pub struct FinderState {
    picker: SharedFilePicker,
    #[allow(dead_code)]
    frecency: SharedFrecency,
    base: PathBuf,
    ready: bool,
}

impl Default for FinderState {
    fn default() -> Self {
        Self::init()
    }
}

impl FinderState {
    /// Build the index over the workspace root, with LMDB frecency under the
    /// state root. Degrades gracefully: if anything fails, `ready=false` and
    /// searches return empty so callers fall back to their existing path.
    pub fn init() -> Self {
        let base = harmonia_config_store::get_own("workspace", "root")
            .ok()
            .flatten()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let db_dir = harmonia_config_store::paths::state_root().join("finder");
        let _ = std::fs::create_dir_all(&db_dir);

        let picker = SharedFilePicker::default();
        let frecency = SharedFrecency::default();
        if let Ok(f) = FrecencyTracker::open(db_dir.join("frecency")) {
            let _ = frecency.init(f);
        }

        let ready = FilePicker::new_with_shared_state(
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
            "[INFO] [finder] index {} over {}",
            if ready { "started" } else { "FAILED" },
            base.display()
        );

        FinderState {
            picker,
            frecency,
            base,
            ready,
        }
    }

    pub fn base(&self) -> &PathBuf {
        &self.base
    }

    /// Fuzzy file-path search → bounded top-`limit` paths, frecency-ranked.
    pub fn find_files(&self, query: &str, limit: usize) -> Vec<FileHit> {
        if !self.ready || query.is_empty() {
            return Vec::new();
        }
        // First search blocks briefly for the background scan; later ones are instant.
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
                pagination: PaginationArgs {
                    offset: 0,
                    limit,
                },
                ..Default::default()
            },
        );
        results
            .items
            .iter()
            .map(|it| FileHit {
                path: it.relative_path(picker),
            })
            .collect()
    }

    /// Content search (grep) → bounded top-`limit` matches.
    pub fn grep(&self, query: &str, limit: usize) -> Vec<GrepHit> {
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
                    .map(|f| f.relative_path(picker))
                    .unwrap_or_default(),
                line: m.line_number,
                text: m.line_content.clone(),
                is_def: m.is_definition,
            })
            .collect()
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
    harmonia_actor_protocol::sexp::extract_u64_or(sexp, ":limit", default as u64).min(cap as u64) as usize
}

/// IPC dispatch — parses the op and returns an s-expr reply.
pub fn dispatch(state: &mut FinderState, sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "find-files" | "find" => {
            let query = extract_sexp_string(sexp, ":query").unwrap_or_default();
            let limit = limit_of(sexp, 12, 50);
            let hits = state.find_files(&query, limit);
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
            let hits = state.grep(&query, limit);
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
        "base" => format!("(:ok :base \"{}\")", esc(&state.base().to_string_lossy())),
        "ready" => format!("(:ok :ready {})", if state.ready { "t" } else { "nil" }),
        other => format!("(:error \"finder: unknown op '{}'\")", esc(other)),
    }
}
