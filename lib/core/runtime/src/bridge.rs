use std::collections::VecDeque;

use ractor::{Actor, ActorProcessingErr, ActorRef};

use harmonia_actor_protocol::HarmoniaMessage;

use crate::msg::BridgeMsg;

/// SbclBridgeActor collects messages destined for SBCL.
///
/// Any actor producing output for SBCL does `cast!(bridge, BridgeMsg::Enqueue { msg })`.
/// When SBCL calls (:drain) over the Unix socket, the IPC layer does
/// `call!(bridge, BridgeMsg::Drain)` and returns the sexp batch.
pub struct SbclBridgeActor;

pub struct BridgeState {
    queue: VecDeque<HarmoniaMessage>,
}

impl Actor for SbclBridgeActor {
    type Msg = BridgeMsg;
    type State = BridgeState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] SbclBridgeActor started");
        Ok(BridgeState {
            queue: VecDeque::new(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            BridgeMsg::Enqueue { msg } => {
                state.queue.push_back(msg);
            }
            BridgeMsg::Drain(reply) => {
                let sexp = drain_to_sexp(&mut state.queue);
                let _ = reply.send(sexp);
            }
        }
        Ok(())
    }
}

fn drain_to_sexp(queue: &mut VecDeque<HarmoniaMessage>) -> String {
    if queue.is_empty() {
        return "()".to_string();
    }
    let messages: Vec<String> = queue.drain(..).map(|m| m.to_sexp()).collect();
    format!("({})", messages.join(" "))
}
