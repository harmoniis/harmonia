//! Top-level RPC request dispatch and capability checks.

use harmonia_node_rpc::{
    capability_for_request, default_capabilities, error_response, success_response,
    NodeRpcRequest, NodeRpcRequestEnvelope, NodeRpcResponseEnvelope, NodeRpcResult,
};
use harmonia_tailnet::model::{MeshMessage, MeshMessageType};

use super::fs_ops;
use super::frontend_ops;
use super::helpers::now_ms;
use super::mesh::outbound_message;
use super::shell_ops;
use super::tmux_ops;
use super::wallet_ops;

pub fn execute_request(
    node: &crate::paths::NodeIdentity,
    grants: &[String],
    request_envelope: NodeRpcRequestEnvelope,
) -> NodeRpcResponseEnvelope {
    let capability = capability_for_request(&request_envelope.body);
    if !capability_allowed(grants, capability) {
        return error_response(
            request_envelope.id,
            "permission-denied",
            format!("pairing does not grant {capability}"),
        );
    }

    match execute_request_inner(node, &request_envelope.body, grants) {
        Ok(result) => success_response(request_envelope.id, result),
        Err(err) => error_response(request_envelope.id, "execution-failed", err),
    }
}

fn execute_request_inner(
    node: &crate::paths::NodeIdentity,
    request: &NodeRpcRequest,
    _grants: &[String],
) -> Result<NodeRpcResult, String> {
    match request {
        NodeRpcRequest::Ping { nonce } => Ok(NodeRpcResult::Pong {
            nonce: nonce.clone(),
        }),
        NodeRpcRequest::Capabilities => Ok(NodeRpcResult::Capabilities {
            node_label: node.label.clone(),
            node_role: node.role.as_str().to_string(),
            capabilities: effective_capabilities(_grants),
        }),
        NodeRpcRequest::FsList {
            path,
            include_hidden,
            max_entries,
        } => fs_ops::fs_list(node, path, *include_hidden, *max_entries),
        NodeRpcRequest::FsReadText { path, max_bytes } => {
            fs_ops::fs_read_text(node, path, *max_bytes)
        }
        NodeRpcRequest::ShellExec {
            program,
            args,
            cwd,
            timeout_ms,
        } => shell_ops::shell_exec(node, program, args, cwd.as_ref(), *timeout_ms),
        NodeRpcRequest::TmuxList => tmux_ops::tmux_list(),
        NodeRpcRequest::TmuxSpawn {
            session_name,
            cwd,
            command,
            args,
        } => tmux_ops::tmux_spawn(node, session_name, cwd.as_ref(), command.as_deref(), args),
        NodeRpcRequest::TmuxCapture {
            session_name,
            history_lines,
        } => tmux_ops::tmux_capture(session_name, *history_lines),
        NodeRpcRequest::TmuxSendLine {
            session_name,
            input,
        } => tmux_ops::tmux_send_line(session_name, input),
        NodeRpcRequest::TmuxSendKey { session_name, key } => {
            tmux_ops::tmux_send_key(session_name, key)
        }
        NodeRpcRequest::WalletStatus => wallet_ops::wallet_status(),
        NodeRpcRequest::WalletListSymbols => wallet_ops::wallet_list_symbols(),
        NodeRpcRequest::WalletHasSymbol { symbol } => wallet_ops::wallet_has_symbol(symbol),
        NodeRpcRequest::WalletSetSecret { symbol, value } => {
            wallet_ops::wallet_set_secret(symbol, value)
        }
        NodeRpcRequest::FrontendPairList => frontend_ops::frontend_pair_list(),
        NodeRpcRequest::FrontendConfigure { frontend, values } => {
            frontend_ops::frontend_configure_rpc(frontend, values)
        }
        NodeRpcRequest::FrontendPairInit { frontend } => {
            frontend_ops::frontend_pair_init_rpc(frontend)
        }
        NodeRpcRequest::FrontendPairStatus { frontend } => {
            frontend_ops::frontend_pair_status_rpc(frontend)
        }
        NodeRpcRequest::DatamineQuery {
            query_id,
            lode_id,
            args,
            timeout_ms: _,
            compress,
        } => {
            let args_str = args.join(" ");
            let sexp = format!(
                "(:component \"terraphon\" :op \"datamine\" :lode-id \"{}\" :args \"{}\")",
                lode_id, args_str
            );
            Ok(NodeRpcResult::DatamineQuery {
                query_id: query_id.clone(),
                lode_id: lode_id.clone(),
                data: format!("(:pending \"cross-node datamine not yet wired: {}\")", sexp),
                compressed: *compress,
                elapsed_ms: 0,
                error: None,
            })
        }
        NodeRpcRequest::DatamineCatalog => Ok(NodeRpcResult::DatamineCatalog {
            lodes: vec!["(:pending \"catalog not yet wired\")".into()],
        }),
        NodeRpcRequest::DatamineProbe { lode_id } => Ok(NodeRpcResult::DatamineProbe {
            lode_id: lode_id.clone(),
            available: false,
        }),
        NodeRpcRequest::CrossNodeRecall {
            query_concepts,
            max_results,
            requesting_node: _,
        } => {
            // Route to local memory-field actor via IPC.
            let concepts_sexp: Vec<String> = query_concepts
                .iter()
                .map(|c| format!("\"{}\"", sexp_escape_local(c)))
                .collect();
            let ipc_cmd = format!(
                "(:component \"memory-field\" :op \"field-recall-structural\" :query-concepts ({}) :limit {})",
                concepts_sexp.join(" "),
                max_results
            );
            match super::relay_signal_to_local_agent(&ipc_cmd) {
                Ok(lines) => {
                    let reply = lines.join("");
                    let activations = parse_cross_node_activations(&reply);
                    Ok(NodeRpcResult::CrossNodeRecallResponse {
                        activations,
                        source_node: node.label.clone(),
                    })
                }
                Err(e) => Err(format!("cross-node recall IPC failed: {e}")),
            }
        }
        NodeRpcRequest::MemoryDigestRequest => {
            let ipc_cmd = "(:component \"memory-field\" :op \"digest\")";
            match super::relay_signal_to_local_agent(ipc_cmd) {
                Ok(lines) => Ok(NodeRpcResult::MemoryDigestResponse {
                    digest_sexp: lines.join(""),
                }),
                Err(e) => Err(format!("memory digest IPC failed: {e}")),
            }
        }
    }
}

// ── Helpers for cross-node memory IPC ────────────────────────────────

/// Escape a string for embedding in sexp double-quoted values.
fn sexp_escape_local(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Parse concept/score pairs from a field-recall-structural sexp reply.
/// Extracts :concept "X" :score Y patterns from the response.
fn parse_cross_node_activations(sexp: &str) -> Vec<(String, f64)> {
    sexp.split(":concept")
        .skip(1)
        .filter_map(|chunk| {
            let concept = extract_first_quoted_local(chunk)?;
            let score = extract_f64_after_key(chunk, ":score")?;
            Some((concept, score))
        })
        .collect()
}

/// Extract the first double-quoted string from a chunk.
fn extract_first_quoted_local(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let end = start + s[start..].find('"')?;
    Some(s[start..end].to_string())
}

/// Extract a f64 value after a keyword like :score 0.85.
fn extract_f64_after_key(s: &str, key: &str) -> Option<f64> {
    let pos = s.find(key)? + key.len();
    let rest = s[pos..].trim_start();
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num.parse().ok()
}

pub fn handle_command_message(
    node: &crate::paths::NodeIdentity,
    pairing: &crate::pairing::PairingRecord,
    msg: &MeshMessage,
) -> Option<MeshMessage> {
    let request: NodeRpcRequestEnvelope = match serde_json::from_str(&msg.payload) {
        Ok(request) => request,
        Err(err) => {
            let response = error_response(
                format!("invalid-{}", now_ms()),
                "invalid-request",
                format!("invalid rpc payload: {err}"),
            );
            return Some(outbound_message(
                node,
                pairing,
                MeshMessageType::Command,
                serde_json::to_string(&response).ok()?,
                msg.session.clone(),
            ));
        }
    };

    let response = execute_request(node, &pairing.grants, request);
    Some(outbound_message(
        node,
        pairing,
        MeshMessageType::Command,
        serde_json::to_string(&response).ok()?,
        msg.session.clone(),
    ))
}

fn capability_allowed(grants: &[String], capability: &str) -> bool {
    grants.is_empty() || grants.iter().any(|grant| grant == capability)
}

fn effective_capabilities(grants: &[String]) -> Vec<String> {
    if grants.is_empty() {
        return default_capabilities();
    }
    let mut caps = grants.to_vec();
    caps.sort();
    caps.dedup();
    caps
}
