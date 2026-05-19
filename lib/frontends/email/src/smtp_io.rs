//! Synchronous SMTP submission (called from the actor's send via spawn_blocking).

#[derive(Clone)]
pub(crate) struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub from: String,
    pub tls: String,
    pub default_subject: String,
}

pub(crate) fn smtp_send(cfg: &SmtpConfig, to: &str, body: &str) -> Result<(), String> {
    let email = lettre::Message::builder()
        .from(cfg.from.parse().map_err(|e| format!("from addr: {e}"))?)
        .to(to.parse().map_err(|e| format!("to addr: {e}"))?)
        .subject(cfg.default_subject.clone())
        .body(body.to_string())
        .map_err(|e| format!("build email: {e}"))?;

    let creds = lettre::transport::smtp::authentication::Credentials::new(
        cfg.user.clone(),
        cfg.password.clone(),
    );

    match cfg.tls.as_str() {
        "tls" => {
            let transport = lettre::SmtpTransport::relay(&cfg.host)
                .map_err(|e| format!("smtp relay: {e}"))?
                .port(cfg.port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
        "none" => {
            let transport = lettre::SmtpTransport::builder_dangerous(&cfg.host)
                .port(cfg.port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
        _ => {
            // "starttls" is the default
            let transport = lettre::SmtpTransport::starttls_relay(&cfg.host)
                .map_err(|e| format!("smtp starttls: {e}"))?
                .port(cfg.port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
    }
    Ok(())
}
