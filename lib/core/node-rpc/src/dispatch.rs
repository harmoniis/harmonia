use crate::capability;
use crate::request::NodeRpcRequest;

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
        NodeRpcRequest::DatamineQuery { .. } => capability::DATAMINE_QUERY,
        NodeRpcRequest::DatamineCatalog => capability::DATAMINE_CATALOG,
        NodeRpcRequest::DatamineProbe { .. } => capability::DATAMINE_PROBE,
    }
}
