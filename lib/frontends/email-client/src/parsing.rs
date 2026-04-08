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

pub(crate) fn format_email_metadata(sender: &str, headers: &EmailPaymentHeaders) -> String {
    let mut metadata = vec![
        format!(":channel-class \"email-imap\""),
        format!(":node-id \"{}\"", escape_metadata(sender)),
        ":remote t".to_string(),
    ];
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

pub(crate) fn parse_body_text(raw: &[u8]) -> String {
    match mailparse::parse_mail(raw) {
        Ok(parsed) => {
            // Prefer text/plain subpart
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
            // Fall back to main body
            parsed
                .get_body()
                .unwrap_or_else(|_| String::from_utf8_lossy(raw).to_string())
        }
        Err(_) => String::from_utf8_lossy(raw).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{format_email_metadata, parse_payment_headers};

    #[test]
    fn parses_payment_headers_into_metadata_fields() {
        let raw = concat!(
            "From: payer@example.com\r\n",
            "X-Harmoniis-Payment-Rail: voucher\r\n",
            "X-Harmoniis-Payment-Proof: proof-123\r\n",
            "X-Harmoniis-Payment-Action: post\r\n",
            "X-Harmoniis-Payment-Challenge: challenge-42\r\n",
            "\r\n",
        );
        let headers = parse_payment_headers(raw.as_bytes());
        let metadata = format_email_metadata("payer@example.com", &headers);

        assert!(metadata.contains(":payment-rail \"voucher\""));
        assert!(metadata.contains(":payment-proof \"proof-123\""));
        assert!(metadata.contains(":payment-action \"post\""));
        assert!(metadata.contains(":payment-challenge \"challenge-42\""));
    }
}
