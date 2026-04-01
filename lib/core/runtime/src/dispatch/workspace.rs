//! Workspace component dispatch — the agent's hands for reading the world.
//!
//! Parallel async file operations. The LLM controls these through REPL only.
//! The agent never implements tools in Lisp — tools are Rust actors.
//!
//! Ops: read-file, grep, list-files, file-exists, file-info, write-file.
//! Security: paths are sandboxed to workspace root (no escape via ../).

use harmonia_actor_protocol::{extract_sexp_string, extract_sexp_u64_or, sexp_escape};
use std::path::{Path, PathBuf};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

/// Resolve and validate a path within the workspace root.
/// Returns None if the path escapes the sandbox.
fn safe_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let candidate = if Path::new(relative).is_absolute() {
        PathBuf::from(relative)
    } else {
        root.join(relative)
    };
    // Canonicalize to resolve .. and symlinks.
    let canonical = candidate.canonicalize().ok()?;
    let root_canonical = root.canonicalize().ok()?;
    if canonical.starts_with(&root_canonical) {
        Some(canonical)
    } else {
        None // Escape attempt blocked.
    }
}

/// Get workspace root from config-store or fallback to cwd.
fn workspace_root() -> PathBuf {
    harmonia_config_store::get_own("workspace", "root")
        .ok()
        .flatten()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    let root = workspace_root();

    match op.as_str() {
        "healthcheck" => "(:ok :status \"workspace ready\")".to_string(),

        "read-file" => {
            let path_str = extract_sexp_string(sexp, ":path").unwrap_or_default();
            if path_str.is_empty() {
                return "(:error \"read-file: :path required\")".to_string();
            }
            let Some(path) = safe_path(&root, &path_str) else {
                return format!("(:error \"read-file: path outside workspace: {}\")", esc(&path_str));
            };
            let offset = extract_sexp_u64_or(sexp, ":offset", 0) as usize;
            let limit = extract_sexp_u64_or(sexp, ":limit", 200) as usize;
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    let end = (offset + limit).min(lines.len());
                    let start = offset.min(lines.len());
                    let slice: Vec<String> = lines[start..end]
                        .iter()
                        .enumerate()
                        .map(|(i, line)| format!("{}: {}", start + i + 1, line))
                        .collect();
                    let result = slice.join("\n");
                    // Cap at 8KB to prevent memory explosion.
                    let capped = if result.len() > 8192 { &result[..8192] } else { &result };
                    format!("(:ok :lines {} :total {} :result \"{}\")",
                        end - start, lines.len(), esc(capped))
                }
                Err(e) => format!("(:error \"read-file: {}\")", esc(&e.to_string())),
            }
        }

        "grep" => {
            let pattern = extract_sexp_string(sexp, ":pattern").unwrap_or_default();
            let path_str = extract_sexp_string(sexp, ":path").unwrap_or_else(|| ".".into());
            if pattern.is_empty() {
                return "(:error \"grep: :pattern required\")".to_string();
            }
            let Some(search_path) = safe_path(&root, &path_str) else {
                return format!("(:error \"grep: path outside workspace: {}\")", esc(&path_str));
            };
            let limit = extract_sexp_u64_or(sexp, ":limit", 30) as usize;
            // Use grep command for speed and regex support.
            let output = std::process::Command::new("grep")
                .args(["-rn", "--include=*.lisp", "--include=*.rs",
                       "--include=*.md", "--include=*.sexp",
                       "--include=*.toml", "--include=*.json",
                       "-m", &limit.to_string()])
                .arg(&pattern)
                .arg(&search_path)
                .output();
            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    // Strip workspace root prefix for cleaner output.
                    let root_str = root.to_string_lossy();
                    let cleaned = stdout.replace(&*root_str, ".");
                    let capped = if cleaned.len() > 8192 { &cleaned[..8192] } else { &cleaned };
                    let count = capped.lines().count();
                    format!("(:ok :matches {} :result \"{}\")", count, esc(capped.trim()))
                }
                Err(e) => format!("(:error \"grep: {}\")", esc(&e.to_string())),
            }
        }

        "list-files" => {
            let path_str = extract_sexp_string(sexp, ":path").unwrap_or_else(|| ".".into());
            let Some(dir) = safe_path(&root, &path_str) else {
                return format!("(:error \"list-files: path outside workspace: {}\")", esc(&path_str));
            };
            let pattern = extract_sexp_string(sexp, ":pattern").unwrap_or_default();
            let limit = extract_sexp_u64_or(sexp, ":limit", 50) as usize;
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut items: Vec<String> = Vec::new();
                    for entry in entries.take(limit * 2) {
                        if let Ok(entry) = entry {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if !pattern.is_empty() && !name.contains(&pattern) {
                                continue;
                            }
                            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            items.push(format!("(:name \"{}\" :type {} :size {})",
                                esc(&name),
                                if is_dir { ":dir" } else { ":file" },
                                size));
                            if items.len() >= limit { break; }
                        }
                    }
                    format!("(:ok :count {} :entries ({}))", items.len(), items.join(" "))
                }
                Err(e) => format!("(:error \"list-files: {}\")", esc(&e.to_string())),
            }
        }

        "file-exists" => {
            let path_str = extract_sexp_string(sexp, ":path").unwrap_or_default();
            let Some(path) = safe_path(&root, &path_str) else {
                return "(:ok :exists nil)".to_string();
            };
            format!("(:ok :exists {})", if path.exists() { "t" } else { "nil" })
        }

        "file-info" => {
            let path_str = extract_sexp_string(sexp, ":path").unwrap_or_default();
            let Some(path) = safe_path(&root, &path_str) else {
                return format!("(:error \"file-info: path outside workspace: {}\")", esc(&path_str));
            };
            match std::fs::metadata(&path) {
                Ok(meta) => {
                    let size = meta.len();
                    let is_dir = meta.is_dir();
                    let lines = if !is_dir {
                        std::fs::read_to_string(&path)
                            .map(|c| c.lines().count())
                            .unwrap_or(0)
                    } else { 0 };
                    format!("(:ok :size {} :lines {} :type {})",
                        size, lines, if is_dir { ":dir" } else { ":file" })
                }
                Err(e) => format!("(:error \"file-info: {}\")", esc(&e.to_string())),
            }
        }

        _ => format!("(:error \"workspace: unknown op '{}'\")", esc(&op)),
    }
}
