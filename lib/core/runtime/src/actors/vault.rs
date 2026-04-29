//! VaultActor — secret storage access. Actor owns VaultState directly.

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_observability::{ObsMsg, Traceable};

use super::ComponentMsg;

pub struct VaultActor;

pub struct VaultActorState {
    vault: harmonia_vault::VaultState,
    obs: Option<ActorRef<ObsMsg>>,
}

impl Actor for VaultActor {
    type Msg = ComponentMsg;
    type State = VaultActorState;
    type Arguments = Option<ActorRef<ObsMsg>>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        obs: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let mut vault = harmonia_vault::VaultState::new();
        if let Err(e) = harmonia_vault::init_state(&mut vault) {
            eprintln!("[WARN] [runtime] VaultActor init failed: {e}");
        }
        eprintln!("[INFO] [runtime] VaultActor started");
        Ok(VaultActorState { vault, obs })
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
                    let symbol = crate::dispatch::extract_vault_symbol(&sexp);
                    if !symbol.is_empty() {
                        state.obs.trace_event("vault-access", "tool", json!({"symbol": symbol}));
                    }
                }
                let result = crate::dispatch::dispatch_vault(&sexp, &mut state.vault);
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
