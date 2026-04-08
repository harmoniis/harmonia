//! Tailnet component dispatch — pure functional, declarative.

use super::{dispatch_op, sexp_string_list};

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "start" => dispatch_op!("start",
            harmonia_tailnet::transport::start_listener().map(|_| "(:ok)".to_string())),
        "poll" => {
            let messages = harmonia_tailnet::transport::poll_messages();
            let items = messages.iter().map(|m| format!(
                "(:from \"{}\" :type \"{}\" :payload \"{}\")",
                harmonia_actor_protocol::sexp_escape(&m.from.to_string()),
                harmonia_actor_protocol::sexp_escape(&format!("{:?}", m.msg_type)),
                harmonia_actor_protocol::sexp_escape(&m.payload),
            )).collect::<Vec<_>>().join(" ");
            format!("(:ok :messages ({}))", items)
        }
        "send" => "(:ok)".to_string(), // TODO: construct MeshMessage from sexp fields
        "discover" => dispatch_op!("discover",
            harmonia_tailnet::discover_peers().map(|peers| {
                let items = peers.iter().map(|p| p.id.0.clone()).collect::<Vec<_>>();
                format!("(:ok :peers ({}))", sexp_string_list(&items))
            })),
        "stop" => { harmonia_tailnet::transport::stop_listener(); "(:ok)".to_string() }
        _ => format!("(:error \"unknown tailnet op: {}\")", op),
    }
}
