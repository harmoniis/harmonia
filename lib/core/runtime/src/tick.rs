//! Per-actor tick loops — each actor ticks at its own cadence (no thundering herd).

use crate::actors::{ComponentMsg, MatrixMsg};

/// Spawn a per-actor tick loop at the given interval.
pub fn spawn_tick(actor: ractor::ActorRef<ComponentMsg>, interval: std::time::Duration) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(interval);
        loop {
            timer.tick().await;
            let _ = actor.cast(ComponentMsg::Tick);
        }
    });
}

/// Spawn a tick loop for the matrix actor (separate message type).
pub fn spawn_matrix_tick(actor: ractor::ActorRef<MatrixMsg>, interval: std::time::Duration) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(interval);
        loop {
            timer.tick().await;
            let _ = actor.cast(MatrixMsg::Tick);
        }
    });
}
