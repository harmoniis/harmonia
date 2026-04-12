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
        "send" => {
            let to = harmonia_actor_protocol::extract_sexp_string(sexp, ":to").unwrap_or_default();
            let from = harmonia_actor_protocol::extract_sexp_string(sexp, ":from")
                .unwrap_or_else(|| "self".to_string());
            let payload = harmonia_actor_protocol::extract_sexp_string(sexp, ":payload").unwrap_or_default();
            let msg_type_str = harmonia_actor_protocol::extract_sexp_string(sexp, ":type").unwrap_or_default();
            let msg_type = match msg_type_str.as_str() {
                "heartbeat" => harmonia_tailnet::model::MeshMessageType::Heartbeat,
                "discovery" => harmonia_tailnet::model::MeshMessageType::Discovery,
                "command"   => harmonia_tailnet::model::MeshMessageType::Command,
                _           => harmonia_tailnet::model::MeshMessageType::Signal,
            };
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let message = harmonia_tailnet::model::MeshMessage {
                from,
                to: to.clone(),
                payload,
                msg_type,
                origin: None,
                session: None,
                timestamp_ms: now_ms,
                hmac: String::new(),
            };
            dispatch_op!("send",
                harmonia_tailnet::transport::send_message(&to, &message).map(|_| "(:ok)".to_string()))
        }
        "discover" => dispatch_op!("discover",
            harmonia_tailnet::discover_peers().map(|peers| {
                let items = peers.iter().map(|p| p.id.0.clone()).collect::<Vec<_>>();
                format!("(:ok :peers ({}))", sexp_string_list(&items))
            })),
        "stop" => { harmonia_tailnet::transport::stop_listener(); "(:ok)".to_string() }
        _ => format!("(:error \"unknown tailnet op: {}\")", op),
    }
}
