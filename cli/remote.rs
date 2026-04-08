use harmonia_node_rpc::{
    NodePathRef, NodePathScope, NodeRpcRequest, NodeRpcResponse, NodeRpcResult,
};

pub fn run(action: &crate::RemoteAction) -> Result<(), Box<dyn std::error::Error>> {
    let node = crate::paths::current_node_identity()?;
    let pairing = crate::pairing::load_default_pairing(&node)?
        .ok_or("no saved pairing for this node; run `harmonia pairing invite` on the peer and `harmonia` locally to pair first")?;

    let request = action_to_request(action)?;
    let response = crate::node_rpc::request_remote(&node, &pairing, request, 30_000)?;
    print_response(response.body)
}

fn action_to_request(
    action: &crate::RemoteAction,
) -> Result<NodeRpcRequest, Box<dyn std::error::Error>> {
    Ok(match action {
        crate::RemoteAction::Capabilities => NodeRpcRequest::Capabilities,
        crate::RemoteAction::Fs { action } => match action {
            crate::RemoteFsAction::List {
                path,
                hidden,
                max_entries,
            } => NodeRpcRequest::FsList {
                path: parse_path_ref(path, NodePathScope::Workspace),
                include_hidden: *hidden,
                max_entries: *max_entries,
            },
            crate::RemoteFsAction::Read { path, max_bytes } => NodeRpcRequest::FsReadText {
                path: parse_path_ref(path, NodePathScope::Workspace),
                max_bytes: *max_bytes,
            },
        },
        crate::RemoteAction::Shell {
            program,
            args,
            cwd,
            timeout_ms,
        } => NodeRpcRequest::ShellExec {
            program: program.clone(),
            args: args.clone(),
            cwd: cwd
                .as_ref()
                .map(|raw| parse_path_ref(raw, NodePathScope::Workspace)),
            timeout_ms: *timeout_ms,
        },
        crate::RemoteAction::Tmux { action } => match action {
            crate::RemoteTmuxAction::List => NodeRpcRequest::TmuxList,
            crate::RemoteTmuxAction::Spawn {
                session,
                cwd,
                command,
                args,
            } => NodeRpcRequest::TmuxSpawn {
                session_name: session.clone(),
                cwd: cwd
                    .as_ref()
                    .map(|raw| parse_path_ref(raw, NodePathScope::Workspace)),
                command: command.clone(),
                args: args.clone(),
            },
            crate::RemoteTmuxAction::Capture { session, history } => NodeRpcRequest::TmuxCapture {
                session_name: session.clone(),
                history_lines: *history,
            },
            crate::RemoteTmuxAction::Send { session, input } => NodeRpcRequest::TmuxSendLine {
                session_name: session.clone(),
                input: input.clone(),
            },
            crate::RemoteTmuxAction::Key { session, key } => NodeRpcRequest::TmuxSendKey {
                session_name: session.clone(),
                key: key.clone(),
            },
        },
        crate::RemoteAction::Wallet { action } => match action {
            crate::RemoteWalletAction::Status => NodeRpcRequest::WalletStatus,
            crate::RemoteWalletAction::Symbols => NodeRpcRequest::WalletListSymbols,
            crate::RemoteWalletAction::Has { symbol } => NodeRpcRequest::WalletHasSymbol {
                symbol: symbol.clone(),
            },
            crate::RemoteWalletAction::Set { symbol, value } => NodeRpcRequest::WalletSetSecret {
                symbol: symbol.clone(),
                value: value.clone(),
            },
        },
    })
}

fn parse_path_ref(raw: &str, default_scope: NodePathScope) -> NodePathRef {
    let trimmed = raw.trim();
    if let Some((scope, path)) = trimmed.split_once(':') {
        let scope = match scope {
            "workspace" => Some(NodePathScope::Workspace),
            "home" => Some(NodePathScope::Home),
            "data" => Some(NodePathScope::Data),
            "node" => Some(NodePathScope::Node),
            "absolute" => Some(NodePathScope::Absolute),
            _ => None,
        };
        if let Some(scope) = scope {
            return NodePathRef::new(scope, path);
        }
    }
    if trimmed.starts_with('/') {
        NodePathRef::new(NodePathScope::Absolute, trimmed)
    } else {
        NodePathRef::new(default_scope, trimmed)
    }
}

fn print_response(response: NodeRpcResponse) -> Result<(), Box<dyn std::error::Error>> {
    match response {
        NodeRpcResponse::Error { code, message } => Err(format!("{code}: {message}").into()),
        NodeRpcResponse::Success { result } => {
            match result {
                NodeRpcResult::Pong { nonce } => {
                    println!("pong {}", nonce.unwrap_or_default().trim());
                }
                NodeRpcResult::Capabilities {
                    node_label,
                    node_role,
                    capabilities,
                } => {
                    println!("remote node: {} ({})", node_label, node_role);
                    for capability in capabilities {
                        println!("  {}", capability);
                    }
                }
                NodeRpcResult::FsList { entries } => {
                    for entry in entries {
                        println!(
                            "{}\t{}\t{}",
                            if entry.is_dir { "dir " } else { "file" },
                            entry.size_bytes,
                            entry.path
                        );
                    }
                }
                NodeRpcResult::FsReadText {
                    text, truncated, ..
                } => {
                    print!("{text}");
                    if truncated {
                        eprintln!("\n[truncated]");
                    }
                }
                NodeRpcResult::ShellExec {
                    status,
                    stdout,
                    stderr,
                    timed_out,
                } => {
                    println!(
                        "status: {}{}",
                        status
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "signal".to_string()),
                        if timed_out { " (timed out)" } else { "" }
                    );
                    if !stdout.is_empty() {
                        println!("\n[stdout]");
                        print!("{stdout}");
                    }
                    if !stderr.is_empty() {
                        println!("\n[stderr]");
                        print!("{stderr}");
                    }
                }
                NodeRpcResult::TmuxList { sessions } => {
                    for session in sessions {
                        println!("{session}");
                    }
                }
                NodeRpcResult::TmuxSpawn { session_name } => {
                    println!("spawned {}", session_name);
                }
                NodeRpcResult::TmuxCapture {
                    session_name,
                    output,
                } => {
                    println!("[{}]", session_name);
                    print!("{output}");
                }
                NodeRpcResult::TmuxSendLine { session_name } => {
                    println!("sent line to {}", session_name);
                }
                NodeRpcResult::TmuxSendKey { session_name, key } => {
                    println!("sent key {} to {}", key, session_name);
                }
                NodeRpcResult::WalletStatus {
                    wallet_db,
                    wallet_present,
                    vault_db,
                    vault_present,
                    symbol_count,
                } => {
                    println!(
                        "wallet: {} ({})",
                        wallet_db,
                        if wallet_present { "present" } else { "missing" }
                    );
                    println!(
                        "vault:  {} ({})",
                        vault_db,
                        if vault_present { "present" } else { "missing" }
                    );
                    println!("symbols: {}", symbol_count);
                }
                NodeRpcResult::WalletListSymbols { symbols } => {
                    for symbol in symbols {
                        println!("{symbol}");
                    }
                }
                NodeRpcResult::WalletHasSymbol { symbol, present } => {
                    println!("{}: {}", symbol, if present { "yes" } else { "no" });
                }
                NodeRpcResult::WalletSetSecret { symbol } => {
                    println!("stored {}", symbol);
                }
                NodeRpcResult::FrontendPairList { frontends } => {
                    for fe in frontends {
                        println!(
                            "{}\t{}\t{}",
                            fe.name,
                            if fe.pairable { "pairable" } else { "---" },
                            fe.status
                        );
                    }
                }
                NodeRpcResult::FrontendConfigure {
                    frontend,
                    qr_data,
                    instructions,
                } => {
                    println!("{frontend}: {instructions}");
                    if let Some(data) = qr_data {
                        println!("{data}");
                    }
                }
                NodeRpcResult::FrontendPairInit {
                    frontend,
                    qr_data,
                    instructions,
                } => {
                    println!("{frontend}: {instructions}");
                    if let Some(data) = qr_data {
                        println!("{data}");
                    }
                }
                NodeRpcResult::FrontendPairStatus {
                    frontend,
                    paired,
                    message,
                } => {
                    println!(
                        "{}: {} ({})",
                        frontend,
                        if paired { "linked" } else { "not linked" },
                        message
                    );
                }
                // Datamining results from cross-node queries.
                NodeRpcResult::DatamineQuery {
                    lode_id, data, elapsed_ms, error, ..
                } => {
                    if let Some(err) = error {
                        println!("datamine {} error: {}", lode_id, err);
                    } else {
                        println!("datamine {} ({}ms): {}", lode_id, elapsed_ms, data);
                    }
                }
                NodeRpcResult::DatamineCatalog { lodes } => {
                    for lode in lodes {
                        println!("  {}", lode);
                    }
                }
                NodeRpcResult::DatamineProbe { lode_id, available } => {
                    println!("{}: {}", lode_id, if available { "available" } else { "unavailable" });
                }
            }
            Ok(())
        }
    }
}
