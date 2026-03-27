//! Gateway component dispatch — poll/send across all frontends.

use harmonia_actor_protocol::{extract_sexp_string, sexp_escape};
use serde_json::json;

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "poll" => {
            let envelopes = poll_all_frontends();
            format!("(:ok :envelopes ({}))", envelopes.join(" "))
        }
        "send" => {
            let frontend = extract_sexp_string(sexp, ":frontend").unwrap_or_default();
            let channel = extract_sexp_string(sexp, ":channel").unwrap_or_default();
            let payload = extract_sexp_string(sexp, ":payload").unwrap_or_default();
            let result = send_to_frontend(&frontend, &channel, &payload);
            if harmonia_observability::harmonia_observability_is_standard() {
                let obs_ref = harmonia_observability::get_obs_actor().cloned();
                use harmonia_observability::Traceable;
                obs_ref.trace_event(
                    "gateway-send",
                    "tool",
                    json!({"frontend": frontend, "channel": channel, "success": result.is_ok()}),
                );
            }
            match result {
                Ok(()) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "is-allowed" => "(:ok :allowed t)".to_string(),
        _ => format!("(:error \"unknown gateway op: {}\")", op),
    }
}

/// Poll ALL initialized frontends and return sexp envelopes.
fn poll_all_frontends() -> Vec<String> {
    let mut envelopes = Vec::new();

    // TUI — local trusted session
    for (address, payload) in harmonia_tui::terminal::poll() {
        envelopes.push(make_envelope("tui", &address, &payload, "owner"));
    }

    // Messaging frontends — each gracefully returns empty if not initialized
    poll_frontend(
        &mut envelopes,
        "telegram",
        harmonia_telegram::bot::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "slack",
        harmonia_slack::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "discord",
        harmonia_discord::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "signal",
        harmonia_signal::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "mattermost",
        harmonia_mattermost::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "whatsapp",
        harmonia_whatsapp::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "nostr",
        harmonia_nostr::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "email",
        harmonia_email_client::client::poll(),
        "authenticated",
    );

    #[cfg(target_os = "macos")]
    poll_frontend(
        &mut envelopes,
        "imessage",
        harmonia_imessage::client::poll(),
        "authenticated",
    );

    poll_frontend(
        &mut envelopes,
        "tailscale",
        harmonia_tailscale_frontend::bridge::poll(),
        "authenticated",
    );

    envelopes
}

fn poll_frontend(
    envelopes: &mut Vec<String>,
    kind: &str,
    result: Result<Vec<(String, String, Option<String>)>, String>,
    label: &str,
) {
    for (address, payload, _metadata) in result.unwrap_or_default() {
        envelopes.push(make_envelope(kind, &address, &payload, label));
    }
}

fn make_envelope(kind: &str, address: &str, payload: &str, label: &str) -> String {
    format!(
        "(:channel (:kind \"{}\" :address \"{}\") :body (:text \"{}\") :peer (:device-id \"{}\") :security (:label :{}) :capabilities (:text t))",
        esc(kind),
        esc(address),
        esc(payload),
        esc(address),
        label
    )
}

/// Route an outbound message to the correct frontend.
fn send_to_frontend(frontend: &str, channel: &str, payload: &str) -> Result<(), String> {
    match frontend {
        "tui" => {
            harmonia_tui::terminal::send(channel, payload);
            Ok(())
        }
        "telegram" => harmonia_telegram::bot::send(channel, payload),
        "slack" => harmonia_slack::client::send(channel, payload),
        "discord" => harmonia_discord::client::send(channel, payload),
        "signal" => harmonia_signal::client::send(channel, payload),
        "mattermost" => harmonia_mattermost::client::send(channel, payload),
        "whatsapp" => harmonia_whatsapp::client::send(channel, payload),
        "nostr" => harmonia_nostr::client::send(channel, payload),
        "email" | "email-client" => harmonia_email_client::client::send(channel, payload),
        #[cfg(target_os = "macos")]
        "imessage" => harmonia_imessage::client::send(channel, payload),
        "tailscale" => harmonia_tailscale_frontend::bridge::send(channel, payload),
        _ => Err(format!("unknown frontend: {frontend}")),
    }
}
