use serde::{Deserialize, Serialize};

use crate::request::NodeFsEntry;

pub const PROTOCOL_VERSION: u8 = 1;

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

pub type NodeRpcRequestEnvelope = RpcEnvelope<crate::request::NodeRpcRequest>;
pub type NodeRpcResponseEnvelope = RpcEnvelope<NodeRpcResponse>;

/// A frontend that supports device pairing via QR code or link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairableFrontend {
    pub name: String,
    pub display: String,
    pub status: String,
    pub pairable: bool,
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
    DatamineQuery {
        query_id: String,
        lode_id: String,
        data: String,
        compressed: bool,
        elapsed_ms: u64,
        #[serde(default)]
        error: Option<String>,
    },
    DatamineCatalog {
        lodes: Vec<String>,
    },
    DatamineProbe {
        lode_id: String,
        available: bool,
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
