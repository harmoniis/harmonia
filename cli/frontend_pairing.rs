//! Unified frontend setup and pairing flow for the interactive Harmonia CLI.

use crate::menus::{interactive_select, MenuAction, MenuItem};
use dialoguer::{Input, Password};
use harmonia_node_rpc::{
    FrontendConfigEntry, NodeRpcRequest, NodeRpcResponse, NodeRpcResult, PairableFrontend,
};
use std::io::Stdout;

const BOLD_CYAN: &str = "\x1b[1;36m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

const QR_FRONTENDS: &[&str] = &["whatsapp", "signal"];

pub enum PairingTarget {
    Local {
        node_label: String,
    },
    Remote {
        node: crate::paths::NodeIdentity,
        pairing: crate::pairing::PairingRecord,
    },
}

impl PairingTarget {
    fn node_label(&self) -> &str {
        match self {
            PairingTarget::Local { node_label } => node_label,
            PairingTarget::Remote { pairing, .. } => &pairing.remote_label,
        }
    }
}

struct FrontendField {
    key: &'static str,
    prompt: &'static str,
    default: Option<&'static str>,
    secret: bool,
    optional: bool,
}

struct FrontendCatalogEntry {
    name: &'static str,
    display: &'static str,
    fields: &'static [FrontendField],
}

const TELEGRAM_FIELDS: &[FrontendField] = &[FrontendField {
    key: "bot-token",
    prompt: "Telegram bot token",
    default: None,
    secret: true,
    optional: false,
}];

const SLACK_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "bot-token",
        prompt: "Slack bot token",
        default: None,
        secret: true,
        optional: false,
    },
    FrontendField {
        key: "app-token",
        prompt: "Slack app token",
        default: None,
        secret: true,
        optional: false,
    },
    FrontendField {
        key: "channels",
        prompt: "Slack channel IDs (comma-separated)",
        default: None,
        secret: false,
        optional: false,
    },
];

const DISCORD_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "bot-token",
        prompt: "Discord bot token",
        default: None,
        secret: true,
        optional: false,
    },
    FrontendField {
        key: "channels",
        prompt: "Discord channel IDs (comma-separated)",
        default: None,
        secret: false,
        optional: false,
    },
];

const MATTERMOST_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "api-url",
        prompt: "Mattermost API URL",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "bot-token",
        prompt: "Mattermost bot token",
        default: None,
        secret: true,
        optional: false,
    },
    FrontendField {
        key: "channels",
        prompt: "Mattermost channel IDs (comma-separated)",
        default: None,
        secret: false,
        optional: false,
    },
];

const WHATSAPP_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "api-url",
        prompt: "WhatsApp bridge URL",
        default: Some("http://127.0.0.1:3000"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "api-key",
        prompt: "WhatsApp bridge API key",
        default: None,
        secret: true,
        optional: true,
    },
];

const SIGNAL_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "rpc-url",
        prompt: "Signal bridge URL",
        default: Some("http://127.0.0.1:8080"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "auth-token",
        prompt: "Signal bridge auth token",
        default: None,
        secret: true,
        optional: true,
    },
];

#[cfg(target_os = "macos")]
const IMESSAGE_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "server-url",
        prompt: "BlueBubbles server URL",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "password",
        prompt: "BlueBubbles password",
        default: None,
        secret: true,
        optional: true,
    },
];

const EMAIL_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "imap-host",
        prompt: "IMAP host",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "imap-port",
        prompt: "IMAP port",
        default: Some("993"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "imap-user",
        prompt: "IMAP username",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "imap-password",
        prompt: "IMAP password",
        default: None,
        secret: true,
        optional: false,
    },
    FrontendField {
        key: "imap-mailbox",
        prompt: "IMAP mailbox",
        default: Some("INBOX"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "imap-tls",
        prompt: "IMAP TLS (true/false)",
        default: Some("true"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "smtp-host",
        prompt: "SMTP host",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "smtp-port",
        prompt: "SMTP port",
        default: Some("587"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "smtp-user",
        prompt: "SMTP username",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "smtp-password",
        prompt: "SMTP password (Enter to reuse IMAP password)",
        default: None,
        secret: true,
        optional: true,
    },
    FrontendField {
        key: "smtp-from",
        prompt: "SMTP from address",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "smtp-tls",
        prompt: "SMTP TLS mode (starttls/tls/none)",
        default: Some("starttls"),
        secret: false,
        optional: false,
    },
];

const NOSTR_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "private-key",
        prompt: "Nostr private key",
        default: None,
        secret: true,
        optional: false,
    },
    FrontendField {
        key: "relays",
        prompt: "Nostr relays (comma-separated)",
        default: Some("wss://relay.damus.io,wss://relay.primal.net,wss://nos.lol"),
        secret: false,
        optional: true,
    },
];

const TAILSCALE_FIELDS: &[FrontendField] = &[FrontendField {
    key: "auth-key",
    prompt: "Tailscale auth key",
    default: None,
    secret: true,
    optional: false,
}];

const HTTP2_FIELDS: &[FrontendField] = &[
    FrontendField {
        key: "bind",
        prompt: "HTTP/2 bind address",
        default: Some("127.0.0.1:9443"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "ca-cert",
        prompt: "Client CA certificate path",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "server-cert",
        prompt: "Server certificate path",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "server-key",
        prompt: "Server private key path",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "trusted-client-fingerprints",
        prompt: "Trusted client identity fingerprints (comma-separated)",
        default: None,
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "max-concurrent-streams",
        prompt: "Max concurrent streams",
        default: Some("64"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "session-idle-timeout-ms",
        prompt: "Session idle timeout (ms)",
        default: Some("300000"),
        secret: false,
        optional: false,
    },
    FrontendField {
        key: "max-frame-bytes",
        prompt: "Max frame bytes",
        default: Some("65536"),
        secret: false,
        optional: false,
    },
];

fn frontend_catalog() -> Vec<FrontendCatalogEntry> {
    let mut entries = vec![
        FrontendCatalogEntry {
            name: "telegram",
            display: "Telegram",
            fields: TELEGRAM_FIELDS,
        },
        FrontendCatalogEntry {
            name: "slack",
            display: "Slack",
            fields: SLACK_FIELDS,
        },
        FrontendCatalogEntry {
            name: "discord",
            display: "Discord",
            fields: DISCORD_FIELDS,
        },
        FrontendCatalogEntry {
            name: "mattermost",
            display: "Mattermost",
            fields: MATTERMOST_FIELDS,
        },
        FrontendCatalogEntry {
            name: "whatsapp",
            display: "WhatsApp",
            fields: WHATSAPP_FIELDS,
        },
        FrontendCatalogEntry {
            name: "signal",
            display: "Signal",
            fields: SIGNAL_FIELDS,
        },
        FrontendCatalogEntry {
            name: "email",
            display: "Email",
            fields: EMAIL_FIELDS,
        },
        FrontendCatalogEntry {
            name: "nostr",
            display: "Nostr",
            fields: NOSTR_FIELDS,
        },
        FrontendCatalogEntry {
            name: "tailscale",
            display: "Tailscale",
            fields: TAILSCALE_FIELDS,
        },
        FrontendCatalogEntry {
            name: "http2",
            display: "HTTP/2 mTLS",
            fields: HTTP2_FIELDS,
        },
    ];
    #[cfg(target_os = "macos")]
    entries.push(FrontendCatalogEntry {
        name: "imessage",
        display: "iMessage",
        fields: IMESSAGE_FIELDS,
    });
    entries
}

pub fn detect_target(
    node: &crate::paths::NodeIdentity,
) -> Result<PairingTarget, Box<dyn std::error::Error>> {
    match node.role {
        crate::paths::NodeRole::TuiClient | crate::paths::NodeRole::MqttClient => {
            let pairing = crate::pairing::load_default_pairing(node)?
                .ok_or("no pairing found — pair with an agent node first")?;
            Ok(PairingTarget::Remote {
                node: node.clone(),
                pairing,
            })
        }
        crate::paths::NodeRole::Agent => Ok(PairingTarget::Local {
            node_label: node.label.clone(),
        }),
    }
}

fn wait_for_key() {
    eprintln!("  {DIM}Press any key to continue...{RESET}\n");
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::event::read();
    let _ = crossterm::terminal::disable_raw_mode();
}

pub fn run_pairing_menu(
    stdout: &mut Stdout,
    node: &crate::paths::NodeIdentity,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = detect_target(node)?;
    let node_label = target.node_label().to_string();

    loop {
        let frontends = list_pairable(&target)?;
        if frontends.is_empty() {
            eprintln!("  {DIM}No frontends available on {node_label}.{RESET}\n");
            return Ok(());
        }

        let items: Vec<MenuItem> = frontends
            .iter()
            .map(|fe| {
                let status_indicator = if fe.pairable {
                    format!("{YELLOW}● {}{RESET}", fe.status)
                } else if matches!(
                    fe.status.as_str(),
                    "connected" | "device linked" | "configured" | "key configured"
                ) {
                    format!("{GREEN}● {}{RESET}", fe.status)
                } else {
                    format!("{DIM}○ {}{RESET}", fe.status)
                };
                MenuItem::new(
                    &fe.display,
                    &format!("frontend:{}", fe.name),
                    &status_indicator,
                )
            })
            .collect();

        let title = format!("Frontends — {node_label}");
        match interactive_select(stdout, &title, &items)? {
            MenuAction::Command(cmd) | MenuAction::SubMenu(cmd) => {
                if let Some(frontend_name) = cmd.strip_prefix("frontend:") {
                    if let Some(fe) = frontends.iter().find(|entry| entry.name == frontend_name) {
                        manage_frontend(stdout, &target, fe)?;
                    }
                }
            }
            MenuAction::Back | MenuAction::Cancel => break,
        }
    }

    Ok(())
}

fn manage_frontend(
    stdout: &mut Stdout,
    target: &PairingTarget,
    frontend: &PairableFrontend,
) -> Result<(), Box<dyn std::error::Error>> {
    let catalog = frontend_catalog();
    let catalog_entry = catalog.iter().find(|entry| entry.name == frontend.name);

    if frontend.status == "not configured" {
        if frontend.name == "mqtt" {
            let (_, instructions) = configure_frontend(target, frontend.name.as_str(), vec![])?;
            print_instructions(&instructions, true);
            wait_for_key();
            return Ok(());
        }
        let entry = catalog_entry
            .ok_or_else(|| format!("missing frontend catalog for {}", frontend.name))?;
        let values = prompt_frontend_values(entry)?;
        let (qr_data, instructions) = configure_frontend(target, &frontend.name, values)?;
        if QR_FRONTENDS.contains(&frontend.name.as_str()) {
            run_qr_pair_flow(
                stdout,
                target,
                frontend.name.as_str(),
                &frontend.display,
                qr_data,
                &instructions,
            )?;
        } else {
            print_instructions(&instructions, true);
            wait_for_key();
        }
        return Ok(());
    }

    if !frontend.pairable && frontend.name != "mqtt" {
        if let Some(entry) = catalog_entry {
            let values = prompt_frontend_values(entry)?;
            let (_, instructions) = configure_frontend(target, &frontend.name, values)?;
            print_instructions(&instructions, true);
            wait_for_key();
            return Ok(());
        }
    }

    if frontend.pairable {
        if QR_FRONTENDS.contains(&frontend.name.as_str()) {
            let (qr_data, instructions) = pair_frontend(target, frontend.name.as_str())?;
            run_qr_pair_flow(
                stdout,
                target,
                frontend.name.as_str(),
                &frontend.display,
                qr_data,
                &instructions,
            )?;
        } else {
            let (_, instructions) = pair_frontend(target, frontend.name.as_str())?;
            print_instructions(&instructions, true);
            wait_for_key();
        }
        return Ok(());
    }

    eprintln!(
        "\n  {GREEN}✓{RESET} {BOLD}{}{RESET} — {GREEN}{}{RESET} on {}\n",
        frontend.display,
        frontend.status,
        target.node_label(),
    );
    wait_for_key();
    Ok(())
}

fn prompt_frontend_values(
    frontend: &FrontendCatalogEntry,
) -> Result<Vec<FrontendConfigEntry>, Box<dyn std::error::Error>> {
    eprintln!(
        "\n  {BOLD_CYAN}◆{RESET} {BOLD}Configure {}{RESET}\n",
        frontend.display
    );
    let mut values = Vec::new();
    for field in frontend.fields {
        let value = if field.secret {
            Password::new()
                .with_prompt(field.prompt)
                .allow_empty_password(field.optional || field.default.is_some())
                .interact()?
        } else {
            let mut input = Input::<String>::new().with_prompt(field.prompt);
            if let Some(default) = field.default {
                input = input.default(default.to_string());
            }
            if field.optional {
                input = input.allow_empty(true);
            }
            input.interact_text()?
        };
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() && field.optional {
            continue;
        }
        if trimmed.is_empty() && field.default.is_none() && !field.optional {
            return Err(format!("{} is required", field.prompt).into());
        }
        values.push(FrontendConfigEntry {
            key: field.key.to_string(),
            value: trimmed,
            secret: field.secret,
        });
    }
    Ok(values)
}

fn list_pairable(
    target: &PairingTarget,
) -> Result<Vec<PairableFrontend>, Box<dyn std::error::Error>> {
    match target {
        PairingTarget::Local { .. } => Ok(crate::node_rpc::list_pairable_frontends_local()),
        PairingTarget::Remote { node, pairing } => {
            let response = crate::node_rpc::request_remote(
                node,
                pairing,
                NodeRpcRequest::FrontendPairList,
                10_000,
            )?;
            match response.body {
                NodeRpcResponse::Success {
                    result: NodeRpcResult::FrontendPairList { frontends },
                } => Ok(frontends),
                NodeRpcResponse::Error { message, .. } => Err(message.into()),
                _ => Err("unexpected response".into()),
            }
        }
    }
}

fn configure_frontend(
    target: &PairingTarget,
    frontend_name: &str,
    values: Vec<FrontendConfigEntry>,
) -> Result<(Option<String>, String), Box<dyn std::error::Error>> {
    match target {
        PairingTarget::Local { .. } => Ok(crate::node_rpc::frontend_configure_local(
            frontend_name,
            &values,
        )?),
        PairingTarget::Remote { node, pairing } => {
            let response = crate::node_rpc::request_remote(
                node,
                pairing,
                NodeRpcRequest::FrontendConfigure {
                    frontend: frontend_name.to_string(),
                    values,
                },
                15_000,
            )?;
            match response.body {
                NodeRpcResponse::Success {
                    result:
                        NodeRpcResult::FrontendConfigure {
                            qr_data,
                            instructions,
                            ..
                        },
                } => Ok((qr_data, instructions)),
                NodeRpcResponse::Error { message, .. } => Err(message.into()),
                _ => Err("unexpected response".into()),
            }
        }
    }
}

fn pair_frontend(
    target: &PairingTarget,
    frontend_name: &str,
) -> Result<(Option<String>, String), Box<dyn std::error::Error>> {
    match target {
        PairingTarget::Local { .. } => {
            Ok(crate::node_rpc::frontend_pair_init_local(frontend_name)?)
        }
        PairingTarget::Remote { node, pairing } => {
            let response = crate::node_rpc::request_remote(
                node,
                pairing,
                NodeRpcRequest::FrontendPairInit {
                    frontend: frontend_name.to_string(),
                },
                15_000,
            )?;
            match response.body {
                NodeRpcResponse::Success {
                    result:
                        NodeRpcResult::FrontendPairInit {
                            qr_data,
                            instructions,
                            ..
                        },
                } => Ok((qr_data, instructions)),
                NodeRpcResponse::Error { message, .. } => Err(message.into()),
                _ => Err("unexpected response".into()),
            }
        }
    }
}

fn print_instructions(instructions: &str, success: bool) {
    for line in instructions.lines() {
        if success {
            eprintln!("  {GREEN}✓{RESET} {line}");
        } else {
            eprintln!("  {YELLOW}!{RESET} {line}");
        }
    }
    eprintln!();
}

fn run_qr_pair_flow(
    _stdout: &mut Stdout,
    target: &PairingTarget,
    frontend_name: &str,
    display_name: &str,
    qr_data: Option<String>,
    instructions: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!(
        "\n  {BOLD_CYAN}◆{RESET} {BOLD}Pairing {display_name}{RESET} on {BOLD}{}{RESET}...\n",
        target.node_label()
    );
    for line in instructions.lines() {
        eprintln!("  {DIM}{line}{RESET}");
    }
    eprintln!();

    let Some(data) = qr_data else {
        eprintln!("  {RED}No QR code data received.{RESET}\n");
        wait_for_key();
        return Ok(());
    };

    match harmonia_qr_terminal::render_qr_to_string(&data) {
        Ok(qr) => {
            for line in qr.lines() {
                eprintln!("  {line}");
            }
            eprintln!();
        }
        Err(e) => {
            eprintln!("  {RED}QR render error: {e}{RESET}");
            eprintln!("  {BOLD}Raw pairing data:{RESET}");
            eprintln!("  {data}\n");
        }
    }

    eprintln!("  {DIM}Scan the QR code. No additional input is required.{RESET}");
    eprintln!("  {DIM}Waiting for device link...{RESET}\n");

    for attempt in 0..60 {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let (paired, message) = frontend_pair_status(target, frontend_name)?;
        if paired {
            eprintln!(
                "  {GREEN}✓{RESET} {BOLD}{display_name}{RESET} linked: {GREEN}{message}{RESET}\n"
            );
            return Ok(());
        }
        if attempt > 0 && attempt % 10 == 0 {
            eprintln!("  {DIM}Still waiting... ({message}){RESET}");
        }
    }

    eprintln!("  {YELLOW}Timed out waiting for device link. Retry from Frontends later.{RESET}\n");
    wait_for_key();
    Ok(())
}

fn frontend_pair_status(
    target: &PairingTarget,
    frontend_name: &str,
) -> Result<(bool, String), Box<dyn std::error::Error>> {
    match target {
        PairingTarget::Local { .. } => {
            Ok(crate::node_rpc::frontend_pair_status_local(frontend_name)?)
        }
        PairingTarget::Remote { node, pairing } => {
            let response = crate::node_rpc::request_remote(
                node,
                pairing,
                NodeRpcRequest::FrontendPairStatus {
                    frontend: frontend_name.to_string(),
                },
                5_000,
            )?;
            match response.body {
                NodeRpcResponse::Success {
                    result:
                        NodeRpcResult::FrontendPairStatus {
                            paired, message, ..
                        },
                } => Ok((paired, message)),
                NodeRpcResponse::Error { message, .. } => Err(message.into()),
                _ => Err("unexpected response".into()),
            }
        }
    }
}
