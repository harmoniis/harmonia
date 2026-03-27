use std::collections::VecDeque;

use ractor::{Actor, ActorProcessingErr, ActorRef};

use harmonia_actor_protocol::HarmoniaMessage;

use crate::msg::BridgeMsg;

/// Maximum queued messages before oldest are dropped.
const MAX_QUEUE_LEN: usize = 4096;

/// SbclBridgeActor collects messages destined for SBCL.
///
/// Any actor producing output for SBCL does `cast!(bridge, BridgeMsg::Enqueue { msg })`.
/// When SBCL calls (:drain) over IPC, the IPC layer does
/// `call!(bridge, BridgeMsg::Drain)` and returns the sexp batch.
pub struct SbclBridgeActor;

pub struct BridgeState {
    queue: VecDeque<HarmoniaMessage>,
    /// Reused across drains to avoid re-allocation.
    drain_buf: String,
    /// Counter for dropped messages (backpressure).
    dropped: u64,
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
            queue: VecDeque::with_capacity(256),
            drain_buf: String::with_capacity(4096),
            dropped: 0,
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
                if state.queue.len() >= MAX_QUEUE_LEN {
                    state.queue.pop_front();
                    state.dropped += 1;
                    if state.dropped % 100 == 1 {
                        eprintln!(
                            "[WARN] [runtime] SBCL bridge queue full, dropped {} messages",
                            state.dropped
                        );
                    }
                }
                state.queue.push_back(msg);
            }
            BridgeMsg::Drain(reply) => {
                let sexp = drain_to_sexp(&mut state.queue, &mut state.drain_buf);
                let _ = reply.send(sexp);
            }
        }
        Ok(())
    }
}

/// Drain all queued messages into a sexp string, reusing the buffer.
fn drain_to_sexp(queue: &mut VecDeque<HarmoniaMessage>, buf: &mut String) -> String {
    buf.clear();
    if queue.is_empty() {
        return "()".to_string();
    }
    buf.push('(');
    for (i, msg) in queue.drain(..).enumerate() {
        if i > 0 {
            buf.push(' ');
        }
        msg.write_sexp(buf);
    }
    buf.push(')');
    buf.clone()
}
