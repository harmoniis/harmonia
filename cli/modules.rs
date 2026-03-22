//! CLI client for `harmonia modules` — talks to the runtime via IPC.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use console::style;

/// Send a length-prefixed sexp to the runtime and read the reply.
fn ipc_rpc(socket: &std::path::Path, sexp: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;

    // Write length-prefixed sexp
    let payload = sexp.as_bytes();
    let len = (payload.len() as u32).to_be_bytes();
    stream.write_all(&len)?;
    stream.write_all(payload)?;
    stream.flush()?;

    // Read length-prefixed reply
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let reply_len = u32::from_be_bytes(len_buf) as usize;
    if reply_len > 10 * 1024 * 1024 {
        return Err("reply too large".into());
    }
    let mut reply_buf = vec![0u8; reply_len];
    stream.read_exact(&mut reply_buf)?;
    Ok(String::from_utf8_lossy(&reply_buf).to_string())
}

/// List all modules and their status.
pub fn list() -> Result<(), Box<dyn std::error::Error>> {
    let socket = runtime_socket_path()?;
    let reply = ipc_rpc(&socket, "(:modules :op \"list\")")?;

    if reply.starts_with("(:error") {
        eprintln!("{} {}", style("Error:").red().bold(), reply);
        return Ok(());
    }

    // Parse the sexp list of modules and pretty-print
    let modules = parse_module_list(&reply);

    println!();
    println!("  {}", style("Harmonia Modules").cyan().bold());
    println!(
        "  {}",
        style("────────────────────────────────────────").dim()
    );

    let mut loaded_count = 0usize;
    let mut unloaded_count = 0usize;

    for m in &modules {
        let name_col = format!("  {:<22}", m.name);
        let (status_styled, is_loaded) = match m.status.as_str() {
            "loaded" => (style("loaded").green().to_string(), true),
            "unloaded" => (style("unloaded").yellow().to_string(), false),
            s if s.starts_with("error") => (style("error").red().to_string(), false),
            s => (s.to_string(), false),
        };

        if is_loaded {
            loaded_count += 1;
        } else {
            unloaded_count += 1;
        }

        let core_str = if m.core {
            format!("  {}", style("(core)").dim())
        } else {
            String::new()
        };

        let needs_str = if !m.needs.is_empty() && !is_loaded {
            format!("  needs: {}", style(&m.needs).dim())
        } else {
            String::new()
        };

        println!("{}{:<12}{}{}", name_col, status_styled, core_str, needs_str);
    }

    println!(
        "  {}",
        style("────────────────────────────────────────").dim()
    );
    println!(
        "  {} loaded, {} unloaded",
        style(loaded_count).green().bold(),
        style(unloaded_count).yellow().bold()
    );
    println!();

    Ok(())
}

/// Load a module by name.
pub fn load(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let socket = runtime_socket_path()?;
    let sexp = format!("(:modules :op \"load\" :name \"{}\")", sexp_escape(name));
    let reply = ipc_rpc(&socket, &sexp)?;
    print_result(&reply, "load");
    Ok(())
}

/// Unload a module by name.
pub fn unload(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let socket = runtime_socket_path()?;
    let sexp = format!("(:modules :op \"unload\" :name \"{}\")", sexp_escape(name));
    let reply = ipc_rpc(&socket, &sexp)?;
    print_result(&reply, "unload");
    Ok(())
}

/// Reload a module (unload + load).
pub fn reload(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let socket = runtime_socket_path()?;
    let sexp = format!("(:modules :op \"reload\" :name \"{}\")", sexp_escape(name));
    let reply = ipc_rpc(&socket, &sexp)?;
    print_result(&reply, "reload");
    Ok(())
}

fn print_result(reply: &str, op: &str) {
    if reply.starts_with("(:ok") {
        // Extract message from (:ok "message")
        let msg = extract_quoted(reply).unwrap_or_else(|| format!("{} succeeded", op));
        eprintln!("{} {}", style("OK").green().bold(), msg);
    } else if reply.starts_with("(:error") {
        let msg = extract_quoted(reply).unwrap_or_else(|| format!("{} failed", op));
        eprintln!("{} {}", style("Error:").red().bold(), msg);
    } else {
        eprintln!("{}", reply);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn runtime_socket_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    // Match the runtime's state_root logic
    if let Ok(path) = std::env::var("HARMONIA_RUNTIME_SOCKET") {
        return Ok(std::path::PathBuf::from(path));
    }

    let default = std::env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();

    // Ensure config-store is initialized so we can read state-root
    let data_dir = crate::paths::data_dir()?;
    if std::env::var_os("HARMONIA_STATE_ROOT").is_none() {
        std::env::set_var("HARMONIA_STATE_ROOT", data_dir.to_string_lossy().as_ref());
    }
    let _ = harmonia_config_store::init_v2();

    let state_root =
        harmonia_config_store::get_config_or("harmonia-runtime", "global", "state-root", &default)
            .unwrap_or(default);

    let path = std::path::PathBuf::from(state_root).join("runtime.sock");

    if !path.exists() {
        return Err(format!(
            "runtime socket not found at {} — is harmonia running?",
            path.display()
        )
        .into());
    }

    Ok(path)
}

fn sexp_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn extract_quoted(sexp: &str) -> Option<String> {
    let start = sexp.find('"')? + 1;
    let rest = &sexp[start..];
    let mut end = 0;
    let bytes = rest.as_bytes();
    while end < bytes.len() {
        if bytes[end] == b'"' {
            return Some(rest[..end].replace("\\\"", "\"").replace("\\\\", "\\"));
        }
        if bytes[end] == b'\\' {
            end += 1;
        }
        end += 1;
    }
    None
}

struct ModuleInfo {
    name: String,
    status: String,
    core: bool,
    needs: String,
}

/// Parse a sexp list of module entries into structured data.
///
/// Format: ((:name "x" :status loaded :core t :needs "vault ...") ...)
fn parse_module_list(sexp: &str) -> Vec<ModuleInfo> {
    let mut modules = Vec::new();
    let trimmed = sexp.trim();

    // Split on "(:" to find entry boundaries
    // Each entry looks like: (:name "foo" :status loaded ...)
    let mut i = 0;
    let chars: Vec<char> = trimmed.chars().collect();

    while i < chars.len() {
        // Find start of an entry: "(:name"
        if i + 5 < chars.len() && chars[i] == '(' && chars[i + 1] == ':' && chars[i + 2] == 'n' {
            // Find matching close paren
            let start = i;
            let mut depth = 0;
            let mut end = i;
            for j in start..chars.len() {
                if chars[j] == '(' {
                    depth += 1;
                } else if chars[j] == ')' {
                    depth -= 1;
                    if depth == 0 {
                        end = j + 1;
                        break;
                    }
                }
            }
            let entry: String = chars[start..end].iter().collect();
            if let Some(m) = parse_module_entry(&entry) {
                modules.push(m);
            }
            i = end;
        } else {
            i += 1;
        }
    }

    modules
}

fn parse_module_entry(entry: &str) -> Option<ModuleInfo> {
    let name = extract_field_quoted(entry, ":name")?;
    let status = extract_field_unquoted(entry, ":status").unwrap_or_else(|| "unknown".into());
    let core = entry.contains(":core t");
    let needs = extract_field_quoted(entry, ":needs").unwrap_or_default();
    Some(ModuleInfo {
        name,
        status,
        core,
        needs,
    })
}

fn extract_field_quoted(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let inner = &after[1..];
    let bytes = inner.as_bytes();
    let mut end = 0;
    while end < bytes.len() {
        if bytes[end] == b'"' {
            return Some(inner[..end].replace("\\\"", "\"").replace("\\\\", "\\"));
        }
        if bytes[end] == b'\\' {
            end += 1;
        }
        end += 1;
    }
    None
}

fn extract_field_unquoted(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    // Handle quoted status like: :status error "msg"
    // or unquoted like: :status loaded
    let val: String = after
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ')' && *c != '"')
        .collect();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}
