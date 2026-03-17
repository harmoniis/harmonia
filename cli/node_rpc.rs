use harmonia_node_rpc::{
    capability_for_request, default_capabilities, error_response, success_response, NodeFsEntry,
    NodePathRef, NodePathScope, NodeRpcRequest, NodeRpcRequestEnvelope, NodeRpcResponseEnvelope,
    NodeRpcResult, RpcEnvelope,
};
use harmonia_tailnet::mesh;
use harmonia_tailnet::model::{MeshMessage, MeshMessageType, MeshOrigin, MeshSession};
use harmonia_tailnet::transport;
use std::fs;
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub fn mesh_service_config(node: &crate::paths::NodeIdentity) -> String {
    format!(
        "(tailnet-config (id \"{}\") (label \"{}\") (role \"{}\") (port {}) (frontends \"tailscale\") (tools \"session\" \"node-rpc\" \"fs\" \"shell\" \"tmux\" \"wallet\"))",
        crate::pairing::advertised_addr(node),
        node.label,
        node.role.as_str(),
        crate::pairing::tailnet_port()
    )
}

pub fn channel_class_for_node(node: &crate::paths::NodeIdentity) -> &'static str {
    match node.role {
        crate::paths::NodeRole::Agent => "tailscale-agent",
        crate::paths::NodeRole::TuiClient | crate::paths::NodeRole::MqttClient => {
            "tailscale-client"
        }
    }
}

pub fn message_from_pairing(pairing: &crate::pairing::PairingRecord, msg: &MeshMessage) -> bool {
    if msg.from == pairing.remote_addr {
        return true;
    }
    if let Some(origin) = &msg.origin {
        if origin.node_id == pairing.remote_addr {
            return true;
        }
        if origin
            .node_label
            .as_deref()
            .map(|label| label == pairing.remote_label)
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

pub fn outbound_message(
    node: &crate::paths::NodeIdentity,
    pairing: &crate::pairing::PairingRecord,
    msg_type: MeshMessageType,
    payload: String,
    session: Option<MeshSession>,
) -> MeshMessage {
    let node_addr = crate::pairing::advertised_addr(node);
    MeshMessage {
        from: node_addr.clone(),
        to: pairing.remote_addr.clone(),
        payload,
        msg_type,
        origin: Some(MeshOrigin {
            node_id: node_addr,
            node_label: Some(node.label.clone()),
            node_role: Some(node.role.as_str().to_string()),
            channel_class: Some(channel_class_for_node(node).to_string()),
            node_key_id: None,
            transport_security: None,
        }),
        session,
        timestamp_ms: now_ms(),
        hmac: String::new(),
    }
}

pub fn request_remote(
    node: &crate::paths::NodeIdentity,
    pairing: &crate::pairing::PairingRecord,
    request: NodeRpcRequest,
    timeout_ms: u64,
) -> Result<NodeRpcResponseEnvelope, Box<dyn std::error::Error>> {
    mesh::init(&mesh_service_config(node)).map_err(|e| format!("tailnet mesh init failed: {e}"))?;
    transport::start_listener().map_err(|e| format!("tailnet listener failed: {e}"))?;

    let request_id = format!("rpc-{}", now_ms());
    let payload = serde_json::to_string(&RpcEnvelope::new(request_id.clone(), request))?;
    let message = outbound_message(node, pairing, MeshMessageType::Command, payload, None);
    transport::send_message(&pairing.remote_addr, &message)
        .map_err(|e| format!("send remote rpc failed: {e}"))?;

    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1));
    loop {
        for msg in transport::poll_messages() {
            if msg.msg_type != MeshMessageType::Command || !message_from_pairing(pairing, &msg) {
                continue;
            }
            let response: NodeRpcResponseEnvelope = match serde_json::from_str(&msg.payload) {
                Ok(response) => response,
                Err(_) => continue,
            };
            if response.id == request_id {
                transport::stop_listener();
                return Ok(response);
            }
        }
        if Instant::now() >= deadline {
            transport::stop_listener();
            return Err("timed out waiting for remote rpc response".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub fn execute_request(
    node: &crate::paths::NodeIdentity,
    grants: &[String],
    request_envelope: NodeRpcRequestEnvelope,
) -> NodeRpcResponseEnvelope {
    let capability = capability_for_request(&request_envelope.body);
    if !capability_allowed(grants, capability) {
        return error_response(
            request_envelope.id,
            "permission-denied",
            format!("pairing does not grant {capability}"),
        );
    }

    match execute_request_inner(node, &request_envelope.body, grants) {
        Ok(result) => success_response(request_envelope.id, result),
        Err(err) => error_response(request_envelope.id, "execution-failed", err),
    }
}

fn execute_request_inner(
    node: &crate::paths::NodeIdentity,
    request: &NodeRpcRequest,
    grants: &[String],
) -> Result<NodeRpcResult, String> {
    match request {
        NodeRpcRequest::Ping { nonce } => Ok(NodeRpcResult::Pong {
            nonce: nonce.clone(),
        }),
        NodeRpcRequest::Capabilities => Ok(NodeRpcResult::Capabilities {
            node_label: node.label.clone(),
            node_role: node.role.as_str().to_string(),
            capabilities: effective_capabilities(grants),
        }),
        NodeRpcRequest::FsList {
            path,
            include_hidden,
            max_entries,
        } => {
            let dir = resolve_path(node, path)?;
            let mut entries = Vec::new();
            let mut dir_entries: Vec<_> = fs::read_dir(&dir)
                .map_err(|e| format!("read_dir {} failed: {e}", dir.display()))?
                .filter_map(Result::ok)
                .collect();
            dir_entries.sort_by_key(|entry| entry.file_name());
            for entry in dir_entries.into_iter().take((*max_entries).max(1) as usize) {
                let name = entry.file_name().to_string_lossy().to_string();
                if !include_hidden && name.starts_with('.') {
                    continue;
                }
                let metadata = entry
                    .metadata()
                    .map_err(|e| format!("metadata {} failed: {e}", name))?;
                entries.push(NodeFsEntry {
                    name: name.clone(),
                    path: entry.path().display().to_string(),
                    is_dir: metadata.is_dir(),
                    size_bytes: metadata.len(),
                });
            }
            Ok(NodeRpcResult::FsList { entries })
        }
        NodeRpcRequest::FsReadText { path, max_bytes } => {
            let full = resolve_path(node, path)?;
            let max_bytes = (*max_bytes).clamp(1, 256 * 1024) as usize;
            let bytes =
                fs::read(&full).map_err(|e| format!("read {} failed: {e}", full.display()))?;
            let truncated = bytes.len() > max_bytes;
            let slice = if truncated {
                &bytes[..max_bytes]
            } else {
                &bytes[..]
            };
            Ok(NodeRpcResult::FsReadText {
                path: full.display().to_string(),
                text: String::from_utf8_lossy(slice).into_owned(),
                truncated,
            })
        }
        NodeRpcRequest::ShellExec {
            program,
            args,
            cwd,
            timeout_ms,
        } => {
            let cwd = match cwd {
                Some(path) => resolve_path(node, path)?,
                None => default_exec_cwd(node)?,
            };
            let (status, stdout, stderr, timed_out) =
                run_command(program, args, &cwd, *timeout_ms)?;
            Ok(NodeRpcResult::ShellExec {
                status,
                stdout,
                stderr,
                timed_out,
            })
        }
        NodeRpcRequest::TmuxList => Ok(NodeRpcResult::TmuxList {
            sessions: tmux_list_sessions()?,
        }),
        NodeRpcRequest::TmuxSpawn {
            session_name,
            cwd,
            command,
            args,
        } => {
            let cwd = match cwd {
                Some(path) => resolve_path(node, path)?,
                None => default_exec_cwd(node)?,
            };
            tmux_spawn_session(session_name, &cwd, command.as_deref(), args)?;
            Ok(NodeRpcResult::TmuxSpawn {
                session_name: session_name.clone(),
            })
        }
        NodeRpcRequest::TmuxCapture {
            session_name,
            history_lines,
        } => Ok(NodeRpcResult::TmuxCapture {
            session_name: session_name.clone(),
            output: tmux_capture(session_name, *history_lines)?,
        }),
        NodeRpcRequest::TmuxSendLine {
            session_name,
            input,
        } => {
            tmux_send_line(session_name, input)?;
            Ok(NodeRpcResult::TmuxSendLine {
                session_name: session_name.clone(),
            })
        }
        NodeRpcRequest::TmuxSendKey { session_name, key } => {
            tmux_send_key(session_name, key)?;
            Ok(NodeRpcResult::TmuxSendKey {
                session_name: session_name.clone(),
                key: key.clone(),
            })
        }
        NodeRpcRequest::WalletStatus => {
            let (wallet_db, wallet_present, vault_db, vault_present, symbol_count) =
                wallet_status()?;
            Ok(NodeRpcResult::WalletStatus {
                wallet_db,
                wallet_present,
                vault_db,
                vault_present,
                symbol_count,
            })
        }
        NodeRpcRequest::WalletListSymbols => Ok(NodeRpcResult::WalletListSymbols {
            symbols: wallet_symbols()?,
        }),
        NodeRpcRequest::WalletHasSymbol { symbol } => Ok(NodeRpcResult::WalletHasSymbol {
            symbol: symbol.clone(),
            present: wallet_has_symbol(symbol)?,
        }),
        NodeRpcRequest::WalletSetSecret { symbol, value } => {
            wallet_set_secret(symbol, value)?;
            Ok(NodeRpcResult::WalletSetSecret {
                symbol: symbol.clone(),
            })
        }
        NodeRpcRequest::FrontendPairList => {
            let frontends = list_pairable_frontends();
            Ok(NodeRpcResult::FrontendPairList { frontends })
        }
        NodeRpcRequest::FrontendConfigure { frontend, values } => {
            let (qr_data, instructions) = frontend_configure(frontend, values)?;
            Ok(NodeRpcResult::FrontendConfigure {
                frontend: frontend.clone(),
                qr_data,
                instructions,
            })
        }
        NodeRpcRequest::FrontendPairInit { frontend } => {
            let (qr_data, instructions) = frontend_pair_init(frontend)?;
            Ok(NodeRpcResult::FrontendPairInit {
                frontend: frontend.clone(),
                qr_data,
                instructions,
            })
        }
        NodeRpcRequest::FrontendPairStatus { frontend } => {
            let (paired, message) = frontend_pair_status(frontend)?;
            Ok(NodeRpcResult::FrontendPairStatus {
                frontend: frontend.clone(),
                paired,
                message,
            })
        }
    }
}

pub fn handle_command_message(
    node: &crate::paths::NodeIdentity,
    pairing: &crate::pairing::PairingRecord,
    msg: &MeshMessage,
) -> Option<MeshMessage> {
    let request: NodeRpcRequestEnvelope = match serde_json::from_str(&msg.payload) {
        Ok(request) => request,
        Err(err) => {
            let response = error_response(
                format!("invalid-{}", now_ms()),
                "invalid-request",
                format!("invalid rpc payload: {err}"),
            );
            return Some(outbound_message(
                node,
                pairing,
                MeshMessageType::Command,
                serde_json::to_string(&response).ok()?,
                msg.session.clone(),
            ));
        }
    };

    let response = execute_request(node, &pairing.grants, request);
    Some(outbound_message(
        node,
        pairing,
        MeshMessageType::Command,
        serde_json::to_string(&response).ok()?,
        msg.session.clone(),
    ))
}

#[cfg(unix)]
pub fn relay_signal_to_local_agent(
    payload: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let socket_path = crate::paths::socket_path()?;
    let mut stream = UnixStream::connect(&socket_path)
        .map_err(|e| format!("connect {}: {e}", socket_path.display()))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(300)))
        .map_err(|e| format!("set read timeout failed: {e}"))?;
    stream.write_all(payload.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let reader = BufReader::new(stream);
    let mut lines = Vec::new();
    for line in reader.lines() {
        match line {
            Ok(line) => lines.push(line),
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(err) => return Err(format!("read local agent response failed: {err}").into()),
        }
    }
    Ok(lines)
}

#[cfg(not(unix))]
pub fn relay_signal_to_local_agent(
    _payload: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Err("local agent relay requires Unix domain sockets on this platform".into())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn capability_allowed(grants: &[String], capability: &str) -> bool {
    grants.is_empty() || grants.iter().any(|grant| grant == capability)
}

fn effective_capabilities(grants: &[String]) -> Vec<String> {
    if grants.is_empty() {
        return default_capabilities();
    }
    let mut caps = grants.to_vec();
    caps.sort();
    caps.dedup();
    caps
}

fn resolve_path(
    node: &crate::paths::NodeIdentity,
    reference: &NodePathRef,
) -> Result<PathBuf, String> {
    match reference.scope {
        NodePathScope::Absolute => {
            let path = PathBuf::from(&reference.path);
            if !path.is_absolute() {
                return Err("absolute scope requires an absolute path".to_string());
            }
            Ok(path)
        }
        NodePathScope::Workspace => resolve_relative_in_root(
            &crate::paths::user_workspace().map_err(|e| e.to_string())?,
            &reference.path,
        ),
        NodePathScope::Home => {
            let home =
                dirs::home_dir().ok_or_else(|| "cannot determine home directory".to_string())?;
            resolve_relative_in_root(&home, &reference.path)
        }
        NodePathScope::Data => resolve_relative_in_root(
            &crate::paths::data_dir().map_err(|e| e.to_string())?,
            &reference.path,
        ),
        NodePathScope::Node => resolve_relative_in_root(
            &crate::paths::node_dir(&node.label).map_err(|e| e.to_string())?,
            &reference.path,
        ),
    }
}

fn resolve_relative_in_root(root: &Path, raw: &str) -> Result<PathBuf, String> {
    let input = Path::new(raw);
    if input.is_absolute() {
        return Err("scoped paths must be relative".to_string());
    }
    for component in input.components() {
        if matches!(component, Component::ParentDir) {
            return Err("path traversal rejected".to_string());
        }
    }
    Ok(root.join(input))
}

fn default_exec_cwd(node: &crate::paths::NodeIdentity) -> Result<PathBuf, String> {
    crate::paths::user_workspace()
        .map_err(|_| ())
        .or_else(|_| crate::paths::node_dir(&node.label).map_err(|_| ()))
        .map_err(|_| "no default working directory available".to_string())
}

fn run_command(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout_ms: u64,
) -> Result<(Option<i32>, String, String, bool), String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {program} failed: {e}"))?;

    let deadline = Instant::now() + Duration::from_millis(timeout_ms.clamp(1, 120_000));
    let mut timed_out = false;
    loop {
        if child
            .try_wait()
            .map_err(|e| format!("wait on {program} failed: {e}"))?
            .is_some()
        {
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("collect output for {program} failed: {e}"))?;
    Ok((
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        timed_out,
    ))
}

fn tmux_run(args: &[&str]) -> Result<String, String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|e| format!("tmux exec failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err("tmux command failed".to_string());
        }
        return Err(format!("tmux error: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn tmux_list_sessions() -> Result<Vec<String>, String> {
    match tmux_run(&["list-sessions", "-F", "#{session_name}"]) {
        Ok(output) => Ok(output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect()),
        Err(err) if err.contains("no server running") || err.contains("no sessions") => Ok(vec![]),
        Err(err) => Err(err),
    }
}

fn tmux_spawn_session(
    session_name: &str,
    cwd: &Path,
    command: Option<&str>,
    args: &[String],
) -> Result<(), String> {
    let mut cmd = Command::new("tmux");
    cmd.arg("new-session")
        .arg("-d")
        .arg("-s")
        .arg(session_name)
        .arg("-c")
        .arg(cwd);
    if let Some(command) = command {
        cmd.arg(command);
        for arg in args {
            cmd.arg(arg);
        }
    }
    let output = cmd
        .output()
        .map_err(|e| format!("tmux spawn failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "tmux spawn error: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn tmux_capture(session_name: &str, history_lines: u32) -> Result<String, String> {
    tmux_run(&[
        "capture-pane",
        "-t",
        session_name,
        "-p",
        "-S",
        &format!("-{}", history_lines.max(1)),
    ])
}

fn tmux_send_line(session_name: &str, input: &str) -> Result<(), String> {
    tmux_run(&["send-keys", "-t", session_name, "-l", input])?;
    tmux_run(&["send-keys", "-t", session_name, "Enter"])?;
    Ok(())
}

fn tmux_send_key(session_name: &str, key: &str) -> Result<(), String> {
    tmux_run(&["send-keys", "-t", session_name, key])?;
    Ok(())
}

fn wallet_status() -> Result<(String, bool, String, bool, usize), String> {
    bind_vault_env()?;
    harmonia_vault::init_from_env()?;
    let wallet_db = crate::paths::wallet_db_path().map_err(|e| e.to_string())?;
    let vault_db = crate::paths::vault_db_path().map_err(|e| e.to_string())?;
    let symbols = harmonia_vault::list_secret_symbols();
    Ok((
        wallet_db.display().to_string(),
        wallet_db.exists(),
        vault_db.display().to_string(),
        vault_db.exists(),
        symbols.len(),
    ))
}

fn wallet_symbols() -> Result<Vec<String>, String> {
    bind_vault_env()?;
    harmonia_vault::init_from_env()?;
    Ok(harmonia_vault::list_secret_symbols())
}

fn wallet_has_symbol(symbol: &str) -> Result<bool, String> {
    bind_vault_env()?;
    harmonia_vault::init_from_env()?;
    Ok(harmonia_vault::has_secret_for_symbol(symbol))
}

fn wallet_set_secret(symbol: &str, value: &str) -> Result<(), String> {
    bind_vault_env()?;
    harmonia_vault::init_from_env()?;
    harmonia_vault::set_secret_for_symbol(symbol, value)
}

pub fn list_pairable_frontends_local() -> Vec<harmonia_node_rpc::PairableFrontend> {
    list_pairable_frontends()
}

pub fn frontend_pair_init_local(frontend: &str) -> Result<(Option<String>, String), String> {
    frontend_pair_init(frontend)
}

pub fn frontend_pair_status_local(frontend: &str) -> Result<(bool, String), String> {
    frontend_pair_status(frontend)
}

pub fn frontend_configure_local(
    frontend: &str,
    values: &[harmonia_node_rpc::FrontendConfigEntry],
) -> Result<(Option<String>, String), String> {
    frontend_configure(frontend, values)
}

// ---------------------------------------------------------------------------
// Frontend pairing — unified config resolution helpers
// ---------------------------------------------------------------------------

fn config_has(component: &str, key: &str) -> bool {
    harmonia_config_store::get_own(component, key)
        .ok()
        .flatten()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

fn vault_has(component: &str, symbols: &[&str]) -> bool {
    for sym in symbols {
        if let Ok(Some(v)) = harmonia_vault::get_secret_for_component(component, sym) {
            if !v.trim().is_empty() {
                return true;
            }
        }
    }
    false
}

fn vault_get(component: &str, symbols: &[&str]) -> Option<String> {
    for sym in symbols {
        if let Ok(Some(v)) = harmonia_vault::get_secret_for_component(component, sym) {
            let trimmed = v.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

fn config_get(component: &str, key: &str) -> Option<String> {
    harmonia_config_store::get_own(component, key)
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
}

fn config_set(component: &str, key: &str, value: &str) -> Result<(), String> {
    harmonia_config_store::set_config(component, component, key, value)
}

fn set_secret_if_present(value: Option<&str>, symbols: &[&str]) -> Result<(), String> {
    if let Some(value) = value {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            let symbol = symbols.first().ok_or("missing vault symbol")?;
            harmonia_vault::set_secret_for_symbol(symbol, trimmed)?;
        }
    }
    Ok(())
}

fn value_for<'a>(
    values: &'a [harmonia_node_rpc::FrontendConfigEntry],
    key: &str,
) -> Option<&'a str> {
    values
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.value.as_str())
}

fn default_if_empty(value: Option<&str>, default: &'static str) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn frontend_configure(
    frontend: &str,
    values: &[harmonia_node_rpc::FrontendConfigEntry],
) -> Result<(Option<String>, String), String> {
    bind_vault_env()?;
    let _ = harmonia_vault::init_from_env();

    match frontend {
        "telegram" => {
            set_secret_if_present(value_for(values, "bot-token"), &["telegram-bot-token"])?;
            let ok = verify_telegram_token()?;
            if !ok {
                return Err("Telegram bot token is invalid".to_string());
            }
            Ok((None, "Telegram bot token verified. Frontend ready.".to_string()))
        }
        "slack" => {
            set_secret_if_present(value_for(values, "bot-token"), &["slack-bot-token"])?;
            set_secret_if_present(value_for(values, "app-token"), &["slack-app-token"])?;
            let channels = value_for(values, "channels").unwrap_or("").trim();
            if channels.is_empty() {
                return Err("Slack requires at least one channel ID".to_string());
            }
            config_set("slack-frontend", "channels", channels)?;
            let ok = verify_slack_token()?;
            if !ok {
                return Err("Slack token verification failed".to_string());
            }
            Ok((None, format!("Slack verified. Saved channels: {channels}")))
        }
        "discord" => {
            set_secret_if_present(value_for(values, "bot-token"), &["discord-bot-token"])?;
            let channels = value_for(values, "channels").unwrap_or("").trim();
            if channels.is_empty() {
                return Err("Discord requires at least one channel ID".to_string());
            }
            config_set("discord-frontend", "channels", channels)?;
            let ok = verify_discord_token()?;
            if !ok {
                return Err("Discord token verification failed".to_string());
            }
            Ok((None, format!("Discord verified. Saved channels: {channels}")))
        }
        "mattermost" => {
            let api_url = value_for(values, "api-url").unwrap_or("").trim();
            let channels = value_for(values, "channels").unwrap_or("").trim();
            if api_url.is_empty() {
                return Err("Mattermost requires api-url".to_string());
            }
            if channels.is_empty() {
                return Err("Mattermost requires at least one channel ID".to_string());
            }
            config_set("mattermost-frontend", "api-url", api_url)?;
            config_set("mattermost-frontend", "channels", channels)?;
            set_secret_if_present(
                value_for(values, "bot-token"),
                &["mattermost-bot-token"],
            )?;
            let ok = verify_mattermost_token()?;
            if !ok {
                return Err("Mattermost auth failed".to_string());
            }
            Ok((None, format!("Mattermost verified. Saved channels: {channels}")))
        }
        "email" => {
            let required = [
                "imap-host",
                "imap-port",
                "imap-user",
                "imap-mailbox",
                "imap-tls",
                "smtp-host",
                "smtp-port",
                "smtp-user",
                "smtp-from",
                "smtp-tls",
            ];
            for key in required {
                let value = value_for(values, key).unwrap_or("").trim();
                if value.is_empty() {
                    return Err(format!("Email requires {key}"));
                }
                config_set("email-frontend", key, value)?;
            }
            set_secret_if_present(
                value_for(values, "imap-password"),
                &["email-imap-password"],
            )?;
            set_secret_if_present(
                value_for(values, "smtp-password"),
                &["email-smtp-password"],
            )?;
            if !vault_has(
                "email-frontend",
                &["email-imap-password", "email-password", "email-smtp-password"],
            ) {
                return Err("Email requires an IMAP password".to_string());
            }
            Ok((None, "Email settings saved.".to_string()))
        }
        "nostr" => {
            let key = value_for(values, "private-key").unwrap_or("").trim();
            if key.is_empty() {
                return Err("Nostr requires a private key".to_string());
            }
            harmonia_vault::set_secret_for_symbol("nostr-private-key", key)?;
            let relays = default_if_empty(
                value_for(values, "relays"),
                "wss://relay.damus.io,wss://relay.primal.net,wss://nos.lol",
            );
            config_set("nostr-frontend", "relays", &relays)?;
            Ok((None, format!("Nostr configured with relays: {relays}")))
        }
        "whatsapp" => {
            let api_url = default_if_empty(value_for(values, "api-url"), "http://127.0.0.1:3000");
            config_set("whatsapp-frontend", "api-url", &api_url)?;
            set_secret_if_present(value_for(values, "api-key"), &["whatsapp-api-key"])?;
            let (qr_data, instructions) = frontend_pair_init(frontend)?;
            Ok((qr_data, instructions))
        }
        "signal" => {
            let rpc_url = default_if_empty(value_for(values, "rpc-url"), "http://127.0.0.1:8080");
            config_set("signal-frontend", "rpc-url", &rpc_url)?;
            set_secret_if_present(value_for(values, "auth-token"), &["signal-auth-token"])?;
            let (qr_data, instructions) = frontend_pair_init(frontend)?;
            Ok((qr_data, instructions))
        }
        #[cfg(target_os = "macos")]
        "imessage" => {
            let server_url = value_for(values, "server-url").unwrap_or("").trim();
            if server_url.is_empty() {
                return Err("iMessage requires server-url".to_string());
            }
            config_set("imessage-frontend", "server-url", server_url)?;
            set_secret_if_present(
                value_for(values, "password"),
                &["bluebubbles-password"],
            )?;
            let ok = verify_imessage_bridge()?;
            if !ok {
                return Err("BlueBubbles bridge is not responding".to_string());
            }
            Ok((None, "BlueBubbles bridge verified. Frontend ready.".to_string()))
        }
        "tailscale" => {
            set_secret_if_present(value_for(values, "auth-key"), &["tailscale-auth-key"])?;
            Ok((None, "Tailscale auth key saved.".to_string()))
        }
        "http2" => {
            let bind = default_if_empty(value_for(values, "bind"), "127.0.0.1:9443");
            bind.parse::<std::net::SocketAddr>()
                .map_err(|e| format!("invalid bind address: {e}"))?;
            let ca_cert = value_for(values, "ca-cert").unwrap_or("").trim();
            let server_cert = value_for(values, "server-cert").unwrap_or("").trim();
            let server_key = value_for(values, "server-key").unwrap_or("").trim();
            for (label, path) in [
                ("ca-cert", ca_cert),
                ("server-cert", server_cert),
                ("server-key", server_key),
            ] {
                if path.is_empty() {
                    return Err(format!("HTTP/2 requires {label}"));
                }
                if !Path::new(path).exists() {
                    return Err(format!("HTTP/2 {label} path does not exist: {path}"));
                }
            }
            let trusted_csv = value_for(values, "trusted-client-fingerprints")
                .unwrap_or("")
                .trim()
                .to_string();
            if trusted_csv.is_empty() {
                return Err("HTTP/2 requires at least one trusted client fingerprint".to_string());
            }
            let trusted: Vec<String> = trusted_csv
                .split(',')
                .map(harmonia_transport_auth::normalize_fingerprint)
                .filter(|value| !value.is_empty())
                .collect();
            if trusted.is_empty() {
                return Err("HTTP/2 requires at least one valid trusted client fingerprint".to_string());
            }
            config_set("http2-frontend", "bind", &bind)?;
            config_set("http2-frontend", "ca-cert", ca_cert)?;
            config_set("http2-frontend", "server-cert", server_cert)?;
            config_set("http2-frontend", "server-key", server_key)?;
            config_set(
                "http2-frontend",
                "trusted-client-fingerprints-json",
                &serde_json::to_string(&trusted).map_err(|e| e.to_string())?,
            )?;
            for key in [
                "max-concurrent-streams",
                "session-idle-timeout-ms",
                "max-frame-bytes",
            ] {
                let value = value_for(values, key).unwrap_or("").trim();
                if value.is_empty() {
                    continue;
                }
                config_set("http2-frontend", key, value)?;
            }
            Ok((
                None,
                format!(
                    "HTTP/2 mTLS configured on {bind}. Trusted client identities: {}",
                    trusted.join(", ")
                ),
            ))
        }
        "mqtt" => Ok((None, "MQTT is managed automatically by Harmonia.".to_string())),
        _ => Err(format!("frontend '{frontend}' does not support configuration")),
    }
}

/// Verify an HTTP endpoint returns 2xx.
fn http_ok(url: &str, bearer: Option<&str>) -> Result<bool, String> {
    let req = ureq::get(url);
    let req = match bearer {
        Some(token) => req.set("Authorization", &format!("Bearer {token}")),
        None => req,
    };
    match req.call() {
        Ok(_) => Ok(true),
        Err(ureq::Error::Status(code, _)) => Ok(code < 500),
        Err(e) => Err(format!("{e}")),
    }
}

/// Verify a Bot-prefixed auth header (Discord).
fn http_ok_bot(url: &str, token: &str) -> Result<bool, String> {
    let req = ureq::get(url)
        .set("Authorization", &format!("Bot {token}"))
        .set("User-Agent", "harmonia-discord/0.1.0");
    match req.call() {
        Ok(_) => Ok(true),
        Err(ureq::Error::Status(401, _)) => Ok(false),
        Err(ureq::Error::Status(code, _)) => Ok(code < 500),
        Err(e) => Err(format!("{e}")),
    }
}

// ---------------------------------------------------------------------------
// Frontend pairing — list / init / status
// ---------------------------------------------------------------------------

fn list_pairable_frontends() -> Vec<harmonia_node_rpc::PairableFrontend> {
    let _ = harmonia_vault::init_from_env();
    let mut frontends = Vec::new();

    // MQTT — local transport configured by Harmonia.
    {
        let configured = config_has("mqtt-frontend", "broker");
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "mqtt".to_string(),
            display: "MQTT".to_string(),
            status: if configured {
                "configured".to_string()
            } else {
                "not configured".to_string()
            },
            pairable: false,
        });
    }

    // WhatsApp — QR code device linking via bridge
    {
        let configured = config_has("whatsapp-frontend", "api-url");
        let (status, pairable) = if configured {
            match harmonia_whatsapp::client::pair_status() {
                Ok((true, _)) => ("connected".to_string(), false),
                Ok((false, msg)) => (msg, true),
                Err(_) => ("bridge unreachable".to_string(), true),
            }
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "whatsapp".to_string(),
            display: "WhatsApp".to_string(),
            status,
            pairable,
        });
    }

    // Signal — QR code device linking via signal-cli bridge
    {
        let configured =
            config_has("signal-frontend", "rpc-url") || config_has("signal-frontend", "account");
        let (status, pairable) = if configured {
            match harmonia_signal::client::pair_status() {
                Ok((true, _)) => ("device linked".to_string(), false),
                Ok((false, msg)) => (msg, true),
                Err(_) => ("bridge unreachable".to_string(), true),
            }
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "signal".to_string(),
            display: "Signal".to_string(),
            status,
            pairable,
        });
    }

    // Telegram — bot token verification
    {
        let has_token = vault_has("telegram-frontend", &["telegram-bot-token", "telegram-bot-api-token"]);
        let (status, pairable) = if has_token {
            match verify_telegram_token() {
                Ok(true) => ("connected".to_string(), false),
                Ok(false) => ("token invalid".to_string(), true),
                Err(_) => ("api unreachable".to_string(), true),
            }
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "telegram".to_string(),
            display: "Telegram".to_string(),
            status,
            pairable,
        });
    }

    // Slack — bot token verification
    {
        let has_token = vault_has("slack-frontend", &["slack-bot-token", "slack-bot-token-v2"]);
        let (status, pairable) = if has_token {
            match verify_slack_token() {
                Ok(true) => ("connected".to_string(), false),
                Ok(false) => ("token invalid".to_string(), true),
                Err(_) => ("api unreachable".to_string(), true),
            }
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "slack".to_string(),
            display: "Slack".to_string(),
            status,
            pairable,
        });
    }

    // Discord — bot token verification
    {
        let has_token = vault_has("discord-frontend", &["discord-bot-token", "discord-token"]);
        let (status, pairable) = if has_token {
            match verify_discord_token() {
                Ok(true) => ("connected".to_string(), false),
                Ok(false) => ("token invalid".to_string(), true),
                Err(_) => ("api unreachable".to_string(), true),
            }
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "discord".to_string(),
            display: "Discord".to_string(),
            status,
            pairable,
        });
    }

    // Mattermost — server + token verification
    {
        let has_url = config_has("mattermost-frontend", "api-url");
        let has_token = vault_has("mattermost-frontend", &["mattermost-bot-token", "mattermost-token"]);
        let (status, pairable) = if has_url && has_token {
            match verify_mattermost_token() {
                Ok(true) => ("connected".to_string(), false),
                Ok(false) => ("auth failed".to_string(), true),
                Err(_) => ("server unreachable".to_string(), true),
            }
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "mattermost".to_string(),
            display: "Mattermost".to_string(),
            status,
            pairable,
        });
    }

    // Email — IMAP/SMTP configuration check
    {
        let has_host = config_has("email-frontend", "imap-host");
        let has_password = vault_has(
            "email-frontend",
            &["email-imap-password", "email-password", "email-smtp-password"],
        );
        let (status, pairable) = if has_host && has_password {
            ("configured".to_string(), false)
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "email".to_string(),
            display: "Email".to_string(),
            status,
            pairable,
        });
    }

    // iMessage — BlueBubbles bridge (macOS only)
    #[cfg(target_os = "macos")]
    {
        let has_url = config_has("imessage-frontend", "server-url");
        let (status, pairable) = if has_url {
            match verify_imessage_bridge() {
                Ok(true) => ("connected".to_string(), false),
                Ok(false) => ("bridge unreachable".to_string(), true),
                Err(_) => ("bridge unreachable".to_string(), true),
            }
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "imessage".to_string(),
            display: "iMessage".to_string(),
            status,
            pairable,
        });
    }

    // Nostr — private key check
    {
        let has_key = vault_has("nostr-frontend", &["nostr-private-key", "nostr-nsec"]);
        let (status, pairable) = if has_key {
            ("key configured".to_string(), false)
        } else {
            ("not configured".to_string(), false)
        };
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "nostr".to_string(),
            display: "Nostr".to_string(),
            status,
            pairable,
        });
    }

    // Tailscale — auth key stored in vault.
    {
        let configured = vault_has("tailscale-frontend", &["tailscale-auth-key"]);
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "tailscale".to_string(),
            display: "Tailscale".to_string(),
            status: if configured {
                "configured".to_string()
            } else {
                "not configured".to_string()
            },
            pairable: false,
        });
    }

    // HTTP/2 mTLS — local transport configured by certificate paths and trust list.
    {
        let configured = config_has("http2-frontend", "bind")
            && config_has("http2-frontend", "ca-cert")
            && config_has("http2-frontend", "server-cert")
            && config_has("http2-frontend", "server-key")
            && config_has("http2-frontend", "trusted-client-fingerprints-json");
        frontends.push(harmonia_node_rpc::PairableFrontend {
            name: "http2".to_string(),
            display: "HTTP/2 mTLS".to_string(),
            status: if configured {
                "configured".to_string()
            } else {
                "not configured".to_string()
            },
            pairable: false,
        });
    }

    frontends
}

// ---------------------------------------------------------------------------
// Token verification helpers
// ---------------------------------------------------------------------------

fn verify_telegram_token() -> Result<bool, String> {
    let token = vault_get("telegram-frontend", &["telegram-bot-token", "telegram-bot-api-token"])
        .ok_or("no token")?;
    let url = format!("https://api.telegram.org/bot{token}/getMe");
    match ureq::get(&url).call() {
        Ok(resp) => {
            let body = resp.into_string().unwrap_or_default();
            Ok(body.contains("\"ok\":true"))
        }
        Err(ureq::Error::Status(401, _)) => Ok(false),
        Err(e) => Err(format!("{e}")),
    }
}

fn verify_slack_token() -> Result<bool, String> {
    let token = vault_get("slack-frontend", &["slack-bot-token", "slack-bot-token-v2"])
        .ok_or("no token")?;
    match ureq::post("https://slack.com/api/auth.test")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string("")
    {
        Ok(resp) => {
            let body = resp.into_string().unwrap_or_default();
            Ok(body.contains("\"ok\":true"))
        }
        Err(e) => Err(format!("{e}")),
    }
}

fn verify_discord_token() -> Result<bool, String> {
    let token = vault_get("discord-frontend", &["discord-bot-token", "discord-token"])
        .ok_or("no token")?;
    http_ok_bot("https://discord.com/api/v10/users/@me", &token)
}

fn verify_mattermost_token() -> Result<bool, String> {
    let url = config_get("mattermost-frontend", "api-url").ok_or("no url")?;
    let token = vault_get("mattermost-frontend", &["mattermost-bot-token", "mattermost-token"])
        .ok_or("no token")?;
    http_ok(&format!("{url}/api/v4/users/me"), Some(&token))
}

#[cfg(target_os = "macos")]
fn verify_imessage_bridge() -> Result<bool, String> {
    let url = config_get("imessage-frontend", "server-url").ok_or("no url")?;
    let password = vault_get("imessage-frontend", &["bluebubbles-password", "imessage-password"]);
    http_ok(
        &format!("{url}/api/v1/server/info"),
        password.as_deref(),
    )
}

fn frontend_pair_init(frontend: &str) -> Result<(Option<String>, String), String> {
    let _ = harmonia_vault::init_from_env();
    match frontend {
        "whatsapp" => {
            let qr = harmonia_whatsapp::client::pair_init()?;
            Ok((
                qr,
                "Scan the QR code with WhatsApp on your phone:\nWhatsApp > Settings > Linked Devices > Link a Device".to_string(),
            ))
        }
        "signal" => {
            let uri = harmonia_signal::client::pair_init()?;
            Ok((
                uri,
                "Scan the QR code with Signal on your phone:\nSignal > Settings > Linked Devices > Link New Device".to_string(),
            ))
        }
        "telegram" => {
            match verify_telegram_token() {
                Ok(true) => Ok((None, "Telegram bot token verified. Bot is connected and receiving messages.".to_string())),
                Ok(false) => Err("Telegram bot token is invalid. Update it via `harmonia setup`.".to_string()),
                Err(e) => Err(format!("Cannot reach Telegram API: {e}")),
            }
        }
        "slack" => {
            match verify_slack_token() {
                Ok(true) => Ok((None, "Slack bot token verified. Bot is connected.".to_string())),
                Ok(false) => Err("Slack bot token is invalid. Update it via `harmonia setup`.".to_string()),
                Err(e) => Err(format!("Cannot reach Slack API: {e}")),
            }
        }
        "discord" => {
            match verify_discord_token() {
                Ok(true) => Ok((None, "Discord bot token verified. Bot is connected.".to_string())),
                Ok(false) => Err("Discord bot token is invalid. Update it via `harmonia setup`.".to_string()),
                Err(e) => Err(format!("Cannot reach Discord API: {e}")),
            }
        }
        "mattermost" => {
            match verify_mattermost_token() {
                Ok(true) => Ok((None, "Mattermost bot token verified. Bot is connected.".to_string())),
                Ok(false) => Err("Mattermost auth failed. Check api-url and bot token via `harmonia setup`.".to_string()),
                Err(e) => Err(format!("Cannot reach Mattermost server: {e}")),
            }
        }
        #[cfg(target_os = "macos")]
        "imessage" => {
            match verify_imessage_bridge() {
                Ok(true) => Ok((None, "BlueBubbles bridge is reachable. iMessage is connected.".to_string())),
                Ok(false) => Err("BlueBubbles bridge is not responding. Check server-url via `harmonia setup`.".to_string()),
                Err(e) => Err(format!("Cannot reach BlueBubbles: {e}")),
            }
        }
        _ => Err(format!("frontend '{frontend}' does not support linking")),
    }
}

fn frontend_pair_status(frontend: &str) -> Result<(bool, String), String> {
    let _ = harmonia_vault::init_from_env();
    match frontend {
        "whatsapp" => harmonia_whatsapp::client::pair_status(),
        "signal" => {
            let status = harmonia_signal::client::pair_status()?;
            if status.0 {
                let _ = discover_and_store_signal_account();
            }
            Ok(status)
        }
        "telegram" => verify_telegram_token().map(|ok| (ok, if ok { "connected" } else { "token invalid" }.to_string())),
        "slack" => verify_slack_token().map(|ok| (ok, if ok { "connected" } else { "token invalid" }.to_string())),
        "discord" => verify_discord_token().map(|ok| (ok, if ok { "connected" } else { "token invalid" }.to_string())),
        "mattermost" => verify_mattermost_token().map(|ok| (ok, if ok { "connected" } else { "auth failed" }.to_string())),
        #[cfg(target_os = "macos")]
        "imessage" => verify_imessage_bridge().map(|ok| (ok, if ok { "connected" } else { "bridge unreachable" }.to_string())),
        "email" => {
            let configured = config_has("email-frontend", "imap-host")
                && vault_has("email-frontend", &["email-imap-password", "email-password"]);
            Ok((configured, if configured { "configured" } else { "not configured" }.to_string()))
        }
        "nostr" => {
            let configured = vault_has("nostr-frontend", &["nostr-private-key", "nostr-nsec"]);
            Ok((configured, if configured { "key configured" } else { "not configured" }.to_string()))
        }
        "http2" => {
            let configured = config_has("http2-frontend", "bind")
                && config_has("http2-frontend", "ca-cert")
                && config_has("http2-frontend", "server-cert")
                && config_has("http2-frontend", "server-key")
                && config_has("http2-frontend", "trusted-client-fingerprints-json");
            Ok((
                configured,
                if configured {
                    "configured"
                } else {
                    "not configured"
                }
                .to_string(),
            ))
        }
        _ => Err(format!("unknown frontend '{frontend}'")),
    }
}

fn discover_and_store_signal_account() -> Result<(), String> {
    let rpc_url = config_get("signal-frontend", "rpc-url").unwrap_or_default();
    if rpc_url.trim().is_empty() || config_has("signal-frontend", "account") {
        return Ok(());
    }
    let auth = vault_get(
        "signal-frontend",
        &["signal-auth-token", "signal-auth-token-v2"],
    );
    let endpoints = [
        format!("{rpc_url}/v1/accounts"),
        format!("{rpc_url}/v2/accounts"),
    ];
    for endpoint in endpoints {
        let req = ureq::get(&endpoint);
        let req = match auth.as_deref() {
            Some(token) => req.set("Authorization", &format!("Bearer {token}")),
            None => req,
        };
        let response = match req.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(404, _)) => continue,
            Err(_) => continue,
        };
        let json: serde_json::Value = match response.into_json() {
            Ok(json) => json,
            Err(_) => continue,
        };
        if let Some(account) = extract_signal_account(&json) {
            let _ = config_set("signal-frontend", "account", &account);
            return Ok(());
        }
    }
    Ok(())
}

fn extract_signal_account(json: &serde_json::Value) -> Option<String> {
    if let Some(account) = json.get("number").and_then(|value| value.as_str()) {
        let trimmed = account.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(account) = json.get("account").and_then(|value| value.as_str()) {
        let trimmed = account.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(accounts) = json.as_array() {
        for entry in accounts {
            if let Some(account) = extract_signal_account(entry) {
                return Some(account);
            }
        }
    }
    if let Some(results) = json.get("accounts").and_then(|value| value.as_array()) {
        for entry in results {
            if let Some(account) = extract_signal_account(entry) {
                return Some(account);
            }
        }
    }
    None
}

fn bind_vault_env() -> Result<(), String> {
    let vault_db = crate::paths::vault_db_path().map_err(|e| e.to_string())?;
    let wallet_root = crate::paths::wallet_root_path().map_err(|e| e.to_string())?;
    let wallet_db = crate::paths::wallet_db_path().map_err(|e| e.to_string())?;
    let state_root = crate::paths::data_dir().map_err(|e| e.to_string())?;
    let _ = crate::paths::set_config_value("global", "vault-db", &vault_db.to_string_lossy());
    let _ = crate::paths::set_config_value("global", "wallet-root", &wallet_root.to_string_lossy());
    let _ = crate::paths::set_config_value("global", "wallet-db", &wallet_db.to_string_lossy());
    let _ = crate::paths::set_config_value("global", "state-root", &state_root.to_string_lossy());
    std::env::set_var("HARMONIA_VAULT_DB", vault_db.to_string_lossy().as_ref());
    std::env::set_var("HARMONIA_WALLET_ROOT", wallet_root.to_string_lossy().as_ref());
    std::env::set_var(
        "HARMONIA_VAULT_WALLET_DB",
        wallet_db.to_string_lossy().as_ref(),
    );
    std::env::set_var("HARMONIA_STATE_ROOT", state_root.to_string_lossy().as_ref());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonia_node_rpc::NodeRpcResponse;

    #[test]
    fn scoped_path_rejects_parent_traversal() {
        let root = std::env::temp_dir().join(format!("harmonia-node-rpc-{}", now_ms()));
        let err = resolve_relative_in_root(&root, "../secret").unwrap_err();
        assert!(err.contains("traversal"));
    }

    #[test]
    fn execute_ping_returns_pong() {
        let node = crate::paths::NodeIdentity {
            label: "rpc-node".to_string(),
            hostname: "rpc-node".to_string(),
            role: crate::paths::NodeRole::TuiClient,
            install_profile: crate::paths::InstallProfile::TuiClient,
        };
        let response = execute_request(
            &node,
            &[harmonia_node_rpc::capability::PING.to_string()],
            RpcEnvelope::new(
                "rpc-test",
                NodeRpcRequest::Ping {
                    nonce: Some("n1".to_string()),
                },
            ),
        );
        match response.body {
            NodeRpcResponse::Success {
                result: NodeRpcResult::Pong { nonce },
            } => assert_eq!(nonce.as_deref(), Some("n1")),
            other => panic!("unexpected response: {other:?}"),
        }
    }
}
