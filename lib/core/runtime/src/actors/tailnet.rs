//! TailnetActor — mesh network transport.

use ractor::{Actor, ActorProcessingErr, ActorRef};

use harmonia_actor_protocol::{now_unix, ActorKind, HarmoniaMessage, MessagePayload};
use harmonia_observability::ObsMsg;

use crate::msg::BridgeMsg;
use super::ComponentMsg;

pub struct TailnetActor;

pub struct TailnetState {
    bridge: ActorRef<BridgeMsg>,
}

impl Actor for TailnetActor {
    type Msg = ComponentMsg;
    type State = TailnetState;
    type Arguments = (ActorRef<BridgeMsg>, Option<ActorRef<ObsMsg>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, _obs): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let _ = harmonia_tailnet::transport::start_listener();
        eprintln!("[INFO] [runtime] TailnetActor started");
        Ok(TailnetState { bridge })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                let messages = harmonia_tailnet::transport::poll_messages();
                for mesh_msg in messages {
                    let msg = HarmoniaMessage {
                        id: 0,
                        source: 0,
                        target: 0,
                        kind: ActorKind::Tailnet,
                        timestamp: now_unix(),
                        payload: MessagePayload::MeshInbound {
                            from_node: mesh_msg.from.to_string(),
                            msg_type: format!("{:?}", mesh_msg.msg_type),
                            payload: mesh_msg.payload,
                        },
                    };
                    let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("tailnet", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                harmonia_tailnet::transport::stop_listener();
                eprintln!("[INFO] [runtime] TailnetActor shutting down");
            }
        }
        Ok(())
    }
}
