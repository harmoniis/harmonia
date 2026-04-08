//! Restart logic for supervised actors: bridge, components, matrix, observability.

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};

use crate::msg::RuntimeMsg;

use super::state::{RuntimeState, MAX_RESPAWNS};

pub(super) async fn handle_supervision_event(
    myself: ActorRef<RuntimeMsg>,
    message: SupervisionEvent,
    state: &mut RuntimeState,
) -> Result<(), ActorProcessingErr> {
    if state.shutting_down {
        // During shutdown we expect actors to terminate -- don't restart anything.
        return Ok(());
    }

    let failed_id = match &message {
        SupervisionEvent::ActorTerminated(cell, _, reason) => {
            eprintln!(
                "[INFO] [runtime] Supervised actor terminated: id={}, reason={reason:?}",
                cell.get_id()
            );
            cell.get_id()
        }
        SupervisionEvent::ActorFailed(cell, err) => {
            eprintln!(
                "[WARN] [runtime] Supervised actor failed: id={}, err={err}",
                cell.get_id()
            );
            cell.get_id()
        }
        _ => return Ok(()),
    };

    if try_restart_bridge(failed_id, &myself, state).await? {
        return Ok(());
    }
    if try_restart_component(failed_id, &myself, state).await? {
        return Ok(());
    }
    if try_restart_matrix(failed_id, &myself, state).await? {
        return Ok(());
    }
    try_restart_obs(failed_id, &myself, state).await?;

    Ok(())
}

async fn try_restart_bridge(
    failed_id: ractor::ActorId,
    myself: &ActorRef<RuntimeMsg>,
    state: &mut RuntimeState,
) -> Result<bool, ActorProcessingErr> {
    let bridge_id = state.bridge.get_id();
    if failed_id != bridge_id {
        return Ok(false);
    }

    let count = state
        .respawn_counts
        .entry("sbcl-bridge".to_string())
        .or_insert(0);
    *count += 1;
    if *count > MAX_RESPAWNS {
        eprintln!(
            "[ERROR] [runtime] SbclBridgeActor exceeded max respawns ({MAX_RESPAWNS}), giving up"
        );
        return Ok(true);
    }
    eprintln!("[INFO] [runtime] Respawning SbclBridgeActor ({count}/{MAX_RESPAWNS})");
    match Actor::spawn_linked(
        Some("sbcl-bridge".to_string()),
        crate::bridge::SbclBridgeActor,
        (),
        myself.get_cell(),
    )
    .await
    {
        Ok((new_bridge, _)) => {
            state.bridge = new_bridge;
            eprintln!("[INFO] [runtime] SbclBridgeActor respawned successfully");
        }
        Err(e) => {
            eprintln!("[ERROR] [runtime] Failed to respawn SbclBridgeActor: {e}");
        }
    }
    Ok(true)
}

async fn try_restart_component(
    failed_id: ractor::ActorId,
    myself: &ActorRef<RuntimeMsg>,
    state: &mut RuntimeState,
) -> Result<bool, ActorProcessingErr> {
    let component_name = state
        .component_actors
        .iter()
        .find(|(_, r)| r.get_id() == failed_id)
        .map(|(name, _)| name.clone());

    let name = match component_name {
        Some(n) => n,
        None => return Ok(false),
    };

    let count = state.respawn_counts.entry(name.clone()).or_insert(0);
    *count += 1;
    if *count > MAX_RESPAWNS {
        eprintln!(
            "[ERROR] [runtime] Component '{name}' exceeded max respawns ({MAX_RESPAWNS}), giving up"
        );
        state.component_actors.remove(&name);
        return Ok(true);
    }
    eprintln!("[INFO] [runtime] Respawning component actor '{name}' ({count}/{MAX_RESPAWNS})");
    // Unregister crashed actor from DynamicRegistry and TopicBus before respawn.
    if let Some(reg) = &state.dynamic_registry { reg.unregister(&name); }
    if let Some(old_ref) = state.component_actors.get(&name) {
        if let Some(bus) = &state.topic_bus { bus.unsubscribe_all(old_ref); }
    }
    let spawn_result = match name.as_str() {
        "chronicle" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::ChronicleComponentActor,
            (),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "gateway" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::GatewayActor,
            (state.bridge.clone(), state.obs_actor.clone()),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "tailnet" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::TailnetActor,
            (state.bridge.clone(), state.obs_actor.clone()),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "signalograd" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::SignalogradActor,
            (state.bridge.clone(), state.obs_actor.clone()),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "vault" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::VaultActor,
            state.obs_actor.clone(),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "config" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::ConfigActor,
            (),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "provider-router" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::ProviderRouterActor,
            (),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "parallel" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::ParallelActor,
            (),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "router" => Actor::spawn_linked(
            Some(name.clone()),
            crate::actors::RouterActor,
            state.obs_actor.clone(),
            myself.get_cell(),
        )
        .await
        .map(|(r, _)| r),
        "mempalace" => Actor::spawn_linked(Some(name.clone()), crate::actors::MemPalaceActor, (), myself.get_cell()).await.map(|(r, _)| r),
        "terraphon" => Actor::spawn_linked(Some(name.clone()), crate::actors::TerraphonActor, (), myself.get_cell()).await.map(|(r, _)| r),
        "ouroboros" => Actor::spawn_linked(Some(name.clone()), crate::actors::OuroborosActor, (), myself.get_cell()).await.map(|(r, _)| r),
        "sessions" => Actor::spawn_linked(Some(name.clone()), crate::actors::SessionActor, (), myself.get_cell()).await.map(|(r, _)| r),
        // Note: "harmonic-matrix" uses MatrixMsg, handled separately
        _ => {
            eprintln!("[WARN] [runtime] Unknown component actor '{name}', cannot respawn");
            return Ok(true);
        }
    };

    match spawn_result {
        Ok(new_ref) => {
            let caps = crate::components::capabilities_for(&name);
            // Re-register in DynamicRegistry so IPC routes to the new actor.
            if let Some(reg) = &state.dynamic_registry {
                reg.register(&name, new_ref.clone(), caps);
            }
            // Re-subscribe to TopicBus for all capabilities.
            if let Some(bus) = &state.topic_bus {
                for cap in caps { bus.subscribe(cap, new_ref.clone()); }
            }
            state.component_actors.insert(name.clone(), new_ref);
            eprintln!("[INFO] [runtime] Component actor '{name}' respawned and re-registered");
        }
        Err(e) => {
            eprintln!("[ERROR] [runtime] Failed to respawn component actor '{name}': {e}");
        }
    }
    Ok(true)
}

async fn try_restart_matrix(
    failed_id: ractor::ActorId,
    myself: &ActorRef<RuntimeMsg>,
    state: &mut RuntimeState,
) -> Result<bool, ActorProcessingErr> {
    let is_matrix = state
        .matrix_actor
        .as_ref()
        .map(|m| m.get_id() == failed_id)
        .unwrap_or(false);

    if !is_matrix {
        return Ok(false);
    }

    eprintln!("[INFO] [runtime] Respawning HarmonicMatrixActor after failure");
    match Actor::spawn_linked(
        Some("harmonic-matrix".to_string()),
        crate::actors::HarmonicMatrixActor,
        (),
        myself.get_cell(),
    )
    .await
    {
        Ok((new_ref, _)) => {
            state.matrix_actor = Some(new_ref);
            eprintln!("[INFO] [runtime] HarmonicMatrixActor respawned successfully");
        }
        Err(e) => {
            eprintln!("[ERROR] [runtime] Failed to respawn HarmonicMatrixActor: {e}");
        }
    }
    Ok(true)
}

async fn try_restart_obs(
    failed_id: ractor::ActorId,
    myself: &ActorRef<RuntimeMsg>,
    state: &mut RuntimeState,
) -> Result<bool, ActorProcessingErr> {
    let is_obs = state
        .obs_actor
        .as_ref()
        .map(|o| o.get_id() == failed_id)
        .unwrap_or(false);

    if !is_obs {
        return Ok(false);
    }

    eprintln!("[INFO] [runtime] Respawning ObservabilityActor after failure");
    let sender = harmonia_observability::start_sender();
    let config = harmonia_observability::get_config().cloned();
    match Actor::spawn_linked(
        Some("observability".to_string()),
        crate::actors::ObservabilityActor,
        (sender, config),
        myself.get_cell(),
    )
    .await
    {
        Ok((new_ref, _)) => {
            harmonia_observability::set_obs_actor(new_ref.clone());
            state.obs_actor = Some(new_ref);
            eprintln!("[INFO] [runtime] ObservabilityActor respawned successfully");
        }
        Err(e) => {
            eprintln!("[ERROR] [runtime] Failed to respawn ObservabilityActor: {e}");
        }
    }
    Ok(true)
}
