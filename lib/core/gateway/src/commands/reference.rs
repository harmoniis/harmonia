/// @path reference expansion — inlines file/directory contents into message text.

/// Maximum bytes to inline per file reference.
const AT_REF_MAX_BYTES: usize = 4096;
/// Maximum number of @references per message.
pub(crate) const AT_REF_MAX_COUNT: usize = 5;

/// Recursively collect file tree paths — simple, no decorations.
fn collect_tree(
    dir: &std::path::Path,
    root: &std::path::Path,
    out: &mut Vec<String>,
    depth: usize,
    limit: usize,
) {
    if out.len() >= limit || depth > 5 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut items: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    items.sort_by_key(|e| e.file_name());
    for entry in items {
        if out.len() >= limit {
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        // Skip hidden files and common noise.
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(&entry.path())
            .to_string_lossy()
            .into_owned();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            out.push(format!("{}/", rel));
            collect_tree(&entry.path(), root, out, depth + 1, limit);
        } else {
            out.push(rel);
        }
    }
}

/// Expand @path references in message text.
///
/// Finds tokens matching `@<path>` where path is a relative or absolute file/dir path.
/// For files: inlines content wrapped in [FILE: path] ... [/FILE] markers.
/// For directories: lists contents wrapped in [DIR: path] ... [/DIR] markers.
/// Paths are resolved relative to the workspace root (config-store or cwd).
///
/// Generic — works for ALL frontends (TUI, Telegram, MQTT, etc.).
pub(crate) fn expand_at_references(text: &str) -> String {
    // Use cwd (like Claude Code), fall back to config, then ".".
    let workspace = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| {
            harmonia_config_store::get_own("workspace", "root")
                .ok()
                .flatten()
                .unwrap_or_else(|| ".".into())
        });
    let root = std::path::Path::new(&workspace);

    let mut result = text.to_string();
    let mut count = 0;

    // Find @references: @word where word looks like a path (contains / or .).
    let tokens: Vec<String> = text
        .split_whitespace()
        .filter(|w| w.starts_with('@') && w.len() > 1)
        .filter(|w| {
            let path_part = &w[1..];
            path_part.contains('/') || path_part.contains('.')
        })
        .map(|w| w.to_string())
        .collect();

    for token in tokens {
        if count >= AT_REF_MAX_COUNT {
            break;
        }
        let path_str = &token[1..]; // Strip @

        let candidate = if std::path::Path::new(path_str).is_absolute() {
            std::path::PathBuf::from(path_str)
        } else {
            root.join(path_str)
        };

        // Security: verify path is within workspace.
        let canonical = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => continue, // File doesn't exist, skip silently.
        };
        let root_canonical = match root.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !canonical.starts_with(&root_canonical) {
            continue; // Path escape attempt, skip.
        }

        let expansion = if canonical.is_dir() {
            // Directory: recursive tree listing (simple, no formatting).
            let mut tree = Vec::new();
            collect_tree(&canonical, &canonical, &mut tree, 0, 100);
            if tree.is_empty() {
                continue;
            }
            format!("\n[DIR: {}]\n{}\n[/DIR]", path_str, tree.join("\n"))
        } else {
            // File: inline content (capped).
            match std::fs::read_to_string(&canonical) {
                Ok(content) => {
                    let capped = if content.len() > AT_REF_MAX_BYTES {
                        format!(
                            "{}...[truncated at {}B]",
                            &content[..AT_REF_MAX_BYTES],
                            AT_REF_MAX_BYTES
                        )
                    } else {
                        content
                    };
                    format!("\n[FILE: {}]\n{}\n[/FILE]", path_str, capped)
                }
                Err(_) => continue,
            }
        };

        result = result.replacen(&token, &expansion, 1);
        count += 1;
    }

    result
}
