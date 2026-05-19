//! Gateway component dispatch — poll/send across all frontends.
//!
//! Frontends migrated to the trait-based actor model
//! (`harmonia_frontend_trait::Frontend`) are reached through the
//! [`crate::frontend_registry::FrontendRegistry`] passed in by the
//! `GatewayActor`. Frontends still on the legacy free-function shape
//! (`crate ::poll/send`) are reached directly. The dispatch loop returns the
//! union, so the gateway has a single uniform entry point regardless of
//! which side of the migration each frontend is on.

use super::{esc, param};
use crate::frontend_registry::FrontendRegistry;
use harmonia_frontend_trait::FrontendMsg;
use harmonia_transport_pgp::{TransportPgp, VerifyOutcome};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Circuit-breaker state for frontend polling. A frontend that fails
/// repeatedly is skipped with exponential backoff so a missing bridge
/// (e.g. whatsapp on port 3000) does not flood logs and waste poll
/// cycles. State is keyed by frontend name; recovery is instant — one
/// successful poll resets the counter to zero.
struct PollHealth {
    consecutive_failures: u32,
    next_attempt_unix_ms: u128,
}

fn poll_health() -> &'static Mutex<HashMap<String, PollHealth>> {
    static H: OnceLock<Mutex<HashMap<String, PollHealth>>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Returns true if this frontend should be skipped this tick because
/// it's in backoff. Idempotent — does not mutate state.
fn skip_due_to_backoff(name: &str) -> bool {
    let map = match poll_health().lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    map.get(name)
        .map(|h| now_unix_ms() < h.next_attempt_unix_ms)
        .unwrap_or(false)
}

/// Record an outcome for `name`. Failures grow backoff geometrically up
/// to ~5 minutes; a success collapses it to zero.
fn record_outcome(name: &str, ok: bool) {
    let mut map = match poll_health().lock() {
        Ok(m) => m,
        Err(p) => p.into_inner(),
    };
    let entry = map.entry(name.to_string()).or_insert(PollHealth {
        consecutive_failures: 0,
        next_attempt_unix_ms: 0,
    });
    if ok {
        entry.consecutive_failures = 0;
        entry.next_attempt_unix_ms = 0;
        return;
    }
    entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
    // 2^n seconds, clamped to 300s (5 min). First failure: ~1s wait.
    let exp = entry.consecutive_failures.saturating_sub(1).min(8);
    let wait_ms: u128 = (1u128 << exp) * 1_000;
    let wait_ms = wait_ms.min(300_000);
    entry.next_attempt_unix_ms = now_unix_ms() + wait_ms;
}

pub(crate) async fn dispatch(
    sexp: &str,
    registry: &FrontendRegistry,
    pgp: &TransportPgp,
) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "poll" => {
            let envelopes = poll_all_frontends(registry, pgp).await;
            format!("(:ok :envelopes ({}))", envelopes.join(" "))
        }
        "send" => {
            let (frontend, channel, payload) = (
                param!(sexp, ":frontend"),
                param!(sexp, ":channel"),
                param!(sexp, ":payload"),
            );
            let result = send_to_frontend(&frontend, &channel, &payload, registry).await;
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
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"send: {}\")", esc(&e)),
            }
        }
        "is-allowed" => "(:ok :allowed t)".to_string(),
        _ => format!("(:error \"unknown gateway op: {}\")", esc(&op)),
    }
}

async fn poll_all_frontends(registry: &FrontendRegistry, pgp: &TransportPgp) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // Trait-based frontends — actor-owned state, reached via the registry.
    // Each `InboundMessage` from MQTT/HTTP/Email goes through `authenticate`
    // before becoming a Lisp envelope: that's the single PGP-verify step
    // that stamps `:auth-method`, `:auth-level`, `:auth-fp` so the policy
    // layer downstream can decide based on uniform metadata regardless of
    // which transport delivered the signal.
    for (name, entry) in registry.entries() {
        if skip_due_to_backoff(&name) {
            continue;
        }
        match ractor::call_t!(entry.actor, FrontendMsg::Poll, 5_000) {
            Ok(Ok(messages)) => {
                record_outcome(&name, true);
                for m in messages {
                    let auth = authenticate(&name, &m.text, pgp).await;
                    out.push(make_envelope(
                        &name,
                        &m.address,
                        &auth.text,
                        entry.security_label,
                        &auth.metadata,
                    ));
                }
            }
            Ok(Err(e)) => {
                record_outcome(&name, false);
                eprintln!("[WARN] [gateway] frontend-{name} poll error: {e}");
            }
            Err(e) => {
                record_outcome(&name, false);
                eprintln!("[WARN] [gateway] frontend-{name} poll rpc error: {e}");
            }
        }
    }

    // Legacy free-function frontends — still on `OnceLock<RwLock<…>>` until
    // they migrate to the `Frontend` trait. As each one moves over the
    // corresponding entry below disappears, until this whole block is gone.
    let legacy_messaging: &[(&str, fn() -> Result<Vec<(String, String, Option<String>)>, String>)] = &[];
    for (name, poll_fn) in legacy_messaging {
        if registry.contains(name) {
            continue;
        }
        out.extend(collect_legacy(name, poll_fn(), "authenticated"));
    }

    out.extend(collect_legacy(
        "tailscale",
        harmonia_tailscale_frontend::bridge::poll(),
        "authenticated",
    ));

    out
}

fn collect_legacy(
    kind: &str,
    result: Result<Vec<(String, String, Option<String>)>, String>,
    label: &str,
) -> Vec<String> {
    result
        .unwrap_or_default()
        .into_iter()
        .map(|(address, payload, _metadata)| {
            // Legacy free-function frontends (currently just `tailscale`)
            // pre-date the gateway-side authenticator. They're treated as
            // unsigned plain transport — same `:auth-method` stamp policy
            // can read.
            let auth = AuthDecision {
                text: payload,
                metadata: format!(
                    ":auth-method :{kind}-plain :auth-level :unsigned"
                ),
            };
            make_envelope(kind, &address, &auth.text, label, &auth.metadata)
        })
        .collect()
}

fn make_envelope(
    kind: &str,
    address: &str,
    payload: &str,
    label: &str,
    auth: &str,
) -> String {
    format!(
        "(:channel (:kind \"{}\" :address \"{}\") :body (:text \"{}\") :peer (:device-id \"{}\") :security (:label :{} {auth}) :capabilities (:text t))",
        esc(kind),
        esc(address),
        esc(payload),
        esc(address),
        label,
    )
}

/// Result of the gateway's per-signal authentication pass: the (possibly
/// re-extracted) text plus the metadata fragment to embed in the
/// envelope's `:security` plist.
struct AuthDecision {
    text: String,
    metadata: String,
}

/// Run PGP verification over the inbound payload across all signed
/// channels (MQTT, HTTP, Email). Plain transports (TUI, SIP) have no
/// signature; they get `:auth-method :<kind>-plain :auth-level
/// :unsigned`. Signed transports try the JSON-envelope shape
/// `{payload, signature}`; if it parses and the signature verifies
/// against the trust-store the signal is unwrapped and stamped
/// `:auth-level :verified`. Verification *errors* (parse failures,
/// crypto rejections) drop the signal entirely so untrusted bodies
/// never reach the orchestrator.
async fn authenticate(kind: &str, raw: &str, pgp: &TransportPgp) -> AuthDecision {
    let signed_channel = matches!(kind, "mqtt" | "http3" | "email");
    if !signed_channel {
        return AuthDecision {
            text: raw.to_string(),
            metadata: format!(":auth-method :{kind}-plain :auth-level :unsigned"),
        };
    }
    // Parse `{payload, signature}` envelope. Plain text bodies (no JSON
    // envelope) are accepted as `:unsigned`.
    let envelope: Option<SignedEnvelope> = serde_json::from_str(raw).ok();
    let Some(env) = envelope else {
        return AuthDecision {
            text: raw.to_string(),
            metadata: format!(":auth-method :{kind}-mtls :auth-level :unsigned"),
        };
    };
    let signature = match env.signature {
        Some(s) if !s.is_empty() => s,
        _ => {
            return AuthDecision {
                text: env.payload,
                metadata: format!(":auth-method :{kind}-mtls :auth-level :unsigned"),
            }
        }
    };
    let outcome = pgp.verify_detached(env.payload.as_bytes().to_vec(), signature).await;
    match outcome {
        VerifyOutcome::Verified { fingerprint } => AuthDecision {
            text: env.payload,
            metadata: format!(
                ":auth-method :{kind}-pgp :auth-level :verified :auth-fp \"{}\"",
                esc(&fingerprint)
            ),
        },
        VerifyOutcome::SignedUntrusted { fingerprint } => AuthDecision {
            text: env.payload,
            metadata: format!(
                ":auth-method :{kind}-pgp :auth-level :untrusted :auth-fp \"{}\"",
                esc(&fingerprint)
            ),
        },
        VerifyOutcome::Unsigned => AuthDecision {
            text: env.payload,
            metadata: format!(":auth-method :{kind}-mtls :auth-level :unsigned"),
        },
        VerifyOutcome::Error(e) => {
            // Refuse to forward signals whose signature parse / verify
            // errored — that's the difference between "untrusted" and
            // "malformed and discarded".
            eprintln!("[WARN] [gateway] {kind} signature error: {e}");
            AuthDecision {
                text: String::new(),
                metadata: format!(":auth-method :{kind}-pgp :auth-level :rejected"),
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct SignedEnvelope {
    payload: String,
    #[serde(default)]
    signature: Option<String>,
}

async fn send_to_frontend(
    frontend: &str,
    channel: &str,
    payload: &str,
    registry: &FrontendRegistry,
) -> Result<(), String> {
    if let Some(entry) = registry.get(frontend) {
        return match ractor::call_t!(
            entry.actor,
            FrontendMsg::Send,
            5_000,
            channel.to_string(),
            payload.to_string()
        ) {
            Ok(r) => r,
            Err(e) => Err(format!("frontend-{frontend} rpc error: {e}")),
        };
    }
    match frontend {
        "tailscale" => harmonia_tailscale_frontend::bridge::send(channel, payload),
        _ => Err(format!("unknown frontend: {frontend}")),
    }
}
