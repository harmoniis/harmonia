use serde::{Deserialize, Serialize};

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
pub struct FrontendConfigEntry {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub secret: bool,
}

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
    DatamineQuery {
        query_id: String,
        lode_id: String,
        args: Vec<String>,
        #[serde(default = "default_datamine_timeout")]
        timeout_ms: u64,
        #[serde(default)]
        compress: bool,
    },
    DatamineCatalog,
    DatamineProbe {
        lode_id: String,
    },
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

fn default_datamine_timeout() -> u64 {
    5000
}
