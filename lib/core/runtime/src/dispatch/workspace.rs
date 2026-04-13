//! Workspace component dispatch — the agent's hands for reading the world.
//!
//! Parallel async file operations. The LLM controls these through REPL only.
//! The agent never implements tools in Lisp — tools are Rust actors.
//!
//! Ops: read-file, grep, list-files, file-exists, file-info, write-file.
//! Security: paths are sandboxed to workspace root (no escape via ../).

use harmonia_actor_protocol::sexp_escape;
use std::path::{Path, PathBuf};

use super::{param, param_u64};

/// Resolve and validate a path within the workspace root.
/// Returns None if the path escapes the sandbox.
fn safe_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let candidate = if Path::new(relative).is_absolute() {
        PathBuf::from(relative)
    } else {
        root.join(relative)
    };
    let canonical = candidate.canonicalize().ok()?;
    let root_canonical = root.canonicalize().ok()?;
    if canonical.starts_with(&root_canonical) {
        Some(canonical)
    } else {
        None
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
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    let root = workspace_root();

    match op.as_str() {
        "healthcheck" => "(:ok :status \"workspace ready\")".to_string(),

        "read-file" => {
            let path_str = param!(sexp, ":path");
            if path_str.is_empty() {
                return "(:error \"read-file: :path required\")".to_string();
            }
            let Some(path) = safe_path(&root, &path_str) else {
                return format!("(:error \"read-file: path outside workspace: {}\")", sexp_escape(&path_str));
            };
            let offset = param_u64!(sexp, ":offset", 0) as usize;
            let limit = param_u64!(sexp, ":limit", 200) as usize;
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = offset.min(lines.len());
                    let end = (offset + limit).min(lines.len());
                    let result: String = lines[start..end]
                        .iter()
                        .enumerate()
                        .map(|(i, line)| format!("{}: {}", start + i + 1, line))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let capped = if result.len() > 8192 { &result[..8192] } else { &result };
                    format!("(:ok :lines {} :total {} :result \"{}\")",
                        end - start, lines.len(), sexp_escape(capped))
                }
                Err(e) => format!("(:error \"read-file: {}\")", sexp_escape(&e.to_string())),
            }
        }

        "grep" => {
            let pattern = param!(sexp, ":pattern");
            let path_str = param!(sexp, ":path", ".");
            if pattern.is_empty() {
                return "(:error \"grep: :pattern required\")".to_string();
            }
            let Some(search_path) = safe_path(&root, &path_str) else {
                return format!("(:error \"grep: path outside workspace: {}\")", sexp_escape(&path_str));
            };
            let limit = param_u64!(sexp, ":limit", 30) as usize;
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
                    let root_str = root.to_string_lossy();
                    let cleaned = stdout.replace(&*root_str, ".");
                    let capped = if cleaned.len() > 8192 { &cleaned[..8192] } else { &cleaned };
                    let count = capped.lines().count();
                    format!("(:ok :matches {} :result \"{}\")", count, sexp_escape(capped.trim()))
                }
                Err(e) => format!("(:error \"grep: {}\")", sexp_escape(&e.to_string())),
            }
        }

        "list-files" => {
            let path_str = param!(sexp, ":path", ".");
            let Some(dir) = safe_path(&root, &path_str) else {
                return format!("(:error \"list-files: path outside workspace: {}\")", sexp_escape(&path_str));
            };
            let pattern = param!(sexp, ":pattern");
            let limit = param_u64!(sexp, ":limit", 50) as usize;
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let items: Vec<String> = entries
                        .take(limit * 2)
                        .filter_map(|e| e.ok())
                        .filter(|entry| {
                            pattern.is_empty() || entry.file_name().to_string_lossy().contains(&*pattern)
                        })
                        .take(limit)
                        .map(|entry| {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            format!("(:name \"{}\" :type {} :size {})",
                                sexp_escape(&name),
                                if is_dir { ":dir" } else { ":file" },
                                size)
                        })
                        .collect();
                    format!("(:ok :count {} :entries ({}))", items.len(), items.join(" "))
                }
                Err(e) => format!("(:error \"list-files: {}\")", sexp_escape(&e.to_string())),
            }
        }

        "file-exists" => {
            let path_str = param!(sexp, ":path");
            let Some(path) = safe_path(&root, &path_str) else {
                return "(:ok :exists nil)".to_string();
            };
            format!("(:ok :exists {})", if path.exists() { "t" } else { "nil" })
        }

        "file-info" => {
            let path_str = param!(sexp, ":path");
            let Some(path) = safe_path(&root, &path_str) else {
                return format!("(:error \"file-info: path outside workspace: {}\")", sexp_escape(&path_str));
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
                Err(e) => format!("(:error \"file-info: {}\")", sexp_escape(&e.to_string())),
            }
        }

        "exec" => {
            let cmd = param!(sexp, ":cmd");
            if cmd.is_empty() {
                return "(:error \"exec: :cmd required\")".to_string();
            }
            let args_str = param!(sexp, ":args");
            let cwd_str = param!(sexp, ":cwd", ".");
            let cwd = root.join(&cwd_str);
            // When cmd contains spaces and args is empty, use sh -c for natural commands.
            // Models write (exec "uname -a") not (exec "uname" "-a").
            let output = if args_str.is_empty() && cmd.contains(' ') {
                std::process::Command::new("sh")
                    .args(["-c", &cmd])
                    .current_dir(&cwd)
                    .output()
            } else {
                let args: Vec<&str> = if args_str.is_empty() {
                    vec![]
                } else {
                    args_str.split_whitespace().collect()
                };
                std::process::Command::new(&cmd)
                    .args(&args)
                    .current_dir(&cwd)
                    .output()
            };
            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    // Only include stderr if the process FAILED (non-zero exit).
                    // Successful processes with stderr warnings should return stdout only.
                    let combined = if out.status.success() || stderr.is_empty() {
                        if stdout.is_empty() && !stderr.is_empty() {
                            format!("STDERR:\n{}", stderr)  // no stdout but has stderr
                        } else {
                            stdout.to_string()  // success: stdout only
                        }
                    } else {
                        format!("{}\nSTDERR:\n{}", stdout, stderr)  // failure: both
                    };
                    let capped = if combined.len() > 8192 { &combined[..8192] } else { &combined };
                    let exit_code = out.status.code().unwrap_or(-1);
                    format!("(:ok :exit {} :result \"{}\")", exit_code, sexp_escape(capped.trim()))
                }
                Err(e) => format!("(:error \"exec: {}\")", sexp_escape(&e.to_string())),
            }
        }

        "write-file" => {
            let path_str = param!(sexp, ":path");
            let content = param!(sexp, ":content");
            if path_str.is_empty() {
                return "(:error \"write-file: :path required\")".to_string();
            }
            let path = root.join(&path_str);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&path, &content) {
                Ok(_) => format!("(:ok :bytes {})", content.len()),
                Err(e) => format!("(:error \"write-file: {}\")", sexp_escape(&e.to_string())),
            }
        }

        "append-file" => {
            let path_str = param!(sexp, ":path");
            let content = param!(sexp, ":content");
            if path_str.is_empty() {
                return "(:error \"append-file: :path required\")".to_string();
            }
            let path = root.join(&path_str);
            use std::io::Write;
            match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                Ok(mut f) => match f.write_all(content.as_bytes()) {
                    Ok(_) => format!("(:ok :bytes {})", content.len()),
                    Err(e) => format!("(:error \"append-file: {}\")", sexp_escape(&e.to_string())),
                },
                Err(e) => format!("(:error \"append-file: {}\")", sexp_escape(&e.to_string())),
            }
        }

        _ => format!("(:error \"workspace: unknown op '{}'\")", sexp_escape(&op)),
    }
}
