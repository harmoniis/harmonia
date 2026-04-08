//! VaultActor — secret storage access.

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_observability::{ObsMsg, Traceable};

use super::ComponentMsg;

pub struct VaultActor;

impl Actor for VaultActor {
    type Msg = ComponentMsg;
    type State = Option<ActorRef<ObsMsg>>;
    type Arguments = Option<ActorRef<ObsMsg>>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        obs: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] VaultActor started");
        Ok(obs)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Dispatch(sexp, reply) => {
                if harmonia_observability::harmonia_observability_is_verbose() {
                    // Trace vault access (symbol name only, never the value)
                    let symbol = crate::dispatch::extract_vault_symbol(&sexp);
                    if !symbol.is_empty() {
                        state.trace_event("vault-access", "tool", json!({"symbol": symbol}));
                    }
                }
                let result = crate::dispatch::dispatch("vault", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] VaultActor shutting down");
            }
            ComponentMsg::Tick => { /* vault does not tick */ }
        }
        Ok(())
    }
}
