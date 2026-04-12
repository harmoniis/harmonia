//! Memory-field component dispatch — routes sexp commands through the Service pattern.
//!
//! Parse sexp -> FieldCommand -> handle(&self) -> (Delta, Result) -> apply(&mut self) -> to_sexp()
//! Mutation is confined to apply(). Serialization happens at the boundary.

use harmonia_actor_protocol::sexp_escape;

use super::{param, param_f64, param_u64};

pub(crate) fn dispatch(
    sexp: &str,
    field: &mut harmonia_memory_field::FieldState,
) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();

    // Special cases that don't go through Service (init, last-field-basin).
    match op.as_str() {
        "init" => return "(:ok)".to_string(),
        "last-field-basin" => {
            return match harmonia_chronicle::tables::harmonic::last_field_basin() {
                Ok((basin, energy, dwell, threshold)) => {
                    format!(
                        "(:ok :basin \"{}\" :coercive-energy {:.3} :dwell-ticks {} :threshold {:.3})",
                        sexp_escape(&basin), energy, dwell, threshold
                    )
                }
                Err(e) => format!("(:error \"last-field-basin: {e}\")"),
            };
        }
        _ => {}
    }

    // Parse command from sexp.
    let cmd = match parse_command(sexp, &op) {
        Some(cmd) => cmd,
        None => return format!("(:error \"unknown memory-field op: {}\")", op),
    };

    // Handle + apply + serialize (the Service pattern).
    use harmonia_actor_protocol::Service;
    match field.handle(cmd) {
        Ok((delta, result)) => {
            field.apply(delta);
            result.to_sexp()
        }
        Err(e) => {
            let msg = e.to_string();
            format!("(:error \"{}: {}\")", op, sexp_escape(&msg))
        }
    }
}

/// Parse sexp into a typed FieldCommand. Returns None for unknown ops.
fn parse_command(sexp: &str, op: &str) -> Option<harmonia_memory_field::FieldCommand> {
    use harmonia_memory_field::FieldCommand;
    match op {
        "load-graph" => Some(FieldCommand::LoadGraph {
            nodes: parse_memory_field_nodes(sexp),
            edges: parse_memory_field_edges(sexp),
            directed_weights: parse_memory_field_directed_weights(sexp),
        }),
        "field-recall" => {
            let concepts = parse_string_list(sexp, ":query-concepts");
            let access = parse_memory_field_access_counts(sexp);
            let limit = param_u64!(sexp, ":limit", 0) as usize;
            Some(FieldCommand::Recall {
                query_concepts: concepts,
                access_counts: access,
                limit: if limit == 0 { 10 } else { limit },
            })
        }
        "field-recall-structural" => {
            let concepts = parse_string_list(sexp, ":query-concepts");
            let limit = param_u64!(sexp, ":limit", 0) as usize;
            Some(FieldCommand::RecallStructural {
                query_concepts: concepts,
                limit: if limit == 0 { 5 } else { limit },
            })
        }
        "step-attractors" => Some(FieldCommand::StepAttractors {
            signal: param_f64!(sexp, ":signal", 0.0),
            noise: param_f64!(sexp, ":noise", 0.0),
        }),
        "basin-status" => Some(FieldCommand::BasinStatus),
        "eigenmode-status" => Some(FieldCommand::EigenmodeStatus),
        "status" => Some(FieldCommand::Status),
        "restore-basin" => {
            let basin = param!(sexp, ":basin", "thomas-0");
            let energy = param_f64!(sexp, ":coercive-energy", 0.0);
            let dwell = param_u64!(sexp, ":dwell-ticks", 0);
            let threshold = param_f64!(sexp, ":threshold", 0.0);
            Some(FieldCommand::RestoreBasin {
                basin_str: basin,
                coercive_energy: energy,
                dwell_ticks: dwell,
                threshold: if threshold < 0.01 { 0.35 } else { threshold },
            })
        }
        "current-basin" => Some(FieldCommand::CurrentBasin),
        "dream" => Some(FieldCommand::Dream),
        "dream-stats" => Some(FieldCommand::DreamStats),
        "edge-currents" => Some(FieldCommand::EdgeCurrents),
        "digest" => Some(FieldCommand::Digest),
        "load-genesis" => {
            let concepts = parse_genesis_concepts(sexp);
            let edges = parse_genesis_edges(sexp);
            let entry = harmonia_memory_field::GenesisEntry { concepts, edges };
            Some(FieldCommand::LoadGenesis {
                entries: vec![entry],
            })
        }
        "bootstrap" => Some(FieldCommand::Bootstrap),
        "reset" => Some(FieldCommand::Reset),
        "checkpoint" => Some(FieldCommand::Checkpoint),
        "save-to-disk" => Some(FieldCommand::SaveToDisk),
        "load-from-disk" => Some(FieldCommand::LoadFromDisk),
        "restore-state" => {
            let tx = param_f64!(sexp, ":thomas-x", 0.1);
            let ty = param_f64!(sexp, ":thomas-y", 0.0);
            let tz = param_f64!(sexp, ":thomas-z", 0.0);
            let ax = param_f64!(sexp, ":aizawa-x", 0.1);
            let ay = param_f64!(sexp, ":aizawa-y", 0.0);
            let az = param_f64!(sexp, ":aizawa-z", 0.0);
            let hx = param_f64!(sexp, ":halvorsen-x", 0.1);
            let hy = param_f64!(sexp, ":halvorsen-y", 0.0);
            let hz = param_f64!(sexp, ":halvorsen-z", 0.0);
            let signal = param_f64!(sexp, ":last-signal", 0.5);
            let noise = param_f64!(sexp, ":last-noise", 0.2);
            let thomas_b = param_f64!(sexp, ":thomas-b", 0.208);
            let sb0 = param_f64!(sexp, ":sb0", 1.0 / 6.0);
            let sb1 = param_f64!(sexp, ":sb1", 1.0 / 6.0);
            let sb2 = param_f64!(sexp, ":sb2", 1.0 / 6.0);
            let sb3 = param_f64!(sexp, ":sb3", 1.0 / 6.0);
            let sb4 = param_f64!(sexp, ":sb4", 1.0 / 6.0);
            let sb5 = param_f64!(sexp, ":sb5", 1.0 / 6.0);
            Some(FieldCommand::RestoreState {
                thomas: (tx, ty, tz),
                aizawa: (ax, ay, az),
                halvorsen: (hx, hy, hz),
                signal,
                noise,
                soft_basins: [sb0, sb1, sb2, sb3, sb4, sb5],
                thomas_b,
            })
        }
        _ => None,
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

/// Parse directed weights (forward-weight, reverse-weight) from edge sexp.
/// Returns (a, b, forward_weight, reverse_weight) for edges that have directed data.
fn parse_memory_field_directed_weights(sexp: &str) -> Vec<(String, String, f64, f64)> {
    sexp.find(":edges")
        .and_then(|start| sexp[start + 6..].find('(').map(|lp| &sexp[start + 6 + lp..]))
        .into_iter()
        .flat_map(|inner| inner.split(":a ").skip(1))
        .filter_map(|chunk| {
            let a = extract_first_quoted(chunk).filter(|s| !s.is_empty())?;
            let b = extract_after_keyword(chunk, ":b").filter(|s| !s.is_empty())?;
            let fwd = extract_after_keyword(chunk, ":forward-weight")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let rev = extract_after_keyword(chunk, ":reverse-weight")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            // Only include if at least one directed weight is present
            if fwd > 0.0 || rev > 0.0 {
                Some((a, b, fwd.max(1.0), rev.max(1.0)))
            } else {
                None
            }
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

// ── Phase 8: Genesis sexp parsers ──────────────────────────────────────

/// Parse genesis concept pairs from sexp: :concepts ("concept1" "domain1" "concept2" "domain2" ...)
fn parse_genesis_concepts(sexp: &str) -> Vec<(String, String)> {
    harmonia_actor_protocol::extract_sexp_string_list(sexp, ":concepts")
        .chunks(2)
        .filter_map(|pair| {
            if pair.len() == 2 { Some((pair[0].clone(), pair[1].clone())) } else { None }
        })
        .collect()
}

/// Parse genesis edge triples from sexp, reusing the existing edge parser.
fn parse_genesis_edges(sexp: &str) -> Vec<(String, String, f64)> {
    parse_memory_field_edges(sexp).into_iter()
        .map(|(a, b, w, _)| (a, b, w))
        .collect()
}
