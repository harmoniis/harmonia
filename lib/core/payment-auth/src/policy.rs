/// Policy parsing, evaluation, and decision functions.
use harmonia_baseband_channel_protocol::ChannelEnvelope;
use harmonia_config_store::{get_own, init as init_config_store};

use crate::model::{PaymentRequirement, PolicyDecision, COMPONENT, DEFAULT_ALLOWED_RAILS};

pub fn build_policy_query(
    envelope: &ChannelEnvelope,
    payment: &crate::model::InboundPaymentMetadata,
) -> String {
    let session_id = envelope
        .session
        .as_ref()
        .map(|value| value.id.as_str())
        .unwrap_or("");
    let origin_fp = envelope.peer.origin_fp.as_deref().unwrap_or("");
    let requested_action = payment
        .action_hint
        .as_deref()
        .unwrap_or_else(|| default_action_hint(envelope));
    format!(
        "(:frontend \"{}\" :address \"{}\" :session-id \"{}\" :origin-fp \"{}\" :requested-action \"{}\" :type-name \"{}\" :body-format \"{}\" :body-text \"{}\" :security-label \"{}\")",
        escape_sexp(&envelope.channel.kind),
        escape_sexp(&envelope.channel.address),
        escape_sexp(session_id),
        escape_sexp(origin_fp),
        escape_sexp(requested_action),
        escape_sexp(&envelope.type_name),
        escape_sexp(&envelope.body.format),
        escape_sexp(&trim_for_policy(&envelope.body.text)),
        escape_sexp(envelope.security.label.as_str()),
    )
}

pub fn parse_policy_response(raw: &str) -> Result<PolicyDecision, String> {
    let mode = sexp_symbol_value(raw, "mode").unwrap_or_else(|| "free".to_string());
    match mode.as_str() {
        "free" => Ok(PolicyDecision::Free),
        "deny" => Ok(PolicyDecision::Deny {
            code: sexp_string_value(raw, "code").unwrap_or_else(|| "payment_denied".to_string()),
            message: sexp_string_value(raw, "message")
                .unwrap_or_else(|| "Payment policy denied this action.".to_string()),
        }),
        "pay" => {
            let action = sexp_string_value(raw, "action")
                .ok_or_else(|| "payment policy response missing :action".to_string())?;
            let price = sexp_string_value(raw, "price")
                .ok_or_else(|| "payment policy response missing :price".to_string())?;
            let unit = sexp_string_value(raw, "unit")
                .ok_or_else(|| "payment policy response missing :unit".to_string())?;
            let allowed_rails = sexp_string_list(raw, "allowed-rails")
                .into_iter()
                .map(|value| value.to_ascii_lowercase())
                .filter(|value| is_supported_rail(value))
                .collect::<Vec<_>>();
            if allowed_rails.is_empty() {
                return Err("payment policy response missing supported rails".to_string());
            }
            Ok(PolicyDecision::Pay(PaymentRequirement {
                action,
                price,
                unit,
                allowed_rails,
                challenge_id: sexp_string_value(raw, "challenge-id"),
                policy_id: sexp_string_value(raw, "policy-id"),
                note: sexp_string_value(raw, "note"),
            }))
        }
        other => Err(format!("unknown payment policy mode '{other}'")),
    }
}

pub fn default_policy_response(requested_action: &str) -> PolicyDecision {
    let action = requested_action.trim().to_ascii_lowercase();
    if action.is_empty() {
        return PolicyDecision::Free;
    }
    let _ = init_config_store();
    let price_key = format!("{action}-price");
    let mode_key = format!("{action}-mode");
    let unit_key = format!("{action}-unit");
    let rails_key = format!("{action}-allowed-rails");
    let mode = get_own(COMPONENT, &mode_key)
        .ok()
        .flatten()
        .unwrap_or_else(|| "free".to_string())
        .trim()
        .to_ascii_lowercase();
    if mode == "deny" {
        return PolicyDecision::Deny {
            code: "payment_denied".to_string(),
            message: format!("Payment policy denied action '{action}'."),
        };
    }
    let Some(price) = get_own(COMPONENT, &price_key).ok().flatten() else {
        return PolicyDecision::Free;
    };
    let unit = get_own(COMPONENT, &unit_key)
        .ok()
        .flatten()
        .unwrap_or_else(|| default_unit_for_action(&action));
    let allowed_rails = get_own(COMPONENT, &rails_key)
        .ok()
        .flatten()
        .map(|value| parse_csv_rails(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            DEFAULT_ALLOWED_RAILS
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        });
    PolicyDecision::Pay(PaymentRequirement {
        action,
        price,
        unit,
        allowed_rails,
        challenge_id: None,
        policy_id: Some("config-store".to_string()),
        note: None,
    })
}

pub(crate) fn default_action_hint(envelope: &ChannelEnvelope) -> &str {
    if envelope.type_name.starts_with("payment.") {
        "paid-message"
    } else {
        "message"
    }
}

pub(crate) fn default_unit_for_action(action: &str) -> String {
    match action {
        "identity" | "post" | "comment" | "rate" => "wats".to_string(),
        _ => "wats".to_string(),
    }
}

// ── S-expression / metadata helpers ──────────────────────────────────

pub(crate) fn metadata_string_value(metadata: Option<&str>, key: &str) -> Option<String> {
    let meta = metadata?;
    let needle = format!(":{} \"", key);
    if let Some(start) = find_case_insensitive(meta, &needle) {
        let from = start + needle.len();
        let rest = &meta[from..];
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    let needle = format!(":{} ", key);
    let start = find_case_insensitive(meta, &needle)?;
    let from = start + needle.len();
    let rest = &meta[from..];
    let end = rest
        .find(|ch: char| ch.is_whitespace() || ch == ')')
        .unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub(crate) fn sexp_string_value(raw: &str, key: &str) -> Option<String> {
    metadata_string_value(Some(raw), key)
}

pub(crate) fn sexp_symbol_value(raw: &str, key: &str) -> Option<String> {
    let needle = format!(":{} :", key);
    let start = find_case_insensitive(raw, &needle)?;
    let rest = &raw[start + needle.len()..];
    let end = rest
        .find(|ch: char| ch.is_whitespace() || ch == ')')
        .unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_ascii_lowercase())
    }
}

pub(crate) fn sexp_string_list(raw: &str, key: &str) -> Vec<String> {
    let needle = format!(":{} (", key);
    let Some(start) = find_case_insensitive(raw, &needle) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let rest = &raw[start + needle.len()..];
    let end = rest.find(')').unwrap_or(rest.len());
    let mut cursor = rest[..end].trim();
    while !cursor.is_empty() {
        if let Some(stripped) = cursor.strip_prefix('"') {
            let Some(end_quote) = stripped.find('"') else {
                break;
            };
            out.push(stripped[..end_quote].to_string());
            cursor = stripped[end_quote + 1..].trim_start();
        } else {
            let split = cursor.find(char::is_whitespace).unwrap_or(cursor.len());
            let value = cursor[..split].trim();
            if !value.is_empty() {
                out.push(value.trim_matches(':').to_string());
            }
            cursor = cursor[split..].trim_start();
        }
    }
    out
}

pub(crate) fn parse_csv_rails(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| is_supported_rail(value))
        .collect()
}

pub(crate) fn is_supported_rail(value: &str) -> bool {
    matches!(value, "webcash" | "voucher" | "bitcoin")
}

pub(crate) fn escape_sexp(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn merge_metadata(base: Option<&str>, extra: &str) -> String {
    fn trim_parens(value: &str) -> &str {
        let trimmed = value.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() >= 2 {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        }
    }
    match base.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => format!("({} {})", trim_parens(value), trim_parens(extra)),
        None => extra.to_string(),
    }
}

pub(crate) fn trim_for_policy(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 256 {
        trimmed.to_string()
    } else {
        trimmed[..256].to_string()
    }
}

pub(crate) fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

pub(crate) fn payment_header_for_rail(rail: &str) -> &'static str {
    match rail {
        "voucher" => "X-Voucher-Secret",
        "bitcoin" => "X-Bitcoin-Secret",
        _ => "X-Webcash-Secret",
    }
}
