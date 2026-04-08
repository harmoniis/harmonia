//! Gateway component dispatch — poll/send across all frontends.

use super::{dispatch_op, param};
use harmonia_actor_protocol::sexp_escape;
use serde_json::json;

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "poll" => {
            let envelopes = poll_all_frontends();
            format!("(:ok :envelopes ({}))", envelopes.join(" "))
        }
        "send" => {
            let (frontend, channel, payload) =
                (param!(sexp, ":frontend"), param!(sexp, ":channel"), param!(sexp, ":payload"));
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
            dispatch_op!("send", result.map(|_| "(:ok)".to_string()))
        }
        "is-allowed" => "(:ok :allowed t)".to_string(),
        _ => format!("(:error \"unknown gateway op: {}\")", op),
    }
}

/// Poll ALL initialized frontends and return sexp envelopes.
fn poll_all_frontends() -> Vec<String> {
    let tui_envelopes = harmonia_tui::terminal::poll()
        .into_iter()
        .map(|(address, payload)| make_envelope("tui", &address, &payload, "owner"));

    let messaging_envelopes = [
        ("telegram", harmonia_telegram::bot::poll()),
        ("slack", harmonia_slack::client::poll()),
        ("discord", harmonia_discord::client::poll()),
        ("signal", harmonia_signal::client::poll()),
        ("mattermost", harmonia_mattermost::client::poll()),
        ("whatsapp", harmonia_whatsapp::client::poll()),
        ("nostr", harmonia_nostr::client::poll()),
        ("email", harmonia_email_client::client::poll()),
    ].into_iter().flat_map(|(kind, result)| {
        collect_frontend(kind, result, "authenticated")
    });

    #[cfg(target_os = "macos")]
    let platform_envelopes = collect_frontend(
        "imessage",
        harmonia_imessage::client::poll(),
        "authenticated",
    );
    #[cfg(not(target_os = "macos"))]
    let platform_envelopes = Vec::new();

    let tailscale_envelopes = collect_frontend(
        "tailscale",
        harmonia_tailscale_frontend::bridge::poll(),
        "authenticated",
    );

    tui_envelopes
        .chain(messaging_envelopes)
        .chain(platform_envelopes)
        .chain(tailscale_envelopes)
        .collect()
}

fn collect_frontend(
    kind: &str,
    result: Result<Vec<(String, String, Option<String>)>, String>,
    label: &str,
) -> Vec<String> {
    result
        .unwrap_or_default()
        .into_iter()
        .map(|(address, payload, _metadata)| make_envelope(kind, &address, &payload, label))
        .collect()
}

fn make_envelope(kind: &str, address: &str, payload: &str, label: &str) -> String {
    format!(
        "(:channel (:kind \"{}\" :address \"{}\") :body (:text \"{}\") :peer (:device-id \"{}\") :security (:label :{}) :capabilities (:text t))",
        sexp_escape(kind),
        sexp_escape(address),
        sexp_escape(payload),
        sexp_escape(address),
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
