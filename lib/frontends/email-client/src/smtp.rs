use crate::state::{state, COMPONENT};

pub fn send(to: &str, text: &str) -> Result<(), String> {
    let (from, smtp_host, smtp_port, smtp_user, smtp_password, smtp_tls, subject) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("email not initialized".into());
        }
        let subject = harmonia_config_store::get_own(COMPONENT, "default-subject")
            .ok()
            .flatten()
            .unwrap_or_else(|| "Harmonia message".to_string());
        (
            s.smtp_from.clone(),
            s.smtp_host.clone(),
            s.smtp_port,
            s.smtp_user.clone(),
            s.smtp_password.clone(),
            s.smtp_tls.clone(),
            subject,
        )
    };

    let email = lettre::Message::builder()
        .from(from.parse().map_err(|e| format!("from addr: {e}"))?)
        .to(to.parse().map_err(|e| format!("to addr: {e}"))?)
        .subject(subject)
        .body(text.to_string())
        .map_err(|e| format!("build email: {e}"))?;

    let creds = lettre::transport::smtp::authentication::Credentials::new(
        smtp_user.clone(),
        smtp_password.clone(),
    );

    match smtp_tls.as_str() {
        "tls" => {
            let transport = lettre::SmtpTransport::relay(&smtp_host)
                .map_err(|e| format!("smtp relay: {e}"))?
                .port(smtp_port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
        "none" => {
            let transport = lettre::SmtpTransport::builder_dangerous(&smtp_host)
                .port(smtp_port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
        _ => {
            // "starttls" (default)
            let transport = lettre::SmtpTransport::starttls_relay(&smtp_host)
                .map_err(|e| format!("smtp starttls: {e}"))?
                .port(smtp_port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
    }

    Ok(())
}
