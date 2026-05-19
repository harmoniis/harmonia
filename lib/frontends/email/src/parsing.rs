//! Email body and header parsing helpers.

use mailparse::MailHeaderMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct EmailPaymentHeaders {
    pub rail: Option<String>,
    pub proof: Option<String>,
    pub action: Option<String>,
    pub challenge: Option<String>,
}

pub(crate) fn parse_payment_headers(raw: &[u8]) -> EmailPaymentHeaders {
    let Ok((headers, _)) = mailparse::parse_headers(raw) else {
        return EmailPaymentHeaders::default();
    };
    EmailPaymentHeaders {
        rail: header_value(&headers, "X-Harmoniis-Payment-Rail"),
        proof: header_value(&headers, "X-Harmoniis-Payment-Proof"),
        action: header_value(&headers, "X-Harmoniis-Payment-Action"),
        challenge: header_value(&headers, "X-Harmoniis-Payment-Challenge"),
    }
}

fn header_value(headers: &[mailparse::MailHeader<'_>], name: &str) -> Option<String> {
    headers
        .get_first_value(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn escape_metadata(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn format_email_metadata(
    sender: &str,
    headers: &EmailPaymentHeaders,
    pgp_state: PgpVerifyState,
) -> String {
    let mut metadata = vec![
        ":channel-class \"email-imap\"".to_string(),
        format!(":node-id \"{}\"", escape_metadata(sender)),
        ":remote t".to_string(),
    ];
    // The email frontend itself does not run PGP verification — that pass
    // happens in the gateway authenticator, which stamps `:auth-method` /
    // `:auth-level` / `:auth-fp` on the envelope. Here we only record
    // whether the message *looked* signed (clearsigned body or attached
    // signature part) so downstream Lisp policy can reason about the shape
    // without re-parsing the MIME tree.
    match pgp_state {
        PgpVerifyState::Unsigned => {
            metadata.push(":pgp-verified nil :pgp-trust :unsigned".to_string());
        }
        PgpVerifyState::SignedUntrusted(fp) => {
            metadata.push(format!(
                ":pgp-verified nil :pgp-trust :untrusted :pgp-fp \"{}\"",
                escape_metadata(&fp)
            ));
        }
    }
    if let Some(rail) = &headers.rail {
        metadata.push(format!(":payment-rail \"{}\"", escape_metadata(rail)));
    }
    if let Some(proof) = &headers.proof {
        metadata.push(format!(":payment-proof \"{}\"", escape_metadata(proof)));
    }
    if let Some(action) = &headers.action {
        metadata.push(format!(":payment-action \"{}\"", escape_metadata(action)));
    }
    if let Some(challenge) = &headers.challenge {
        metadata.push(format!(
            ":payment-challenge \"{}\"",
            escape_metadata(challenge)
        ));
    }
    format!("({})", metadata.join(" "))
}

#[derive(Debug, Clone)]
pub(crate) enum PgpVerifyState {
    SignedUntrusted(String),
    Unsigned,
}

pub(crate) fn parse_body_text(raw: &[u8]) -> String {
    match mailparse::parse_mail(raw) {
        Ok(parsed) => {
            for sub in &parsed.subparts {
                if let Some(ct) = sub
                    .headers
                    .iter()
                    .find(|h| h.get_key_ref() == "Content-Type")
                {
                    if ct.get_value().contains("text/plain") {
                        if let Ok(body) = sub.get_body() {
                            return body;
                        }
                    }
                }
            }
            parsed
                .get_body()
                .unwrap_or_else(|_| String::from_utf8_lossy(raw).to_string())
        }
        Err(_) => String::from_utf8_lossy(raw).to_string(),
    }
}
