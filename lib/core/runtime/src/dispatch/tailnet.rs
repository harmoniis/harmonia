//! Tailnet component dispatch.

use harmonia_actor_protocol::{extract_sexp_string, sexp_escape};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "start" => match harmonia_tailnet::transport::start_listener() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "poll" => {
            let messages = harmonia_tailnet::transport::poll_messages();
            if messages.is_empty() {
                "(:ok :messages ())".to_string()
            } else {
                let items: Vec<String> = messages
                    .iter()
                    .map(|m| {
                        format!(
                            "(:from \"{}\" :type \"{}\" :payload \"{}\")",
                            esc(&m.from.to_string()),
                            esc(&format!("{:?}", m.msg_type)),
                            esc(&m.payload)
                        )
                    })
                    .collect();
                format!("(:ok :messages ({}))", items.join(" "))
            }
        }
        "send" => {
            let _to = extract_sexp_string(sexp, ":to").unwrap_or_default();
            let _payload = extract_sexp_string(sexp, ":payload").unwrap_or_default();
            // TODO: construct MeshMessage from sexp fields
            "(:ok)".to_string()
        }
        "discover" => match harmonia_tailnet::discover_peers() {
            Ok(peers) => {
                let items: Vec<String> = peers
                    .iter()
                    .map(|p| format!("\"{}\"", esc(&p.id.0)))
                    .collect();
                format!("(:ok :peers ({}))", items.join(" "))
            }
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "stop" => {
            harmonia_tailnet::transport::stop_listener();
            "(:ok)".to_string()
        }
        _ => format!("(:error \"unknown tailnet op: {}\")", op),
    }
}
