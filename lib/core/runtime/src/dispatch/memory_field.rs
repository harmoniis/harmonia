//! Memory-field component dispatch — requires actor-owned FieldState.

use harmonia_actor_protocol::sexp_escape;

use super::{dispatch_op, param, param_f64, param_u64};

pub(crate) fn dispatch(
    sexp: &str,
    field: &mut harmonia_memory_field::FieldState,
) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => "(:ok)".to_string(),
        "load-graph" => {
            let nodes = parse_memory_field_nodes(sexp);
            let edges = parse_memory_field_edges(sexp);
            dispatch_op!("load-graph", harmonia_memory_field::load_graph(field, nodes, edges))
        }
        "field-recall" => {
            let concepts = parse_string_list(sexp, ":query-concepts");
            let access = parse_memory_field_access_counts(sexp);
            let limit = param_u64!(sexp, ":limit", 0) as usize;
            let limit = if limit == 0 { 10 } else { limit };
            dispatch_op!("field-recall", harmonia_memory_field::field_recall(field, concepts, access, limit))
        }
        "step-attractors" => {
            let signal = param_f64!(sexp, ":signal", 0.0);
            let noise = param_f64!(sexp, ":noise", 0.0);
            dispatch_op!("step-attractors", harmonia_memory_field::step_attractors(field, signal, noise))
        }
        "basin-status" => dispatch_op!("basin-status", harmonia_memory_field::basin_status(field)),
        "eigenmode-status" => dispatch_op!("eigenmode-status", harmonia_memory_field::eigenmode_status(field)),
        "status" => dispatch_op!("status", harmonia_memory_field::status(field)),
        "restore-basin" => {
            let basin = param!(sexp, ":basin", "thomas-0");
            let energy = param_f64!(sexp, ":coercive-energy", 0.0);
            let dwell = param_u64!(sexp, ":dwell-ticks", 0);
            let threshold = param_f64!(sexp, ":threshold", 0.0);
            let threshold = if threshold < 0.01 { 0.35 } else { threshold };
            dispatch_op!("restore-basin", harmonia_memory_field::restore_basin(field, &basin, energy, dwell, threshold))
        }
        "last-field-basin" => {
            match harmonia_chronicle::tables::harmonic::last_field_basin() {
                Ok((basin, energy, dwell, threshold)) => {
                    format!(
                        "(:ok :basin \"{}\" :coercive-energy {:.3} :dwell-ticks {} :threshold {:.3})",
                        sexp_escape(&basin), energy, dwell, threshold
                    )
                }
                Err(e) => format!("(:error \"last-field-basin: {e}\")"),
            }
        }
        "dream" => dispatch_op!("dream", harmonia_memory_field::field_dream(field)),
        "edge-currents" => dispatch_op!("edge-currents", harmonia_memory_field::edge_current_status(field)),
        "reset" => dispatch_op!("reset", harmonia_memory_field::reset(field)),
        _ => format!("(:error \"unknown memory-field op: {}\")", op),
    }
}

// -- Memory-field sexp parsers (functional iterator chains) ----------------

/// Parse node list from memory-field load-graph sexp.
/// Expected format: :nodes ((:concept "x" :domain "y" :count N :entries ("e1" "e2")) ...)
fn parse_memory_field_nodes(sexp: &str) -> Vec<(String, String, i32, Vec<String>)> {
    sexp.find(":nodes")
        .and_then(|start| sexp[start + 6..].find('(').map(|lp| &sexp[start + 6 + lp..]))
        .into_iter()
        .flat_map(|inner| inner.split(":concept").skip(1))
        .filter_map(|chunk| {
            let concept = extract_first_quoted(chunk)?;
            if concept.is_empty() { return None; }
            let domain = extract_after_keyword(chunk, ":domain").unwrap_or_else(|| "generic".into());
            let count = extract_after_keyword(chunk, ":count")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(1);
            let entries = extract_string_list_inline(chunk, ":entries");
            Some((concept, domain, count, entries))
        })
        .collect()
}

/// Parse edge list from memory-field load-graph sexp.
fn parse_memory_field_edges(sexp: &str) -> Vec<(String, String, f64, bool)> {
    sexp.find(":edges")
        .and_then(|start| sexp[start + 6..].find('(').map(|lp| &sexp[start + 6 + lp..]))
        .into_iter()
        .flat_map(|inner| inner.split(":a ").skip(1))
        .filter_map(|chunk| {
            let a = extract_first_quoted(chunk).filter(|s| !s.is_empty())?;
            let b = extract_after_keyword(chunk, ":b").filter(|s| !s.is_empty())?;
            let weight = extract_after_keyword(chunk, ":weight")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(1.0);
            let inter = extract_after_keyword(chunk, ":interdisciplinary")
                .map(|s| s == "t")
                .unwrap_or(false);
            Some((a, b, weight, inter))
        })
        .collect()
}

/// Parse access counts from memory-field field-recall sexp.
/// Returns (concept, count, last_access_unix_time).
fn parse_memory_field_access_counts(sexp: &str) -> Vec<(String, f64, f64)> {
    sexp.find(":access-counts")
        .map(|start| &sexp[start + 14..])
        .into_iter()
        .flat_map(|rest| rest.split(":concept").skip(1))
        .filter_map(|chunk| {
            let concept = extract_first_quoted(chunk).filter(|s| !s.is_empty())?;
            let count = extract_after_keyword(chunk, ":count")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let last_access = extract_after_keyword(chunk, ":last-access")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            Some((concept, count, last_access))
        })
        .collect()
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
        if token.is_empty() { None } else { Some(token.to_string()) }
    }
}

/// Extract a list of quoted strings after a keyword.
/// Quote-aware tokenizer expressed as fold over chars.
fn extract_string_list_inline(s: &str, key: &str) -> Vec<String> {
    s.find(key)
        .and_then(|pos| {
            let rest = &s[pos + key.len()..];
            let open = rest.find('(')?;
            let inner = &rest[open + 1..];
            let close = inner.find(')')?;
            Some(&inner[..close])
        })
        .map(|content| {
            content.split('"')
                .enumerate()
                .filter_map(|(i, segment)| {
                    // Odd-indexed segments are inside quotes
                    if i % 2 == 1 && !segment.is_empty() {
                        Some(segment.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}
