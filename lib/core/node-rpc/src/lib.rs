use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 1;

pub mod capability {
    pub const PING: &str = "node.ping";
    pub const CAPABILITIES: &str = "node.capabilities";
    pub const FS_LIST: &str = "fs.list";
    pub const FS_READ_TEXT: &str = "fs.read-text";
    pub const SHELL_EXEC: &str = "shell.exec";
    pub const TMUX_LIST: &str = "tmux.list";
    pub const TMUX_SPAWN: &str = "tmux.spawn";
    pub const TMUX_CAPTURE: &str = "tmux.capture";
    pub const TMUX_SEND_LINE: &str = "tmux.send-line";
    pub const TMUX_SEND_KEY: &str = "tmux.send-key";
    pub const WALLET_STATUS: &str = "wallet.status";
    pub const WALLET_LIST_SYMBOLS: &str = "wallet.list-symbols";
    pub const WALLET_HAS_SYMBOL: &str = "wallet.has-symbol";
    pub const WALLET_SET_SECRET: &str = "wallet.set-secret";
    pub const FRONTEND_PAIR_LIST: &str = "frontend.pair-list";
    pub const FRONTEND_CONFIGURE: &str = "frontend.configure";
    pub const FRONTEND_PAIR_INIT: &str = "frontend.pair-init";
    pub const FRONTEND_PAIR_STATUS: &str = "frontend.pair-status";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodePathScope {
    Workspace,
    Home,
    Data,
    Node,
    Absolute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodePathRef {
    pub scope: NodePathScope,
    pub path: String,
}

impl NodePathRef {
    pub fn new(scope: NodePathScope, path: impl Into<String>) -> Self {
        Self {
            scope,
            path: path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFsEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEnvelope<T> {
    pub version: u8,
    pub id: String,
    pub body: T,
}

impl<T> RpcEnvelope<T> {
    pub fn new(id: impl Into<String>, body: T) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id: id.into(),
            body,
        }
    }
}

pub type NodeRpcRequestEnvelope = RpcEnvelope<NodeRpcRequest>;
pub type NodeRpcResponseEnvelope = RpcEnvelope<NodeRpcResponse>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "kebab-case")]
pub enum NodeRpcRequest {
    Ping {
        #[serde(default)]
        nonce: Option<String>,
    },
    Capabilities,
    FsList {
        path: NodePathRef,
        #[serde(default)]
        include_hidden: bool,
        #[serde(default = "default_max_entries")]
        max_entries: u32,
    },
    FsReadText {
        path: NodePathRef,
        #[serde(default = "default_max_bytes")]
        max_bytes: u64,
    },
    ShellExec {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        cwd: Option<NodePathRef>,
        #[serde(default = "default_timeout_ms")]
        timeout_ms: u64,
    },
    TmuxList,
    TmuxSpawn {
        session_name: String,
        #[serde(default)]
        cwd: Option<NodePathRef>,
        #[serde(default)]
        command: Option<String>,
        #[serde(default)]
        args: Vec<String>,
    },
    TmuxCapture {
        session_name: String,
        #[serde(default = "default_tmux_history_lines")]
        history_lines: u32,
    },
    TmuxSendLine {
        session_name: String,
        input: String,
    },
    TmuxSendKey {
        session_name: String,
        key: String,
    },
    WalletStatus,
    WalletListSymbols,
    WalletHasSymbol {
        symbol: String,
    },
    WalletSetSecret {
        symbol: String,
        value: String,
    },
    /// List frontends that support device pairing (QR code linking).
    FrontendPairList,
    /// Persist frontend config and secrets, then return next-step instructions.
    FrontendConfigure {
        frontend: String,
        #[serde(default)]
        values: Vec<FrontendConfigEntry>,
    },
    /// Initiate device pairing for a specific frontend. Returns QR code data.
    FrontendPairInit {
        frontend: String,
    },
    /// Check the pairing status for a frontend after initiation.
    FrontendPairStatus {
        frontend: String,
    },
}

/// A frontend that supports device pairing via QR code or link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairableFrontend {
    pub name: String,
    pub display: String,
    pub status: String,
    pub pairable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfigEntry {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum NodeRpcResponse {
    Success { result: NodeRpcResult },
    Error { code: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "kebab-case")]
pub enum NodeRpcResult {
    Pong {
        #[serde(default)]
        nonce: Option<String>,
    },
    Capabilities {
        node_label: String,
        node_role: String,
        capabilities: Vec<String>,
    },
    FsList {
        entries: Vec<NodeFsEntry>,
    },
    FsReadText {
        path: String,
        text: String,
        truncated: bool,
    },
    ShellExec {
        status: Option<i32>,
        stdout: String,
        stderr: String,
        timed_out: bool,
    },
    TmuxList {
        sessions: Vec<String>,
    },
    TmuxSpawn {
        session_name: String,
    },
    TmuxCapture {
        session_name: String,
        output: String,
    },
    TmuxSendLine {
        session_name: String,
    },
    TmuxSendKey {
        session_name: String,
        key: String,
    },
    WalletStatus {
        wallet_db: String,
        wallet_present: bool,
        vault_db: String,
        vault_present: bool,
        symbol_count: usize,
    },
    WalletListSymbols {
        symbols: Vec<String>,
    },
    WalletHasSymbol {
        symbol: String,
        present: bool,
    },
    WalletSetSecret {
        symbol: String,
    },
    FrontendPairList {
        frontends: Vec<PairableFrontend>,
    },
    FrontendConfigure {
        frontend: String,
        qr_data: Option<String>,
        instructions: String,
    },
    FrontendPairInit {
        frontend: String,
        qr_data: Option<String>,
        instructions: String,
    },
    FrontendPairStatus {
        frontend: String,
        paired: bool,
        message: String,
    },
}

pub fn success_response(id: impl Into<String>, result: NodeRpcResult) -> NodeRpcResponseEnvelope {
    RpcEnvelope::new(id, NodeRpcResponse::Success { result })
}

pub fn error_response(
    id: impl Into<String>,
    code: impl Into<String>,
    message: impl Into<String>,
) -> NodeRpcResponseEnvelope {
    RpcEnvelope::new(
        id,
        NodeRpcResponse::Error {
            code: code.into(),
            message: message.into(),
        },
    )
}

pub fn default_capabilities() -> Vec<String> {
    vec![
        capability::PING.to_string(),
        capability::CAPABILITIES.to_string(),
        capability::FS_LIST.to_string(),
        capability::FS_READ_TEXT.to_string(),
        capability::SHELL_EXEC.to_string(),
        capability::TMUX_LIST.to_string(),
        capability::TMUX_SPAWN.to_string(),
        capability::TMUX_CAPTURE.to_string(),
        capability::TMUX_SEND_LINE.to_string(),
        capability::TMUX_SEND_KEY.to_string(),
        capability::WALLET_STATUS.to_string(),
        capability::WALLET_LIST_SYMBOLS.to_string(),
        capability::WALLET_HAS_SYMBOL.to_string(),
        capability::WALLET_SET_SECRET.to_string(),
        capability::FRONTEND_PAIR_LIST.to_string(),
        capability::FRONTEND_CONFIGURE.to_string(),
        capability::FRONTEND_PAIR_INIT.to_string(),
        capability::FRONTEND_PAIR_STATUS.to_string(),
    ]
}

pub fn capability_for_request(request: &NodeRpcRequest) -> &'static str {
    match request {
        NodeRpcRequest::Ping { .. } => capability::PING,
        NodeRpcRequest::Capabilities => capability::CAPABILITIES,
        NodeRpcRequest::FsList { .. } => capability::FS_LIST,
        NodeRpcRequest::FsReadText { .. } => capability::FS_READ_TEXT,
        NodeRpcRequest::ShellExec { .. } => capability::SHELL_EXEC,
        NodeRpcRequest::TmuxList => capability::TMUX_LIST,
        NodeRpcRequest::TmuxSpawn { .. } => capability::TMUX_SPAWN,
        NodeRpcRequest::TmuxCapture { .. } => capability::TMUX_CAPTURE,
        NodeRpcRequest::TmuxSendLine { .. } => capability::TMUX_SEND_LINE,
        NodeRpcRequest::TmuxSendKey { .. } => capability::TMUX_SEND_KEY,
        NodeRpcRequest::WalletStatus => capability::WALLET_STATUS,
        NodeRpcRequest::WalletListSymbols => capability::WALLET_LIST_SYMBOLS,
        NodeRpcRequest::WalletHasSymbol { .. } => capability::WALLET_HAS_SYMBOL,
        NodeRpcRequest::WalletSetSecret { .. } => capability::WALLET_SET_SECRET,
        NodeRpcRequest::FrontendPairList => capability::FRONTEND_PAIR_LIST,
        NodeRpcRequest::FrontendConfigure { .. } => capability::FRONTEND_CONFIGURE,
        NodeRpcRequest::FrontendPairInit { .. } => capability::FRONTEND_PAIR_INIT,
        NodeRpcRequest::FrontendPairStatus { .. } => capability::FRONTEND_PAIR_STATUS,
    }
}

fn default_max_entries() -> u32 {
    256
}

fn default_max_bytes() -> u64 {
    64 * 1024
}

fn default_timeout_ms() -> u64 {
    30_000
}

fn default_tmux_history_lines() -> u32 {
    200
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_capability_mapping_is_stable() {
        assert_eq!(
            capability_for_request(&NodeRpcRequest::WalletStatus),
            capability::WALLET_STATUS
        );
        assert_eq!(
            capability_for_request(&NodeRpcRequest::TmuxSendKey {
                session_name: "a".to_string(),
                key: "Enter".to_string(),
            }),
            capability::TMUX_SEND_KEY
        );
    }

    #[test]
    fn default_capabilities_cover_wallet_write() {
        let caps = default_capabilities();
        assert!(caps.contains(&capability::WALLET_SET_SECRET.to_string()));
        assert!(caps.contains(&capability::SHELL_EXEC.to_string()));
    }
}
