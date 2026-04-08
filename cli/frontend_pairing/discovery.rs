//! Frontend discovery and RPC dispatching for local/remote targets.

use harmonia_node_rpc::{
    FrontendConfigEntry, NodeRpcRequest, NodeRpcResponse, NodeRpcResult, PairableFrontend,
};

use super::PairingTarget;

pub(crate) fn detect_target(
    node: &crate::paths::NodeIdentity,
) -> Result<PairingTarget, Box<dyn std::error::Error>> {
    match node.role {
        crate::paths::NodeRole::TuiClient | crate::paths::NodeRole::MqttClient => {
            let pairing = crate::pairing::load_default_pairing(node)?
                .ok_or("no pairing found — pair with an agent node first")?;
            Ok(PairingTarget::Remote {
                node: node.clone(),
                pairing,
            })
        }
        crate::paths::NodeRole::Agent => Ok(PairingTarget::Local {
            node_label: node.label.clone(),
        }),
    }
}

pub(crate) fn list_pairable(
    target: &PairingTarget,
) -> Result<Vec<PairableFrontend>, Box<dyn std::error::Error>> {
    match target {
        PairingTarget::Local { .. } => Ok(crate::node_rpc::list_pairable_frontends_local()),
        PairingTarget::Remote { node, pairing } => {
            let response = crate::node_rpc::request_remote(
                node,
                pairing,
                NodeRpcRequest::FrontendPairList,
                10_000,
            )?;
            match response.body {
                NodeRpcResponse::Success {
                    result: NodeRpcResult::FrontendPairList { frontends },
                } => Ok(frontends),
                NodeRpcResponse::Error { message, .. } => Err(message.into()),
                _ => Err("unexpected response".into()),
            }
        }
    }
}

pub(crate) fn configure_frontend(
    target: &PairingTarget,
    frontend_name: &str,
    values: Vec<FrontendConfigEntry>,
) -> Result<(Option<String>, String), Box<dyn std::error::Error>> {
    match target {
        PairingTarget::Local { .. } => Ok(crate::node_rpc::frontend_configure_local(
            frontend_name,
            &values,
        )?),
        PairingTarget::Remote { node, pairing } => {
            let response = crate::node_rpc::request_remote(
                node,
                pairing,
                NodeRpcRequest::FrontendConfigure {
                    frontend: frontend_name.to_string(),
                    values,
                },
                15_000,
            )?;
            match response.body {
                NodeRpcResponse::Success {
                    result:
                        NodeRpcResult::FrontendConfigure {
                            qr_data,
                            instructions,
                            ..
                        },
                } => Ok((qr_data, instructions)),
                NodeRpcResponse::Error { message, .. } => Err(message.into()),
                _ => Err("unexpected response".into()),
            }
        }
    }
}

pub(crate) fn pair_frontend(
    target: &PairingTarget,
    frontend_name: &str,
) -> Result<(Option<String>, String), Box<dyn std::error::Error>> {
    match target {
        PairingTarget::Local { .. } => {
            Ok(crate::node_rpc::frontend_pair_init_local(frontend_name)?)
        }
        PairingTarget::Remote { node, pairing } => {
            let response = crate::node_rpc::request_remote(
                node,
                pairing,
                NodeRpcRequest::FrontendPairInit {
                    frontend: frontend_name.to_string(),
                },
                15_000,
            )?;
            match response.body {
                NodeRpcResponse::Success {
                    result:
                        NodeRpcResult::FrontendPairInit {
                            qr_data,
                            instructions,
                            ..
                        },
                } => Ok((qr_data, instructions)),
                NodeRpcResponse::Error { message, .. } => Err(message.into()),
                _ => Err("unexpected response".into()),
            }
        }
    }
}

pub(crate) fn frontend_pair_status(
    target: &PairingTarget,
    frontend_name: &str,
) -> Result<(bool, String), Box<dyn std::error::Error>> {
    match target {
        PairingTarget::Local { .. } => {
            Ok(crate::node_rpc::frontend_pair_status_local(frontend_name)?)
        }
        PairingTarget::Remote { node, pairing } => {
            let response = crate::node_rpc::request_remote(
                node,
                pairing,
                NodeRpcRequest::FrontendPairStatus {
                    frontend: frontend_name.to_string(),
                },
                5_000,
            )?;
            match response.body {
                NodeRpcResponse::Success {
                    result:
                        NodeRpcResult::FrontendPairStatus {
                            paired, message, ..
                        },
                } => Ok((paired, message)),
                NodeRpcResponse::Error { message, .. } => Err(message.into()),
                _ => Err("unexpected response".into()),
            }
        }
    }
}
