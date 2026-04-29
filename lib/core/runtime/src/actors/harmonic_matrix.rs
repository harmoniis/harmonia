//! HarmonicMatrixActor — typed message enum, serialized operations.
//!
//! The matrix owns its state through the mailbox.
//! All operations are serialized through the actor, no lock contention.

use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

pub enum MatrixMsg {
    RegisterNode {
        id: String,
        kind: String,
    },
    RegisterEdge {
        from: String,
        to: String,
        weight: f64,
        min_harmony: f64,
    },
    ObserveRoute {
        from: String,
        to: String,
        success: bool,
        latency_ms: u64,
        cost_usd: f64,
    },
    LogEvent {
        component: String,
        direction: String,
        channel: String,
        payload: String,
        success: bool,
        error: String,
    },
    SetToolEnabled {
        node: String,
        enabled: bool,
    },
    RouteAllowed {
        from: String,
        to: String,
        signal: f64,
        noise: f64,
        reply: RpcReplyPort<bool>,
    },
    Report(RpcReplyPort<String>),
    StoreSummary(RpcReplyPort<String>),
    Tick,
    Shutdown,
}

pub struct HarmonicMatrixActor;

impl Actor for HarmonicMatrixActor {
    type Msg = MatrixMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        let _ = harmonia_harmonic_matrix::runtime::store::init();
        eprintln!("[INFO] [runtime] HarmonicMatrixActor started");
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            MatrixMsg::RegisterNode { id, kind } => {
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::register_node(&id, &kind) {
                    eprintln!("[WARN] [matrix] register-node failed: {e}");
                }
            }
            MatrixMsg::RegisterEdge {
                from,
                to,
                weight,
                min_harmony,
            } => {
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::register_edge(
                    &from,
                    &to,
                    weight,
                    min_harmony,
                ) {
                    eprintln!("[WARN] [matrix] register-edge failed: {e}");
                }
            }
            MatrixMsg::ObserveRoute {
                from,
                to,
                success,
                latency_ms,
                cost_usd,
            } => {
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::observe_route(
                    &from, &to, success, latency_ms, cost_usd,
                ) {
                    eprintln!("[WARN] [matrix] observe-route failed: {e}");
                }
            }
            MatrixMsg::LogEvent {
                component,
                direction,
                channel,
                payload,
                success,
                error,
            } => {
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::log_event(
                    &component, &direction, &channel, &payload, success, &error,
                ) {
                    eprintln!("[WARN] [matrix] log-event failed: {e}");
                }
            }
            MatrixMsg::SetToolEnabled { node, enabled } => {
                if let Err(e) =
                    harmonia_harmonic_matrix::runtime::ops::set_tool_enabled(&node, enabled)
                {
                    eprintln!("[WARN] [matrix] set-tool-enabled failed: {e}");
                }
            }
            MatrixMsg::RouteAllowed {
                from,
                to,
                signal,
                noise,
                reply,
            } => {
                let result = harmonia_harmonic_matrix::runtime::ops::route_allowed(
                    &from, &to, signal, noise,
                );
                let _ = reply.send(result.unwrap_or(false));
            }
            MatrixMsg::Report(reply) => {
                let result = harmonia_harmonic_matrix::runtime::reports::report()
                    .unwrap_or_else(|e| format!("(:error \"{}\")", e));
                let _ = reply.send(result);
            }
            MatrixMsg::StoreSummary(reply) => {
                let result = harmonia_harmonic_matrix::runtime::store::store_summary()
                    .unwrap_or_else(|e| format!("(:error \"{}\")", e));
                let _ = reply.send(result);
            }
            MatrixMsg::Tick => {
                // Advance the matrix epoch: increment the counter, age the rolling
                // route-sample histories so long-running processes do not grow
                // unbounded, persist when the store opts in.
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::advance_epoch() {
                    eprintln!("[WARN] [matrix] advance-epoch failed: {e}");
                }
            }
            MatrixMsg::Shutdown => {
                eprintln!("[INFO] [runtime] HarmonicMatrixActor shutting down");
            }
        }
        Ok(())
    }
}
