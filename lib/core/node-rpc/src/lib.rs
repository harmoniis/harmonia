pub mod capability;
mod dispatch;
mod request;
mod response;

pub use dispatch::capability_for_request;
pub use request::{FrontendConfigEntry, NodeFsEntry, NodePathRef, NodePathScope, NodeRpcRequest};
pub use response::{
    error_response, success_response, NodeRpcRequestEnvelope, NodeRpcResponse,
    NodeRpcResponseEnvelope, NodeRpcResult, PairableFrontend, RpcEnvelope, PROTOCOL_VERSION,
};

pub use capability::default_capabilities;

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
    fn request_capability_mapping_datamine() {
        assert_eq!(
            capability_for_request(&NodeRpcRequest::DatamineQuery {
                query_id: "test".to_string(),
                lode_id: "git-log".to_string(),
                args: vec![],
                timeout_ms: 5000,
                compress: false,
            }),
            capability::DATAMINE_QUERY
        );
    }

    #[test]
    fn default_capabilities_cover_wallet_write() {
        let caps = default_capabilities();
        assert!(caps.contains(&capability::WALLET_SET_SECRET.to_string()));
        assert!(caps.contains(&capability::SHELL_EXEC.to_string()));
    }
}
