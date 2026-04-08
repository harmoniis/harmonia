//! Interactive pairing menu and QR code flow.

use crate::menus::{interactive_select, MenuAction, MenuItem};
use harmonia_node_rpc::PairableFrontend;
use std::io::Stdout;

use super::catalog::{frontend_catalog, prompt_frontend_values};
use super::discovery::{configure_frontend, frontend_pair_status, list_pairable, pair_frontend};
use super::PairingTarget;
use super::{BOLD, BOLD_CYAN, DIM, GREEN, RED, RESET, YELLOW};

const QR_FRONTENDS: &[&str] = &["whatsapp", "signal"];

fn wait_for_key() {
    eprintln!("  {DIM}Press any key to continue...{RESET}\n");
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::event::read();
    let _ = crossterm::terminal::disable_raw_mode();
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

pub fn run_pairing_menu(
    stdout: &mut Stdout,
    node: &crate::paths::NodeIdentity,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = super::discovery::detect_target(node)?;
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
            run_qr_pair_flow(stdout, target, frontend.name.as_str(), &frontend.display, qr_data, &instructions)?;
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
            run_qr_pair_flow(stdout, target, frontend.name.as_str(), &frontend.display, qr_data, &instructions)?;
        } else {
            let (_, instructions) = pair_frontend(target, frontend.name.as_str())?;
            print_instructions(&instructions, true);
            wait_for_key();
        }
        return Ok(());
    }

    eprintln!(
        "\n  {GREEN}✓{RESET} {BOLD}{}{RESET} — {GREEN}{}{RESET} on {}\n",
        frontend.display, frontend.status, target.node_label(),
    );
    wait_for_key();
    Ok(())
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
            for line in qr.lines() { eprintln!("  {line}"); }
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
            eprintln!("  {GREEN}✓{RESET} {BOLD}{display_name}{RESET} linked: {GREEN}{message}{RESET}\n");
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
