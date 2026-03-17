use std::ffi::{CStr, CString};

use crate::model::ChannelEnvelope;
use crate::registry::Registry;
use harmonia_payment_auth::{
    append_settlement_metadata, build_challenge_payload, build_denied_payload,
    build_policy_query, default_policy_response, extract_payment_metadata, parse_policy_response,
    record_challenge, settle_payment, PaymentRequirement, PolicyDecision,
};

extern "C" {
    fn free(ptr: *mut std::ffi::c_void);
}

pub fn intercept_paid_actions(
    registry: &Registry,
    envelopes: Vec<ChannelEnvelope>,
) -> Vec<ChannelEnvelope> {
    let mut forwarded = Vec::with_capacity(envelopes.len());
    for mut envelope in envelopes {
        let payment = extract_payment_metadata(&envelope);
        let requested_action = payment
            .action_hint
            .clone()
            .unwrap_or_else(|| default_action_hint(&envelope));
        let decision = query_payment_policy(&build_policy_query(&envelope, &payment), &requested_action);
        match decision {
            PolicyDecision::Free => forwarded.push(envelope),
            PolicyDecision::Deny { code, message } => {
                let payload = build_denied_payload(&code, &message);
                if let Err(err) = crate::baseband::send_signal(
                    registry,
                    &envelope.channel.kind,
                    &envelope.channel.address,
                    &payload,
                ) {
                    log::warn!("gateway: failed to send payment denial: {err}");
                }
            }
            PolicyDecision::Pay(mut requirement) => {
                if requirement.challenge_id.is_none() {
                    requirement.challenge_id = Some(default_challenge_id(&envelope, &requirement));
                }
                let challenge_id = requirement.challenge_id.clone().unwrap_or_default();
                if payment.proof.is_none() || payment.rail.is_none() {
                    if let Err(err) = record_challenge(&envelope, &requirement, &challenge_id) {
                        log::warn!("gateway: failed to record payment challenge: {err}");
                    }
                    let payload = build_challenge_payload(
                        &requirement,
                        "payment_required",
                        &format!("Payment required for {}", requirement.action),
                    );
                    if let Err(err) = crate::baseband::send_signal(
                        registry,
                        &envelope.channel.kind,
                        &envelope.channel.address,
                        &payload,
                    ) {
                        log::warn!("gateway: failed to send payment challenge: {err}");
                    }
                    continue;
                }

                match settle_payment(&envelope, &requirement, &payment) {
                    Ok(receipt) => {
                        envelope.transport.raw_metadata = Some(append_settlement_metadata(
                            envelope.transport.raw_metadata.as_deref(),
                            &receipt,
                        ));
                        forwarded.push(envelope);
                    }
                    Err(error) => {
                        let code = if error.contains("not allowed") {
                            "payment_rail_mismatch"
                        } else {
                            "invalid_payment"
                        };
                        if let Err(err) = record_challenge(&envelope, &requirement, &challenge_id) {
                            log::warn!("gateway: failed to record rejected payment: {err}");
                        }
                        let payload = build_challenge_payload(&requirement, code, &error);
                        if let Err(err) = crate::baseband::send_signal(
                            registry,
                            &envelope.channel.kind,
                            &envelope.channel.address,
                            &payload,
                        ) {
                            log::warn!("gateway: failed to send rejected payment response: {err}");
                        }
                    }
                }
            }
        }
    }
    forwarded
}

fn query_payment_policy(summary: &str, requested_action: &str) -> PolicyDecision {
    let Some(handler) = crate::state::payment_policy_query() else {
        return default_policy_response(requested_action);
    };
    let Ok(summary_c) = CString::new(summary) else {
        return default_policy_response(requested_action);
    };
    let result_ptr = unsafe { handler(summary_c.as_ptr()) };
    if result_ptr.is_null() {
        return default_policy_response(requested_action);
    }
    let raw = unsafe { CStr::from_ptr(result_ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { free(result_ptr as *mut std::ffi::c_void) };
    parse_policy_response(&raw).unwrap_or_else(|err| {
        log::warn!("gateway: invalid payment policy response '{raw}': {err}");
        default_policy_response(requested_action)
    })
}

fn default_challenge_id(envelope: &ChannelEnvelope, requirement: &PaymentRequirement) -> String {
    format!(
        "challenge-{}-{}-{}",
        requirement.action.replace(' ', "-"),
        envelope.id,
        envelope.audit.timestamp_ms
    )
}

fn default_action_hint(envelope: &ChannelEnvelope) -> String {
    if envelope.type_name.starts_with("payment.") {
        "paid-message".to_string()
    } else {
        "message".to_string()
    }
}
