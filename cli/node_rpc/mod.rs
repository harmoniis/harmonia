//! Node RPC execution — dispatches typed requests to operation handlers.

mod execute;
mod frontend_ops;
mod frontend_verify;
mod fs_ops;
pub(crate) mod helpers;
mod mesh;
mod shell_ops;
mod tmux_ops;
mod wallet_ops;

// Re-export the public surface used by other CLI modules.
pub use execute::handle_command_message;
pub use frontend_ops::{
    frontend_configure_local, frontend_pair_init_local, frontend_pair_status_local,
    list_pairable_frontends_local,
};
pub use mesh::{mesh_service_config, message_from_pairing, outbound_message, request_remote};

use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

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

#[cfg(test)]
mod tests {
    use super::*;
    use harmonia_node_rpc::{NodeRpcRequest, NodeRpcResponse, NodeRpcResult, RpcEnvelope};

    #[test]
    fn scoped_path_rejects_parent_traversal() {
        let root = std::env::temp_dir().join(format!(
            "harmonia-node-rpc-{}",
            helpers::now_ms()
        ));
        let err = helpers::resolve_relative_in_root(&root, "../secret").unwrap_err();
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
        let response = execute::execute_request(
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
