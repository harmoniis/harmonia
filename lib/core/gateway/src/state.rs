use crate::registry::Registry;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static GATEWAY: OnceLock<GatewayState> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
/// Actor ID assigned by the unified registry (0 = not registered yet)
static ACTOR_ID: AtomicU64 = AtomicU64::new(0);

pub struct GatewayState {
    pub registry: Registry,
    pub initialized: RwLock<bool>,
}

pub fn gateway() -> &'static GatewayState {
    GATEWAY.get_or_init(|| GatewayState {
        registry: Registry::new(),
        initialized: RwLock::new(false),
    })
}

pub fn init() -> Result<(), String> {
    let gw = gateway();
    let mut init = gw.initialized.write();
    if *init {
        return Ok(());
    }
    *init = true;

    // Register as a gateway actor in the unified registry (via dlsym)
    if harmonia_actor_protocol::client::is_available() {
        match harmonia_actor_protocol::client::register("gateway") {
            Ok(id) => {
                ACTOR_ID.store(id, Ordering::SeqCst);
                log::info!("gateway registered as actor {}", id);
            }
            Err(e) => {
                log::warn!("gateway actor registration failed: {}", e);
            }
        }
    }

    Ok(())
}

pub fn actor_id() -> u64 {
    ACTOR_ID.load(Ordering::SeqCst)
}

pub fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub fn set_error(msg: impl Into<String>) {
    *last_error().write() = msg.into();
}

pub fn clear_error() {
    last_error().write().clear();
}
