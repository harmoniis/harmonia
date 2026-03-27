//! Memory-field component dispatch — requires actor-owned FieldState.

use harmonia_actor_protocol::{extract_sexp_f64, extract_sexp_string, extract_sexp_u64_or, sexp_escape};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(
    sexp: &str,
    field: &mut harmonia_memory_field::FieldState,
) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => {
            // Init is a no-op: the actor already owns a FieldState.
            "(:ok)".to_string()
        }
        "load-graph" => {
            let nodes = parse_memory_field_nodes(sexp);
            let edges = parse_memory_field_edges(sexp);
            match harmonia_memory_field::load_graph(field, nodes, edges) {
                Ok(result) => result,
                Err(e) => format!("(:error \"load-graph: {e}\")"),
            }
        }
        "field-recall" => {
            let concepts = parse_string_list(sexp, ":query-concepts");
            let access = parse_memory_field_access_counts(sexp);
            let limit = extract_sexp_u64_or(sexp, ":limit", 0) as usize;
            let limit = if limit == 0 { 10 } else { limit };
            match harmonia_memory_field::field_recall(field, concepts, access, limit) {
                Ok(result) => result,
                Err(e) => format!("(:error \"field-recall: {e}\")"),
            }
        }
        "step-attractors" => {
            let signal = extract_sexp_f64(sexp, ":signal").unwrap_or(0.0);
            let noise = extract_sexp_f64(sexp, ":noise").unwrap_or(0.0);
            match harmonia_memory_field::step_attractors(field, signal, noise) {
                Ok(result) => result,
                Err(e) => format!("(:error \"step-attractors: {e}\")"),
            }
        }
        "basin-status" => match harmonia_memory_field::basin_status(field) {
            Ok(result) => result,
            Err(e) => format!("(:error \"basin-status: {e}\")"),
        },
        "eigenmode-status" => match harmonia_memory_field::eigenmode_status(field) {
            Ok(result) => result,
            Err(e) => format!("(:error \"eigenmode-status: {e}\")"),
        },
        "status" => match harmonia_memory_field::status(field) {
            Ok(result) => result,
            Err(e) => format!("(:error \"status: {e}\")"),
        },
        "restore-basin" => {
            let basin = extract_sexp_string(sexp, ":basin").unwrap_or_else(|| "thomas-0".into());
            let energy = extract_sexp_f64(sexp, ":coercive-energy").unwrap_or(0.0);
            let dwell = extract_sexp_u64_or(sexp, ":dwell-ticks", 0);
            let threshold = extract_sexp_f64(sexp, ":threshold").unwrap_or(0.0);
            let threshold = if threshold < 0.01 { 0.35 } else { threshold };
            match harmonia_memory_field::restore_basin(field, &basin, energy, dwell, threshold) {
                Ok(result) => result,
                Err(e) => format!("(:error \"restore-basin: {e}\")"),
            }
        }
        "last-field-basin" => {
            // Query Chronicle for last basin state (used by Lisp warm-start).
            match harmonia_chronicle::tables::harmonic::last_field_basin() {
                Ok((basin, energy, dwell, threshold)) => {
                    format!(
                        "(:ok :basin \"{}\" :coercive-energy {:.3} :dwell-ticks {} :threshold {:.3})",
                        esc(&basin), energy, dwell, threshold
                    )
                }
                Err(e) => format!("(:error \"last-field-basin: {e}\")"),
            }
        }
        "reset" => match harmonia_memory_field::reset(field) {
            Ok(result) => result,
            Err(e) => format!("(:error \"reset: {e}\")"),
        },
        _ => format!("(:error \"unknown memory-field op: {}\")", op),
    }
}

// ── Memory-field sexp parsers ────────────────────────────────────────

/// Parse node list from memory-field load-graph sexp.
/// Expected format: :nodes ((:concept "x" :domain "y" :count N :entries ("e1" "e2")) ...)
fn parse_memory_field_nodes(sexp: &str) -> Vec<(String, String, i32, Vec<String>)> {
    let mut nodes = Vec::new();
    if let Some(nodes_start) = sexp.find(":nodes") {
        let rest = &sexp[nodes_start + 6..];
        if let Some(list_start) = rest.find('(') {
            let inner = &rest[list_start..];
            for chunk in inner.split(":concept").skip(1) {
                let concept = extract_first_quoted(chunk).unwrap_or_default();
                let domain = extract_after_keyword(chunk, ":domain").unwrap_or_else(|| "generic".into());
                let count = extract_after_keyword(chunk, ":count")
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(1);
                let entries = extract_string_list_inline(chunk, ":entries");
                if !concept.is_empty() {
                    nodes.push((concept, domain, count, entries));
                }
            }
        }
    }
    nodes
}

/// Parse edge list from memory-field load-graph sexp.
fn parse_memory_field_edges(sexp: &str) -> Vec<(String, String, f64, bool)> {
    let mut edges = Vec::new();
    if let Some(edges_start) = sexp.find(":edges") {
        let rest = &sexp[edges_start + 6..];
        if let Some(list_start) = rest.find('(') {
            let inner = &rest[list_start..];
            for chunk in inner.split(":a ").skip(1) {
                let a = extract_first_quoted(chunk).unwrap_or_default();
                let b = extract_after_keyword(chunk, ":b").unwrap_or_default();
                let weight = extract_after_keyword(chunk, ":weight")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);
                let inter = extract_after_keyword(chunk, ":interdisciplinary")
                    .map(|s| s == "t")
                    .unwrap_or(false);
                if !a.is_empty() && !b.is_empty() {
                    edges.push((a, b, weight, inter));
                }
            }
        }
    }
    edges
}

/// Parse access counts from memory-field field-recall sexp.
fn parse_memory_field_access_counts(sexp: &str) -> Vec<(String, f64)> {
    let mut counts = Vec::new();
    if let Some(start) = sexp.find(":access-counts") {
        let rest = &sexp[start + 14..];
        for chunk in rest.split(":concept").skip(1) {
            let concept = extract_first_quoted(chunk).unwrap_or_default();
            let count = extract_after_keyword(chunk, ":count")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            if !concept.is_empty() {
                counts.push((concept, count));
            }
        }
    }
    counts
}

/// Parse a list of quoted strings like ("a" "b" "c") after a keyword.
fn parse_string_list(sexp: &str, key: &str) -> Vec<String> {
    extract_string_list_inline(sexp, key)
}

/// Extract the first quoted string from a text chunk.
fn extract_first_quoted(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let end = start + s[start..].find('"')?;
    Some(s[start..end].to_string())
}

/// Extract a value after a keyword like :domain "engineering".
fn extract_after_keyword(s: &str, key: &str) -> Option<String> {
    let pos = s.find(key)? + key.len();
    let rest = s[pos..].trim_start();
    if rest.starts_with('"') {
        extract_first_quoted(rest)
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')' || c == '(')
            .unwrap_or(rest.len());
        let token = rest[..end].trim();
        if token.is_empty() {
            None
        } else {
            Some(token.to_string())
        }
    }
}

/// Extract a list of quoted strings after a keyword.
fn extract_string_list_inline(s: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    if let Some(pos) = s.find(key) {
        let rest = &s[pos + key.len()..];
        if let Some(open) = rest.find('(') {
            let inner = &rest[open + 1..];
            if let Some(close) = inner.find(')') {
                let content = &inner[..close];
                let mut in_quote = false;
                let mut current = String::new();
                for ch in content.chars() {
                    match ch {
                        '"' if !in_quote => {
                            in_quote = true;
                            current.clear();
                        }
                        '"' if in_quote => {
                            in_quote = false;
                            if !current.is_empty() {
                                items.push(current.clone());
                            }
                        }
                        _ if in_quote => current.push(ch),
                        _ => {}
                    }
                }
            }
        }
    }
    items
}
